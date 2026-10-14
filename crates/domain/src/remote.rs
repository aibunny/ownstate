//! Pure, deterministic repository remote normalization.
//!
//! Equivalent SCP-style SSH, SSH URL, HTTPS, HTTP, and Git transports map to
//! one provider repository identity. The canonical locator never contains a
//! transport scheme, user information, query, or fragment.

use url::{Host, Url};

use crate::capture::{RepositoryLocator, RepositoryProvider};
use crate::error::DomainError;

const MAX_REMOTE_LENGTH: usize = 2048;

/// Normalize a network Git remote into a credential-free repository locator.
///
/// This function performs no I/O. Local filesystem paths are intentionally
/// rejected: a path alone is not a durable repository identity and a remote
/// MCP client must never choose a server filesystem path.
pub fn normalize_remote(raw: &str) -> Result<RepositoryLocator, DomainError> {
    let raw = raw.trim();
    validate_raw(raw)?;

    if !raw.contains("://")
        && !raw.starts_with('/')
        && !raw.starts_with('.')
        && let Some((authority, path)) = raw.split_once(':')
    {
        return parse_scp(authority, path);
    }

    parse_url(raw)
}

fn validate_raw(raw: &str) -> Result<(), DomainError> {
    if raw.is_empty() {
        return Err(invalid("empty remote"));
    }
    if raw.len() > MAX_REMOTE_LENGTH {
        return Err(invalid("remote exceeds maximum length"));
    }
    if !raw.is_ascii() {
        return Err(invalid("remote must use ASCII host and path characters"));
    }
    if raw.chars().any(char::is_control) {
        return Err(invalid("remote contains control characters"));
    }
    Ok(())
}

fn parse_scp(authority: &str, raw_path: &str) -> Result<RepositoryLocator, DomainError> {
    if authority.is_empty() || raw_path.is_empty() || authority.contains(['/', '\\']) {
        return Err(invalid("malformed SCP-style remote"));
    }
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host.is_empty() || host.contains('@') || host.contains(':') {
        return Err(invalid("malformed SCP-style host"));
    }
    let host = match Host::parse(host).map_err(|_| invalid("invalid remote host"))? {
        Host::Domain(domain) => domain.to_ascii_lowercase(),
        Host::Ipv4(address) => address.to_string(),
        Host::Ipv6(_) => return Err(invalid("IPv6 SCP-style remotes require ssh:// syntax")),
    };
    let path = raw_path
        .split(['?', '#'])
        .next()
        .ok_or_else(|| invalid("missing repository path"))?;
    build_locator(&host, None, path)
}

fn parse_url(raw: &str) -> Result<RepositoryLocator, DomainError> {
    let url = Url::parse(raw).map_err(|_| invalid("malformed remote URL"))?;
    if !matches!(url.scheme(), "git" | "ssh" | "https" | "http") {
        return Err(invalid("unsupported remote scheme"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| invalid("remote URL has no host"))?
        .to_ascii_lowercase();
    let port = match (url.scheme(), url.port()) {
        ("ssh", Some(22)) | ("https", Some(443)) | ("http", Some(80)) => None,
        (_, port) => port,
    };
    build_locator(&host, port, url.path())
}

fn build_locator(
    host: &str,
    port: Option<u16>,
    raw_path: &str,
) -> Result<RepositoryLocator, DomainError> {
    validate_host(host)?;
    let path = raw_path.trim_matches('/');
    let path = path
        .strip_suffix(".git")
        .unwrap_or(path)
        .trim_end_matches('/');
    validate_path(path)?;

    let provider = detect_provider(host);
    let normalized_path = if matches!(
        provider,
        RepositoryProvider::GitHub | RepositoryProvider::GitLab | RepositoryProvider::Bitbucket
    ) {
        path.to_ascii_lowercase()
    } else {
        path.to_string()
    };
    let authority = port.map_or_else(|| host.to_string(), |port| format!("{host}:{port}"));
    let canonical = format!("{authority}/{normalized_path}");
    let (owner, repository_name) = extract_owner_repo(&normalized_path);

    Ok(RepositoryLocator {
        internal_key: compute_internal_key(&canonical),
        canonical_uri: canonical,
        provider,
        owner,
        repository_name,
        upstream: None,
    })
}

fn validate_host(host: &str) -> Result<(), DomainError> {
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || host.contains("..")
        || host.contains(['/', '\\', '@'])
    {
        return Err(invalid("invalid remote host"));
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), DomainError> {
    if path.is_empty() || path.len() > MAX_REMOTE_LENGTH {
        return Err(invalid("invalid repository path"));
    }
    if path.contains('%') || path.contains('\\') || path.contains(':') {
        return Err(invalid("ambiguous encoded repository path"));
    }
    let segments: Vec<_> = path.split('/').collect();
    if segments.len() < 2
        || segments
            .iter()
            .any(|segment| segment.is_empty() || matches!(*segment, "." | ".."))
    {
        return Err(invalid(
            "repository path must contain an owner and repository",
        ));
    }
    if !segments.iter().all(|segment| {
        segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
    }) {
        return Err(invalid("repository path contains unsupported characters"));
    }
    Ok(())
}

fn extract_owner_repo(path: &str) -> (Option<String>, Option<String>) {
    match path.rsplit_once('/') {
        Some((owner, repository)) => (Some(owner.to_string()), Some(repository.to_string())),
        None => (None, None),
    }
}

fn detect_provider(host: &str) -> RepositoryProvider {
    match host {
        "github.com" => RepositoryProvider::GitHub,
        "gitlab.com" => RepositoryProvider::GitLab,
        "bitbucket.org" => RepositoryProvider::Bitbucket,
        _ => RepositoryProvider::SelfHosted,
    }
}

fn compute_internal_key(canonical: &str) -> String {
    crate::hash::content_hash(format!("git-repository:{canonical}").as_bytes())
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidRemoteUrl {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_github_transports_have_one_identity() {
        let remotes = [
            "git@github.com:Acme/Project.git",
            "ssh://git@github.com/Acme/Project.git",
            "https://github.com/acme/project.git",
            "http://github.com/acme/project/",
        ];
        let locators: Vec<_> = remotes
            .iter()
            .map(|remote| normalize_remote(remote).unwrap())
            .collect();
        assert!(locators.iter().all(|locator| {
            locator.canonical_uri == "github.com/acme/project"
                && locator.internal_key == locators[0].internal_key
        }));
    }

    #[test]
    fn credentials_query_and_fragment_are_stripped() {
        let loc = normalize_remote(
            "https://token:ghp_secret@github.com/acme/project.git?access_token=secret#credential",
        )
        .unwrap();
        assert_eq!(loc.canonical_uri, "github.com/acme/project");
        assert!(!format!("{loc:?}").contains("secret"));
    }

    #[test]
    fn self_hosted_non_default_ports_remain_distinct() {
        let a = normalize_remote("ssh://git@git.example.com:22/team/repo.git").unwrap();
        let b = normalize_remote("ssh://git@git.example.com:2222/team/repo.git").unwrap();
        assert_eq!(a.canonical_uri, "git.example.com/team/repo");
        assert_eq!(b.canonical_uri, "git.example.com:2222/team/repo");
        assert_ne!(a.internal_key, b.internal_key);
    }

    #[test]
    fn deceptive_provider_host_is_self_hosted() {
        let loc = normalize_remote("https://github.com.attacker.example/acme/project.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::SelfHosted);
    }

    #[test]
    fn rejects_local_ambiguous_and_unsafe_inputs() {
        for remote in [
            "",
            "/tmp/repository",
            "file:///tmp/repository",
            "https://github.com/acme/../secret.git",
            "https://github.com/acme/%2e%2e.git",
            "https://github.com/acme/https://evil.git",
            "https://github.com/acme/repo\n.git",
            "https://githuв.com/acme/repo.git",
        ] {
            assert!(normalize_remote(remote).is_err(), "accepted {remote:?}");
        }
    }

    #[test]
    fn provider_and_names_are_extracted() {
        let github = normalize_remote("https://github.com/Acme/Project.git").unwrap();
        assert_eq!(github.provider, RepositoryProvider::GitHub);
        assert_eq!(github.owner.as_deref(), Some("acme"));
        assert_eq!(github.repository_name.as_deref(), Some("project"));

        let gitlab = normalize_remote("https://gitlab.com/group/subgroup/project.git").unwrap();
        assert_eq!(gitlab.provider, RepositoryProvider::GitLab);
        assert_eq!(gitlab.owner.as_deref(), Some("group/subgroup"));
        assert_eq!(gitlab.repository_name.as_deref(), Some("project"));
    }
}
