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
        if parsed.description.len() > 100 {
            violations.push(Violation::err(
                "length",
                format!(
                    "description is {} chars (max 100)",
                    parsed.description.len()
                ),
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
}
