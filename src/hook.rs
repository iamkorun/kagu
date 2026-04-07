use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SENTINEL: &str = "# kagu-managed-hook";

#[derive(Debug)]
pub enum HookError {
    NotAGitRepo,
    AlreadyInstalled,
    NotManaged,
    NotInstalled,
    Io(io::Error),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookError::NotAGitRepo => write!(f, "not inside a git repository"),
            HookError::AlreadyInstalled => write!(f, "a commit-msg hook already exists"),
            HookError::NotManaged => write!(f, "existing commit-msg hook is not managed by kagu"),
            HookError::NotInstalled => write!(f, "no commit-msg hook installed"),
            HookError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for HookError {}

impl From<io::Error> for HookError {
    fn from(e: io::Error) -> Self {
        HookError::Io(e)
    }
}

pub fn hooks_dir(repo_dir: &Path) -> Result<PathBuf, HookError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-parse", "--git-path", "hooks"])
        .output()?;
    if !out.status.success() {
        return Err(HookError::NotAGitRepo);
    }
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let path = if Path::new(&rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        repo_dir.join(rel)
    };
    Ok(path)
}

fn hook_body() -> String {
    format!("#!/bin/sh\n{SENTINEL}\nexec kagu lint \"$1\"\n")
}

pub fn install(repo_dir: &Path) -> Result<PathBuf, HookError> {
    let dir = hooks_dir(repo_dir)?;
    fs::create_dir_all(&dir)?;
    let path = dir.join("commit-msg");
    if path.exists() {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if !existing.contains(SENTINEL) {
            return Err(HookError::AlreadyInstalled);
        }
    }
    fs::write(&path, hook_body())?;
    set_executable(&path)?;
    Ok(path)
}

pub fn uninstall(repo_dir: &Path) -> Result<PathBuf, HookError> {
    let dir = hooks_dir(repo_dir)?;
    let path = dir.join("commit-msg");
    if !path.exists() {
        return Err(HookError::NotInstalled);
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if !existing.contains(SENTINEL) {
        return Err(HookError::NotManaged);
    }
    fs::remove_file(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = fs::metadata(path)?.permissions();
    perm.set_mode(0o755);
    fs::set_permissions(path, perm)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
