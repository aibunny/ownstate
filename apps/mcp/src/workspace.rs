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
    /// `remote.origin.url` of the enclosing git repository, when present.
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
    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!url.is_empty() && url.len() <= 500).then_some(url)
}
