use std::collections::BTreeMap;

use owo_colors::{OwoColorize, Stream};
use serde::Serialize;

use crate::scanner::CommitRecord;
use crate::validator::{ValidationResult, Violation};

#[derive(Debug, Serialize)]
pub struct CommitReport {
    pub sha: String,
    pub author: String,
    pub subject: String,
    pub status: &'static str,
    pub violations: Vec<Violation>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub total: usize,
    pub clean: usize,
    pub warnings: usize,
    pub errors: usize,
    pub skipped: usize,
    pub score: u8,
    pub by_type: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct AuthorStats {
    pub author: String,
    pub total: usize,
    pub clean: usize,
    pub errors: usize,
    pub score: u8,
}

#[derive(Debug, Serialize)]
pub struct FullReport {
    pub commits: Vec<CommitReport>,
    pub summary: Summary,
    pub authors: Option<Vec<AuthorStats>>,
}

pub fn build(
    commits: &[CommitRecord],
    results: &[ValidationResult],
    include_authors: bool,
) -> FullReport {
    assert_eq!(commits.len(), results.len());
    let mut commit_reports = Vec::with_capacity(commits.len());
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut clean = 0usize;
    let mut warnings = 0usize;
    let mut errors = 0usize;
    let mut skipped = 0usize;

    for (c, r) in commits.iter().zip(results.iter()) {
        let status = if r.skipped {
            skipped += 1;
            "skipped"
        } else if r.violations.iter().any(|v| v.is_error()) {
            errors += 1;
            "error"
        } else if !r.violations.is_empty() {
            warnings += 1;
            clean += 1; // warnings still count as ok
            "warning"
        } else {
            clean += 1;
            "clean"
        };

        if let Some(p) = &r.parsed {
            *by_type.entry(p.r#type.clone()).or_insert(0) += 1;
        }

        commit_reports.push(CommitReport {
            sha: c.sha.clone(),
            author: c.author.clone(),
            subject: c.subject.clone(),
            status,
            violations: r.violations.clone(),
        });
    }

    let counted = commits.len().saturating_sub(skipped);
    let score = if counted == 0 {
        100
    } else {
        ((clean as f64 / counted as f64) * 100.0).round() as u8
    };

    let authors = if include_authors {
        let mut map: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
        for (c, r) in commits.iter().zip(results.iter()) {
            if r.skipped {
                continue;
            }
            let entry = map.entry(c.author.clone()).or_insert((0, 0, 0));
            entry.0 += 1;
            if r.violations.iter().any(|v| v.is_error()) {
                entry.2 += 1;
            } else {
                entry.1 += 1;
            }
        }
        let mut v: Vec<AuthorStats> = map
            .into_iter()
            .map(|(author, (total, clean, errors))| {
                let score = if total == 0 {
                    100
                } else {
                    ((clean as f64 / total as f64) * 100.0).round() as u8
                };
                AuthorStats {
                    author,
                    total,
                    clean,
                    errors,
                    score,
                }
            })
            .collect();
        v.sort_by(|a, b| b.total.cmp(&a.total));
        Some(v)
    } else {
        None
    };

    FullReport {
        commits: commit_reports,
        summary: Summary {
            total: commits.len(),
            clean,
            warnings,
            errors,
            skipped,
            score,
            by_type,
        },
        authors,
    }
}

pub fn render_pretty(report: &FullReport, verbose: bool) -> String {
    let mut out = String::new();
    use std::fmt::Write;

    if report.commits.is_empty() {
        let _ = writeln!(
            out,
            "{}",
            "no commits to scan".if_supports_color(Stream::Stdout, |t| t.dimmed())
        );
        return out;
    }

    let _ = writeln!(
        out,
        "{}",
        "kagu scan".if_supports_color(Stream::Stdout, |t| t.bold())
    );
    let _ = writeln!(out);

    for c in &report.commits {
        let short: String = c.sha.chars().take(7).collect();
        let badge = match c.status {
            "clean" => "✓"
                .if_supports_color(Stream::Stdout, |t| t.green())
                .to_string(),
            "warning" => "!"
                .if_supports_color(Stream::Stdout, |t| t.yellow())
                .to_string(),
            "error" => "✗"
                .if_supports_color(Stream::Stdout, |t| t.red())
                .to_string(),
            "skipped" => "·"
                .if_supports_color(Stream::Stdout, |t| t.dimmed())
                .to_string(),
            _ => "?".to_string(),
        };
        // Always show errors; otherwise honor verbose.
        let show = verbose || c.status == "error" || c.status == "warning";
        if show {
            let _ = writeln!(
                out,
                "  {} {}  {}",
                badge,
                short.if_supports_color(Stream::Stdout, |t| t.dimmed()),
                c.subject
            );
            for v in &c.violations {
                let tag = if v.is_error() {
                    "error"
                        .if_supports_color(Stream::Stdout, |t| t.red())
                        .to_string()
                } else {
                    "warn "
                        .if_supports_color(Stream::Stdout, |t| t.yellow())
                        .to_string()
                };
                let _ = writeln!(out, "      {} [{}] {}", tag, v.code, v.message);
            }
        }
    }

    let s = &report.summary;
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "{}",
        "summary".if_supports_color(Stream::Stdout, |t| t.bold())
    );
    let _ = writeln!(
        out,
        "  total: {}  clean: {}  warnings: {}  errors: {}  skipped: {}",
        s.total,
        s.clean
            .to_string()
            .if_supports_color(Stream::Stdout, |t| t.green()),
        s.warnings
            .to_string()
            .if_supports_color(Stream::Stdout, |t| t.yellow()),
        s.errors
            .to_string()
            .if_supports_color(Stream::Stdout, |t| t.red()),
        s.skipped
            .to_string()
            .if_supports_color(Stream::Stdout, |t| t.dimmed()),
    );
    let score_str = s.score.to_string();
    let score_colored = if s.score >= 90 {
        score_str
            .if_supports_color(Stream::Stdout, |t| t.green())
            .to_string()
    } else if s.score >= 70 {
        score_str
            .if_supports_color(Stream::Stdout, |t| t.yellow())
            .to_string()
    } else {
        score_str
            .if_supports_color(Stream::Stdout, |t| t.red())
            .to_string()
    };
    let _ = writeln!(out, "  score: {}/100", score_colored);

    if !s.by_type.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{}",
            "types".if_supports_color(Stream::Stdout, |t| t.bold())
        );
        for (k, v) in &s.by_type {
            let _ = writeln!(out, "  {:<10} {}", k, v);
        }
    }

    if let Some(authors) = &report.authors {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{}",
            "authors".if_supports_color(Stream::Stdout, |t| t.bold())
        );
        let _ = writeln!(
            out,
            "  {:<24} {:>6} {:>6} {:>6} {:>6}",
            "author", "total", "clean", "errors", "score"
        );
        for a in authors {
            let _ = writeln!(
                out,
                "  {:<24} {:>6} {:>6} {:>6} {:>5}/100",
                a.author, a.total, a.clean, a.errors, a.score
            );
        }
    }

    out
}

/// Serialize a report as pretty JSON. Returns an error if serialization fails
/// (should never happen for well-formed reports, but we surface it instead of
/// silently hiding it).
pub fn render_json(report: &FullReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}
