use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};

use crate::hook;
use crate::report;
use crate::scanner;
use crate::validator::{validate, ValidatorOptions};

/// Zero-config conventional commit auditor.
#[derive(Debug, Parser)]
#[command(name = "kagu", version, about, long_about = None)]
struct Cli {
    /// Suppress non-essential output.
    #[arg(long, short, global = true)]
    quiet: bool,

    /// Print every commit, not just violations.
    #[arg(long, short, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Scan a git repository's history for conventional-commit violations.
    Scan(ScanArgs),
    /// Lint a single commit message file (used by the commit-msg hook).
    Lint(LintArgs),
    /// Manage the commit-msg git hook.
    Hook(HookArgs),
}

#[derive(Debug, Args)]
struct ScanArgs {
    /// Repository path (default: current directory).
    #[arg(long, default_value = ".")]
    path: PathBuf,

    /// Only scan commits in `<since>..HEAD` (e.g. `main`, `v1.0.0`).
    #[arg(long)]
    since: Option<String>,

    /// Include a per-author breakdown.
    #[arg(long)]
    authors: bool,

    /// Output as JSON instead of a human-readable report.
    #[arg(long)]
    json: bool,

    /// Require `(scope)` on every commit.
    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct LintArgs {
    /// Path to a commit message file.
    file: PathBuf,
    /// Require `(scope)` on every commit.
    #[arg(long)]
    strict: bool,
}

#[derive(Debug, Args)]
struct HookArgs {
    #[command(subcommand)]
    action: HookAction,
}

#[derive(Debug, Subcommand)]
enum HookAction {
    /// Install kagu as the commit-msg hook in the current repo.
    Install {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Remove the kagu-managed commit-msg hook.
    Uninstall {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

pub fn main() -> i32 {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Scan(a) => run_scan(a, cli.quiet, cli.verbose),
        Cmd::Lint(a) => run_lint(a, cli.quiet),
        Cmd::Hook(a) => run_hook(a, cli.quiet),
    }
}

fn run_scan(args: ScanArgs, quiet: bool, verbose: bool) -> i32 {
    let opts = ValidatorOptions {
        strict: args.strict,
        ..Default::default()
    };

    let commits = match scanner::scan(&args.path, args.since.as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("kagu: {e}");
            return 2;
        }
    };

    let results: Vec<_> = commits.iter().map(|c| validate(&c.subject, opts)).collect();

    let report = report::build(&commits, &results, args.authors);

    if args.json {
        println!("{}", report::render_json(&report));
    } else if !quiet {
        print!("{}", report::render_pretty(&report, verbose));
    }

    if report.summary.errors > 0 {
        1
    } else {
        0
    }
}

fn run_lint(args: LintArgs, quiet: bool) -> i32 {
    let content = match std::fs::read_to_string(&args.file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("kagu: cannot read {}: {e}", args.file.display());
            return 2;
        }
    };
    let opts = ValidatorOptions {
        strict: args.strict,
        ..Default::default()
    };
    let result = validate(&content, opts);
    if result.ok() {
        if !quiet {
            eprintln!("kagu: commit message ok");
        }
        0
    } else {
        eprintln!("kagu: commit message rejected");
        for v in &result.violations {
            let tag = if v.is_error() { "error" } else { "warn " };
            eprintln!("  {} [{}] {}", tag, v.code, v.message);
        }
        1
    }
}

fn run_hook(args: HookArgs, quiet: bool) -> i32 {
    match args.action {
        HookAction::Install { path } => match hook::install(&path) {
            Ok(p) => {
                if !quiet {
                    println!("kagu: installed commit-msg hook at {}", p.display());
                }
                0
            }
            Err(e) => {
                eprintln!("kagu: {e}");
                2
            }
        },
        HookAction::Uninstall { path } => match hook::uninstall(&path) {
            Ok(p) => {
                if !quiet {
                    println!("kagu: removed hook at {}", p.display());
                }
                0
            }
            Err(e) => {
                eprintln!("kagu: {e}");
                2
            }
        },
    }
}

#[allow(dead_code)]
fn _path_str(p: &Path) -> String {
    p.display().to_string()
}
