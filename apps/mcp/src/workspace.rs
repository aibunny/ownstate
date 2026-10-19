//! Workspace detection — an adapter concern: MCP clients (Claude Code,
//! Codex, …) spawn this server in the directory the agent is working in, so
//! the directory name identifies the project and the git origin (when the
//! directory is a repository) is provenance worth recording.

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Workspace {
    /// Directory name of the working directory (the default project name).
    pub name: String,
    /// Absolute path of the working directory — recorded on the project and
    /// used as an identity factor so the same location resolves to the same
    /// project across clients.
    pub root_path: String,
    /// Credential-free canonical repository locator of the enclosing Git
    /// repository, when present.
    pub git_origin: Option<String>,
}

/// Detect the workspace from the process working directory. Returns `None`
/// when no usable directory name exists (e.g. cwd is `/`).
pub fn detect() -> Option<Workspace> {
    let cwd = std::env::current_dir().ok()?;
    let name = cwd.file_name()?.to_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(Workspace {
        git_origin: git_origin(&cwd),
        root_path: cwd.to_string_lossy().into_owned(),
        name,
    })
}

/// The git origin URL for `dir`, when it lies inside a repository that has
/// an `origin` remote. Read via git itself so worktrees/submodules resolve
/// correctly; the value is recorded as data, never executed or parsed.
fn git_origin(dir: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    ownstate_domain::normalize_remote(raw.trim())
        .ok()
        .map(|locator| locator.canonical_uri)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[test]
    fn canonicalization_strips_git_credentials_before_adapter_output() {
        let locator = ownstate_domain::normalize_remote(
            "https://token:secret@github.com/Acme/Project.git?token=secret",
        )
        .expect("test remote must normalize");
        assert_eq!(locator.canonical_uri, "github.com/acme/project");
        assert!(!locator.canonical_uri.contains("secret"));
    }
}
