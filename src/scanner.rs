use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub sha: String,
    pub author: String,
    pub subject: String,
}

#[derive(Debug)]
pub enum ScanError {
    NotAGitRepo,
    PathNotFound(PathBuf),
    GitMissing,
    GitFailed(String),
    Io(io::Error),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::NotAGitRepo => write!(f, "not inside a git repository"),
            ScanError::PathNotFound(p) => write!(f, "path does not exist: {}", p.display()),
            ScanError::GitMissing => write!(f, "`git` executable not found in PATH"),
            ScanError::GitFailed(s) => write!(f, "git command failed: {s}"),
            ScanError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<io::Error> for ScanError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound {
            ScanError::GitMissing
        } else {
            ScanError::Io(e)
        }
    }
}

const SEP: &str = "\x1eKAGU\x1e";
const REC: &str = "\x1fEND\x1f";

/// Run `git log` in `dir`, optionally constrained by `range` (e.g. "main..HEAD"),
/// returning the commits oldest-first.
pub fn scan(dir: &Path, range: Option<&str>) -> Result<Vec<CommitRecord>, ScanError> {
    if !dir.exists() {
        return Err(ScanError::PathNotFound(dir.to_path_buf()));
    }
    // Verify dir is a git repo first.
    let check = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--git-dir"])
        .output()?;
    if !check.status.success() {
        return Err(ScanError::NotAGitRepo);
    }

    let format = format!("--pretty=format:%H{SEP}%an{SEP}%s{REC}");
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).arg("log").arg(&format);
    if let Some(r) = range {
        cmd.arg(r);
    }

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        // empty repo case → git log fails with "does not have any commits yet"
        if stderr.contains("does not have any commits") {
            return Ok(vec![]);
        }
        return Err(ScanError::GitFailed(stderr.trim().to_string()));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut commits = Vec::new();
    for raw in stdout.split(REC) {
        let raw = raw.trim_matches(|c: char| c.is_whitespace());
        if raw.is_empty() {
            continue;
        }
        let parts: Vec<&str> = raw.splitn(3, SEP).collect();
        if parts.len() != 3 {
            continue;
        }
        commits.push(CommitRecord {
            sha: parts[0].to_string(),
            author: parts[1].to_string(),
            subject: parts[2].to_string(),
        });
    }
    // git log returns newest-first; reverse for oldest-first reading flow.
    commits.reverse();
    Ok(commits)
}
