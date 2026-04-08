use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::hook;
use crate::report;
use crate::scanner;
use crate::validator::{validate, ValidatorOptions};

const MAIN_LONG_ABOUT: &str = "\
Zero-config conventional commit auditor.

Point kagu at a git repository and it tells you which commits break
the Conventional Commits v1.0.0 spec. Ships as a single binary, with
no config file, no Node.js, and no setup.

EXAMPLES:
  kagu scan                      audit the full history of the current repo
  kagu scan --since main         audit only commits not on main
  kagu scan --json --authors     machine-readable output with per-author stats
  kagu lint .git/COMMIT_EDITMSG  lint a single commit message file
  kagu hook install              install kagu as a commit-msg hook

EXIT CODES:
  0   all commits are spec-compliant
  1   one or more violations found
  2   system error (path missing, git not found, invalid --since ref)
";

const SCAN_AFTER_HELP: &str = "\
EXAMPLES:
  kagu scan                      audit the full history of the current repo
  kagu scan --since main         CI-friendly: only new commits on this branch
  kagu scan --strict --authors   require scope, show per-author breakdown
  kagu scan --json > report.json dump machine-readable JSON

EXIT CODES:
  0   all commits are spec-compliant
  1   one or more violations found
  2   system error (path missing, git not found, invalid --since ref)
";

/// Zero-config conventional commit auditor.
#[derive(Debug, Parser)]
#[command(
    name = "kagu",
    version,
    about,
    long_about = MAIN_LONG_ABOUT,
    arg_required_else_help = true,
)]
struct Cli {
    /// Suppress non-essential output.
    #[arg(long, short, global = true, conflicts_with = "verbose")]
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
#[command(after_help = SCAN_AFTER_HELP)]
struct ScanArgs {
    /// Repository path (default: current directory).
    #[arg(long, short = 'p', default_value = ".", value_name = "DIR")]
    path: PathBuf,

    /// Only scan commits in `<since>..HEAD` (e.g. `main`, `v1.0.0`).
    #[arg(long, value_name = "REF")]
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
        match report::render_json(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("kagu: failed to serialize report: {e}");
                return 2;
            }
        }
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
