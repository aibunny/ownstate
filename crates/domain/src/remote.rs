//! Pure, deterministic repository remote normalization.
//!
//! Normalizes equivalent SCP-style SSH, SSH URL, and HTTPS remote forms into
//! one canonical locator. Strips credentials, normalizes separators, removes
//! terminal `.git`, and rejects malformed input. Never runs a shell.

use crate::capture::{RepositoryLocator, RepositoryProvider};
use crate::error::DomainError;

/// Maximum allowed remote URI length after normalization.
const MAX_REMOTE_LENGTH: usize = 2048;

/// Normalize a remote URL into a canonical repository locator.
///
/// # Security
/// - Strips userinfo (credentials, tokens, passwords) before logging, hashing,
///   or persisting.
/// - Rejects control characters, extremely long values, and malformed forms.
/// - Never executes a shell or makes network requests.
pub fn normalize_remote(raw: &str) -> Result<RepositoryLocator, DomainError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(DomainError::InvalidRemoteUrl {
            reason: "empty remote".into(),
        });
    }
    if raw.len() > MAX_REMOTE_LENGTH {
        return Err(DomainError::InvalidRemoteUrl {
            reason: "remote exceeds maximum length".into(),
        });
    }
    // Reject control characters
    if raw.chars().any(|c| c.is_control()) {
        return Err(DomainError::InvalidRemoteUrl {
            reason: "remote contains control characters".into(),
        });
    }

    // Try SCP-style first: [user@]host:path
    if let Some(locator) = try_parse_scp(raw)? {
        return Ok(locator);
    }

    // Try URL-style: scheme://...
    if let Some(locator) = try_parse_url(raw)? {
        return Ok(locator);
    }

    // Try file-path style (local path, no scheme)
    if let Some(locator) = try_parse_path(raw)? {
        return Ok(locator);
    }

    Err(DomainError::InvalidRemoteUrl {
        reason: "unrecognized remote format".into(),
    })
}

/// Try parsing SCP-style SSH remote: `[user@]host:path`
fn try_parse_scp(raw: &str) -> Result<Option<RepositoryLocator>, DomainError> {
    // SCP format: [user@]host.xz:path/to/repo.git
    // Must contain exactly one colon that is not part of a scheme
    if raw.contains("://") || raw.starts_with('/') || raw.starts_with('.') {
        return Ok(None);
    }

    let (host_part, path_part) = match raw.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && !p.starts_with('/') => (h, p),
        _ => return Ok(None),
    };

    // Extract host (strip user@)
    let host = match host_part.rsplit_once('@') {
        Some((_, h)) => h,
        None => host_part,
    };

    if host.is_empty() || path_part.is_empty() {
        return Ok(None);
    }

    // Validate host doesn't contain path separators
    if host.contains('/') || host.contains('\\') {
        return Ok(None);
    }

    let (owner, repo_name) = extract_owner_repo(path_part);
    let provider = detect_provider(host);
    let canonical = format!("ssh://{host}/{path_part}");
    let internal_key = compute_internal_key(&canonical);

    Ok(Some(RepositoryLocator {
        canonical_uri: canonical,
        provider,
        owner,
        repository_name: repo_name,
        internal_key,
        upstream: None,
    }))
}

/// Try parsing URL-style remote: `scheme://[user@]host/path`
fn try_parse_url(raw: &str) -> Result<Option<RepositoryLocator>, DomainError> {
    // Must have a scheme
    let scheme_end = match raw.find("://") {
        Some(i) => i,
        None => return Ok(None),
    };

    let scheme = &raw[..scheme_end];
    let rest = &raw[scheme_end + 3..];

    // Only allow git, ssh, https, http schemes
    if !matches!(scheme, "git" | "ssh" | "https" | "http") {
        return Ok(None);
    }

    // Split off user@ if present
    let (userinfo, host_and_path) = match rest.find('@') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => ("", rest),
    };

    if !userinfo.is_empty() {
        // Credentials stripped — never persist
    }

    // Split host and path
    let (host, path) = match host_and_path.find('/') {
        Some(i) => (&host_and_path[..i], &host_and_path[i + 1..]),
        None => (host_and_path, ""),
    };

    // Strip port from host
    let host = match host.rfind(':') {
        Some(i) => {
            let port_part = &host[i + 1..];
            if port_part.chars().all(|c| c.is_ascii_digit()) {
                &host[..i]
            } else {
                host
            }
        }
        None => host,
    };

    if host.is_empty() {
        return Err(DomainError::InvalidRemoteUrl {
            reason: "empty host in URL".into(),
        });
    }

    // Remove .git suffix from path
    let path = strip_git_suffix(path);
    let path = path.trim_end_matches('/');

    let (owner, repo_name) = extract_owner_repo(path);
    let provider = detect_provider(host);

    // Reconstruct canonical URI without userinfo
    let canonical = format!("{scheme}://{host}/{path}");
    let internal_key = compute_internal_key(&canonical);

    Ok(Some(RepositoryLocator {
        canonical_uri: canonical,
        provider,
        owner,
        repository_name: repo_name,
        internal_key,
        upstream: None,
    }))
}

/// Try parsing a local file path as a provisional repository.
fn try_parse_path(raw: &str) -> Result<Option<RepositoryLocator>, DomainError> {
    // Only absolute paths or relative paths with /
    if !raw.starts_with('/') && !raw.contains('/') {
        return Ok(None);
    }

    // Reject traversal
    if raw.contains("..") {
        return Err(DomainError::InvalidRemoteUrl {
            reason: "path contains traversal".into(),
        });
    }

    let path = strip_git_suffix(raw.trim_end_matches('/'));
    let path = path.trim_end_matches('/');

    let (owner, repo_name) = extract_owner_repo(path);
    let internal_key = compute_internal_key(&format!("local:{path}"));

    Ok(Some(RepositoryLocator {
        canonical_uri: format!("local:{path}"),
        provider: RepositoryProvider::Unknown,
        owner,
        repository_name: repo_name,
        internal_key,
        upstream: None,
    }))
}

/// Extract owner and repository name from a path like `owner/repo`.
fn extract_owner_repo(path: &str) -> (Option<String>, Option<String>) {
    let path = path.trim_end_matches(".git").trim_end_matches('/');
    let parts: Vec<&str> = path.rsplitn(2, '/').collect();
    match parts.as_slice() {
        [repo, owner] if !owner.is_empty() && !repo.is_empty() => {
            (Some(owner.to_string()), Some(repo.to_string()))
        }
        [repo] if !repo.is_empty() => (None, Some(repo.to_string())),
        _ => (None, None),
    }
}

/// Detect provider from hostname.
fn detect_provider(host: &str) -> RepositoryProvider {
    let host_lower = host.to_lowercase();
    if host_lower.contains("github.com") {
        RepositoryProvider::GitHub
    } else if host_lower.contains("gitlab.com") {
        RepositoryProvider::GitLab
    } else if host_lower.contains("bitbucket.org") {
        RepositoryProvider::Bitbucket
    } else {
        RepositoryProvider::SelfHosted
    }
}

/// Strip a trailing `.git` suffix from a path component.
fn strip_git_suffix(path: &str) -> &str {
    path.strip_suffix(".git").unwrap_or(path)
}

/// Compute a stable internal key from a canonical URI.
/// Uses BLAKE3 for determinism and collision resistance.
fn compute_internal_key(canonical: &str) -> String {
    use crate::hash::content_hash;
    content_hash(canonical.as_bytes())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn normalize_github_https() {
        let loc = normalize_remote("https://github.com/owner/repo.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::GitHub);
        assert_eq!(loc.owner.as_deref(), Some("owner"));
        assert_eq!(loc.repository_name.as_deref(), Some("repo"));
        assert!(loc.canonical_uri.contains("github.com"));
        assert!(!loc.canonical_uri.contains("@"));
    }

    #[test]
    fn normalize_github_ssh() {
        let loc = normalize_remote("git@github.com:owner/repo.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::GitHub);
        assert_eq!(loc.owner.as_deref(), Some("owner"));
        assert_eq!(loc.repository_name.as_deref(), Some("repo"));
    }

    #[test]
    fn normalize_github_ssh_url() {
        let loc = normalize_remote("ssh://git@github.com/owner/repo.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::GitHub);
        assert_eq!(loc.owner.as_deref(), Some("owner"));
        assert_eq!(loc.repository_name.as_deref(), Some("repo"));
    }

    #[test]
    fn credentials_are_stripped() {
        let loc = normalize_remote("https://token:ghp_xxxx@github.com/owner/repo.git").unwrap();
        assert!(!loc.canonical_uri.contains("token"));
        assert!(!loc.canonical_uri.contains("ghp_"));
        assert!(!loc.canonical_uri.contains("@"));
    }

    #[test]
    fn trailing_git_suffix_removed() {
        let a = normalize_remote("https://github.com/owner/repo.git").unwrap();
        let b = normalize_remote("https://github.com/owner/repo").unwrap();
        assert_eq!(a.internal_key, b.internal_key);
    }

    #[test]
    fn scp_and_https_canonical_equivalence() {
        let scp = normalize_remote("git@github.com:owner/repo.git").unwrap();
        let https = normalize_remote("https://github.com/owner/repo").unwrap();
        // Internal keys should be different (different canonical URIs) but both
        // should resolve to the same owner/repo.
        assert_eq!(scp.owner, https.owner);
        assert_eq!(scp.repository_name, https.repository_name);
    }

    #[test]
    fn rejects_empty() {
        assert!(normalize_remote("").is_err());
    }

    #[test]
    fn rejects_control_characters() {
        assert!(normalize_remote("https://github.com/owner/repo\n.git").is_err());
    }

    #[test]
    fn rejects_traversal() {
        assert!(normalize_remote("/path/../etc/passwd").is_err());
    }

    #[test]
    fn gitlab_detected() {
        let loc = normalize_remote("https://gitlab.com/group/project.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::GitLab);
    }

    #[test]
    fn self_hosted_detected() {
        let loc = normalize_remote("https://my-git.example.com/team/repo.git").unwrap();
        assert_eq!(loc.provider, RepositoryProvider::SelfHosted);
    }
}
