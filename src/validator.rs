use serde::{Deserialize, Serialize};

pub const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParsedCommit {
    pub r#type: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Violation {
    pub code: String,
    pub message: String,
    pub severity: Severity,
}

impl Violation {
    fn err(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: Severity::Error,
        }
    }
    fn warn(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            severity: Severity::Warning,
        }
    }
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub parsed: Option<ParsedCommit>,
    pub violations: Vec<Violation>,
    pub skipped: bool,
}

impl ValidationResult {
    pub fn ok(&self) -> bool {
        !self.violations.iter().any(|v| v.is_error())
    }
}

/// Parse a commit subject line into a ParsedCommit, if it loosely matches the
/// conventional-commit shape. Returns None when the format is unrecognizable.
pub fn parse_subject(subject: &str) -> Option<ParsedCommit> {
    let colon = subject.find(':')?;
    let head = &subject[..colon];
    let rest = subject[colon + 1..].trim_start();

    // head = type or type(scope) or type! or type(scope)!
    let (head_no_bang, breaking) = if let Some(stripped) = head.strip_suffix('!') {
        (stripped, true)
    } else {
        (head, false)
    };

    let (type_part, scope_part) = if let Some(open) = head_no_bang.find('(') {
        if !head_no_bang.ends_with(')') {
            return None;
        }
        let scope = &head_no_bang[open + 1..head_no_bang.len() - 1];
        if scope.is_empty() {
            return None;
        }
        (&head_no_bang[..open], Some(scope.to_string()))
    } else {
        (head_no_bang, None)
    };

    if type_part.is_empty() || !type_part.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    Some(ParsedCommit {
        r#type: type_part.to_ascii_lowercase(),
        scope: scope_part,
        breaking,
        description: rest.to_string(),
    })
}

#[derive(Debug, Clone, Copy)]
pub struct ValidatorOptions<'a> {
    pub allowed_types: &'a [&'a str],
    pub strict: bool,
}

impl<'a> Default for ValidatorOptions<'a> {
    fn default() -> Self {
        Self {
            allowed_types: DEFAULT_TYPES,
            strict: false,
        }
    }
}

/// Should this subject be skipped (merge / initial commit)?
pub fn should_skip(subject: &str) -> bool {
    let s = subject.trim();
    if s.starts_with("Merge ") {
        return true;
    }
    if s.eq_ignore_ascii_case("initial commit") {
        return true;
    }
    false
}

pub fn validate(subject: &str, opts: ValidatorOptions<'_>) -> ValidationResult {
    let subject = subject.lines().next().unwrap_or("").trim_end();

    if should_skip(subject) {
        return ValidationResult {
            parsed: None,
            violations: vec![],
            skipped: true,
        };
    }

    let parsed = parse_subject(subject);
    let mut violations = Vec::new();

    let Some(parsed) = parsed else {
        violations.push(Violation::err(
            "format",
            "subject does not match `<type>(<scope>)?!?: <description>`",
        ));
        return ValidationResult {
            parsed: None,
            violations,
            skipped: false,
        };
    };

    if !opts
        .allowed_types
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&parsed.r#type))
    {
        violations.push(Violation::err(
            "type",
            format!(
                "unknown type `{}` (allowed: {})",
                parsed.r#type,
                opts.allowed_types.join(", ")
            ),
        ));
    }

    if parsed.description.is_empty() {
        violations.push(Violation::err("description", "description is empty"));
    } else {
        let char_count = parsed.description.chars().count();
        if char_count > 100 {
            violations.push(Violation::err(
                "length",
                format!("description is {char_count} chars (max 100)"),
            ));
        }
        if parsed.description.ends_with('.') {
            violations.push(Violation::warn(
                "punctuation",
                "description should not end with `.`",
            ));
        }
    }

    if opts.strict && parsed.scope.is_none() {
        violations.push(Violation::err(
            "scope",
            "strict mode requires `(scope)` on every commit",
        ));
    }

    ValidationResult {
        parsed: Some(parsed),
        violations,
        skipped: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ValidatorOptions<'static> {
        ValidatorOptions::default()
    }

    #[test]
    fn parses_simple() {
        let p = parse_subject("feat: add thing").unwrap();
        assert_eq!(p.r#type, "feat");
        assert_eq!(p.scope, None);
        assert!(!p.breaking);
        assert_eq!(p.description, "add thing");
    }

    #[test]
    fn parses_scope_and_breaking() {
        let p = parse_subject("fix(parser)!: handle eof").unwrap();
        assert_eq!(p.r#type, "fix");
        assert_eq!(p.scope.as_deref(), Some("parser"));
        assert!(p.breaking);
        assert_eq!(p.description, "handle eof");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_subject("just a sentence").is_none());
        assert!(parse_subject("FEAT add thing").is_none());
        assert!(parse_subject("feat(: bad").is_none());
        assert!(parse_subject("feat(): empty scope").is_none());
    }

    #[test]
    fn validates_clean_commit() {
        let r = validate("feat(cli): add scan subcommand", opts());
        assert!(r.ok());
        assert!(r.violations.is_empty());
    }

    #[test]
    fn flags_unknown_type() {
        let r = validate("wibble: do thing", opts());
        assert!(!r.ok());
        assert!(r.violations.iter().any(|v| v.code == "type"));
    }

    #[test]
    fn flags_format() {
        let r = validate("just doing some stuff", opts());
        assert!(!r.ok());
        assert!(r.violations.iter().any(|v| v.code == "format"));
    }

    #[test]
    fn flags_long_description() {
        let long = "a".repeat(120);
        let subject = format!("feat: {long}");
        let r = validate(&subject, opts());
        assert!(r.violations.iter().any(|v| v.code == "length"));
    }

    #[test]
    fn warns_trailing_period() {
        let r = validate("feat: add thing.", opts());
        // warning, not error → still ok overall
        assert!(r.ok());
        assert!(r.violations.iter().any(|v| v.code == "punctuation"));
    }

    #[test]
    fn skips_merge() {
        let r = validate("Merge branch 'main'", opts());
        assert!(r.skipped);
        assert!(r.ok());
    }

    #[test]
    fn skips_initial_commit() {
        let r = validate("Initial commit", opts());
        assert!(r.skipped);
    }

    #[test]
    fn strict_requires_scope() {
        let mut o = opts();
        o.strict = true;
        let r = validate("feat: no scope here", o);
        assert!(!r.ok());
        assert!(r.violations.iter().any(|v| v.code == "scope"));

        let r2 = validate("feat(cli): with scope", o);
        assert!(r2.ok());
    }

    #[test]
    fn empty_description() {
        let r = validate("feat: ", opts());
        assert!(r.violations.iter().any(|v| v.code == "description"));
    }

    #[test]
    fn rejects_type_with_digits() {
        assert!(parse_subject("feat1: add thing").is_none());
        assert!(parse_subject("v2: release").is_none());
    }

    #[test]
    fn parses_breaking_without_scope() {
        let p = parse_subject("feat!: drop legacy api").unwrap();
        assert_eq!(p.r#type, "feat");
        assert_eq!(p.scope, None);
        assert!(p.breaking);
        assert_eq!(p.description, "drop legacy api");
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        // 50 multi-byte chars = 150 bytes, but only 50 chars → well under 100
        let desc: String = "é".repeat(50);
        let subject = format!("feat: {desc}");
        let r = validate(&subject, opts());
        assert!(
            !r.violations.iter().any(|v| v.code == "length"),
            "50 multi-byte chars should not trigger length violation"
        );

        // 101 multi-byte chars = 202 bytes → should fail length
        let desc: String = "é".repeat(101);
        let subject = format!("feat: {desc}");
        let r = validate(&subject, opts());
        let len_vio = r
            .violations
            .iter()
            .find(|v| v.code == "length")
            .expect("should have length violation");
        assert!(
            len_vio.message.contains("101 chars"),
            "got: {}",
            len_vio.message
        );
    }

    #[test]
    fn empty_subject() {
        let r = validate("", opts());
        assert!(!r.ok());
        assert!(r.violations.iter().any(|v| v.code == "format"));
    }

    #[test]
    fn whitespace_only_subject() {
        let r = validate("   ", opts());
        assert!(!r.ok());
        assert!(r.violations.iter().any(|v| v.code == "format"));
    }

    #[test]
    fn multiple_violations_on_same_commit() {
        let long: String = "x".repeat(120);
        let subject = format!("wibble: {long}.");
        let r = validate(&subject, opts());
        // unknown type + length + trailing period
        let codes: Vec<&str> = r.violations.iter().map(|v| v.code.as_str()).collect();
        assert!(codes.contains(&"type"), "got: {codes:?}");
        assert!(codes.contains(&"length"), "got: {codes:?}");
        assert!(codes.contains(&"punctuation"), "got: {codes:?}");
    }

    #[test]
    fn multiline_subject_uses_first_line_only() {
        let r = validate("feat: add thing\n\nLonger body here.", opts());
        assert!(r.ok(), "body should not affect validation");
    }

    #[test]
    fn parse_unicode_description() {
        let p = parse_subject("feat: ✨ sparkle").unwrap();
        assert_eq!(p.description, "✨ sparkle");
    }

    #[test]
    fn type_is_case_insensitive() {
        // uppercase type should be normalized and still match
        let r = validate("FEAT: upper case", opts());
        assert!(r.ok(), "case should be normalized");
    }

    #[test]
    fn skip_merge_is_not_case_sensitive_prefix() {
        // "Merge " as a prefix is the convention git uses
        assert!(should_skip("Merge branch 'feature'"));
        assert!(should_skip("Merge pull request #1 from foo"));
        // Should NOT skip a normal commit that happens to mention merge
        assert!(!should_skip("feat: handle merge conflicts"));
    }
}
