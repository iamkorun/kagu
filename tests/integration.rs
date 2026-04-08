use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_BIN_EXE_kagu"));
    assert!(p.exists(), "binary should exist");
    p.pop();
    p.push("kagu");
    p
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Tester")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "Tester")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .expect("git ran");
    assert!(status.success(), "git {args:?} failed");
}

fn make_repo() -> tempdir::TempDirLike {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    // commits
    std::fs::write(dir.path().join("a"), "1").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "feat(core): add a"]);
    std::fs::write(dir.path().join("b"), "2").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "broken commit message"]);
    std::fs::write(dir.path().join("c"), "3").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "fix: trailing dot."]);
    dir
}

mod tempdir {
    use std::path::{Path, PathBuf};
    pub struct TempDirLike(PathBuf);
    impl TempDirLike {
        pub fn new() -> Self {
            let mut p = std::env::temp_dir();
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            p.push(format!("kagu-it-{nanos}-{}", std::process::id()));
            std::fs::create_dir_all(&p).unwrap();
            TempDirLike(p)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDirLike {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn scan_reports_violations_and_exits_nonzero() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["scan", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "should exit nonzero on violations");
    assert!(stdout.contains("broken commit message"));
    assert!(stdout.contains("score:"));
}

#[test]
fn scan_json_output_parses() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["scan", "--json", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(v["summary"]["total"], 3);
    assert!(v["summary"]["errors"].as_u64().unwrap() >= 1);
    assert!(v["commits"].as_array().unwrap().len() == 3);
}

#[test]
fn scan_authors_breakdown_present() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["scan", "--authors", "--json", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["authors"].is_array());
    assert!(!v["authors"].as_array().unwrap().is_empty());
}

#[test]
fn lint_accepts_good_message() {
    let dir = tempdir::TempDirLike::new();
    let f = dir.path().join("msg");
    std::fs::write(&f, "feat(cli): add scan").unwrap();
    let out = Command::new(bin()).arg("lint").arg(&f).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn lint_rejects_bad_message() {
    let dir = tempdir::TempDirLike::new();
    let f = dir.path().join("msg");
    std::fs::write(&f, "no type here").unwrap();
    let out = Command::new(bin()).arg("lint").arg(&f).output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("rejected"));
}

#[test]
fn hook_install_then_uninstall() {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    let install = Command::new(bin())
        .args(["hook", "install", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(install.status.success(), "{:?}", install);
    let hook = dir.path().join(".git/hooks/commit-msg");
    assert!(hook.exists());
    let body = std::fs::read_to_string(&hook).unwrap();
    assert!(body.contains("kagu-managed-hook"));

    let un = Command::new(bin())
        .args(["hook", "uninstall", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(un.status.success());
    assert!(!hook.exists());
}

#[test]
fn scan_non_git_dir_errors() {
    let dir = tempdir::TempDirLike::new();
    let out = Command::new(bin())
        .args(["scan", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not inside a git repository"));
}

#[test]
fn scan_missing_path_errors_with_clear_message() {
    let out = Command::new(bin())
        .args(["scan", "--path", "/no/such/dir/xyz"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("path does not exist"), "stderr: {stderr}");
}

#[test]
fn scan_empty_repo_is_clean() {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    let out = Command::new(bin())
        .args(["scan", "--json", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success(), "empty repo should exit 0");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["summary"]["total"], 0);
    assert_eq!(v["summary"]["errors"], 0);
    assert_eq!(v["summary"]["score"], 100);
}

#[test]
fn scan_with_invalid_since_errors() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["scan", "--since", "no-such-ref-xyz", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    assert!(!out.status.success(), "should fail for nonexistent ref");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("git"), "stderr: {stderr}");
}

#[test]
fn lint_missing_file_errors() {
    let out = Command::new(bin())
        .args(["lint", "/no/such/file/exists"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("cannot read"));
}

#[test]
fn lint_strict_rejects_missing_scope() {
    let dir = tempdir::TempDirLike::new();
    let f = dir.path().join("msg");
    std::fs::write(&f, "feat: no scope").unwrap();
    let out = Command::new(bin())
        .args(["lint", "--strict"])
        .arg(&f)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("scope"));
}

#[test]
fn hook_install_is_idempotent_for_own_hook() {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    for _ in 0..2 {
        let out = Command::new(bin())
            .args(["hook", "install", "--path"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
    }
}

#[test]
fn hook_install_refuses_foreign_hook() {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    // Pre-create a foreign commit-msg hook
    let hook_path = dir.path().join(".git/hooks/commit-msg");
    std::fs::create_dir_all(hook_path.parent().unwrap()).unwrap();
    std::fs::write(&hook_path, "#!/bin/sh\necho 'other tool'\n").unwrap();

    let out = Command::new(bin())
        .args(["hook", "install", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

#[test]
fn hook_uninstall_fails_when_not_installed() {
    let dir = tempdir::TempDirLike::new();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    let out = Command::new(bin())
        .args(["hook", "uninstall", "--path"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn quiet_and_verbose_conflict() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["-q", "-v", "scan", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "stderr: {stderr}"
    );
}

#[test]
fn quiet_suppresses_stdout_but_preserves_exit_code() {
    let repo = make_repo();
    let out = Command::new(bin())
        .args(["scan", "--quiet", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    assert!(!out.status.success(), "violations should still exit 1");
    assert!(
        out.stdout.is_empty(),
        "--quiet should suppress stdout, got: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn no_color_env_strips_ansi() {
    let repo = make_repo();
    let out = Command::new(bin())
        .env("NO_COLOR", "1")
        .args(["scan", "--verbose", "--path"])
        .arg(repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "NO_COLOR should strip ANSI codes, got: {stdout:?}"
    );
}

#[test]
fn version_flag_prints_version() {
    let out = Command::new(bin()).arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("kagu"));
}

#[test]
fn help_flag_prints_subcommands() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("scan"));
    assert!(stdout.contains("lint"));
    assert!(stdout.contains("hook"));
}
