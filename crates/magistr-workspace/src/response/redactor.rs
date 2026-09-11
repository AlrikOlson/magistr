use super::RedactedText;
use regex::{Captures, Regex};
use std::{collections::HashMap, sync::LazyLock};

static PATTERNS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    [
        ("aws-key", r"(?-u:\b)(?:AKIA|ASIA)[0-9A-Z]{16}(?-u:\b)"),
        (
            "github-token",
            r"(?-u:\b)(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{22,255})(?-u:\b)",
        ),
        (
            "slack-token",
            r"(?-u:\b)xox[baprs]-[A-Za-z0-9-]{10,250}(?-u:\b)",
        ),
        (
            "anthropic-key",
            r"(?-u:\b)sk-ant-[A-Za-z0-9_-]{20,}(?-u:\b)",
        ),
        ("openai-key", r"(?-u:\b)sk-[A-Za-z0-9_-]{20,}(?-u:\b)"),
        ("google-api-key", r"(?-u:\b)AIza[0-9A-Za-z_-]{35}(?-u:\b)"),
        (
            "private-key",
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)",
        ),
        (
            "jwt",
            r"(?-u:\b)eyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}(?-u:\b)",
        ),
    ]
    .into_iter()
    .map(|(kind, pattern)| (kind, compile(pattern)))
    .collect()
});
static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    // ECMAScript whitespace differs from Unicode White_Space: FEFF is in,
    // U+0085 is out. JS /i without /u also folds ASCII credential names only.
    let whitespace =
        r"\t-\r \u{00A0}\u{1680}\u{2000}-\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}";
    compile(&format!(
        r#"(?-u:\b)((?i-u:api[_-]?key|access[_-]?key|secret(?:[_-]?key)?|token|password|passwd))([{whitespace}]*[:=][{whitespace}]*["']?)([^{whitespace}"']{{8,}})(["']?)"#
    ))
});
static MARKER: LazyLock<Regex> = LazyLock::new(|| compile(r"\[REDACTED:[a-z-]+\]"));
static ENTROPY: LazyLock<Regex> = LazyLock::new(|| compile(r"[A-Za-z0-9+=_-]{40,}"));

fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("fixed response redaction patterns compile")
}

/// The pinned ordered credential detectors; used only as responses leave the service.
pub struct Redactor;

impl Redactor {
    #[must_use]
    pub fn redact(text: &str) -> RedactedText {
        let mut out = text.to_owned();
        let mut kinds = Vec::new();
        for (kind, pattern) in PATTERNS.iter() {
            out = pattern
                .replace_all(&out, |_: &Captures<'_>| {
                    kinds.push(*kind);
                    format!("[REDACTED:{kind}]")
                })
                .into_owned();
        }
        out = ASSIGNMENT
            .replace_all(&out, |matched: &Captures<'_>| {
                if MARKER.is_match(&matched[0]) {
                    return matched[0].to_owned();
                }
                kinds.push("credential-assignment");
                format!(
                    "{}{}[REDACTED:credential-assignment]{}",
                    &matched[1], &matched[2], &matched[4]
                )
            })
            .into_owned();
        out = ENTROPY
            .replace_all(&out, |matched: &Captures<'_>| {
                let candidate = &matched[0];
                if candidate.bytes().all(|byte| byte.is_ascii_hexdigit())
                    || Self::entropy(candidate) < 4.5
                {
                    return candidate.to_owned();
                }
                kinds.push("high-entropy");
                "[REDACTED:high-entropy]".to_owned()
            })
            .into_owned();
        RedactedText { text: out, kinds }
    }

    /// Shannon entropy with the reference's Unicode iteration and UTF-16 length.
    #[must_use]
    pub fn entropy(text: &str) -> f64 {
        // Preserve first-seen order so floating-point sums are repeatable.
        let mut positions = HashMap::new();
        let mut counts: Vec<usize> = Vec::new();
        for character in text.chars() {
            let next = counts.len();
            let position = *positions.entry(character).or_insert_with(|| {
                counts.push(0);
                next
            });
            counts[position] += 1;
        }
        let length = text.encode_utf16().count() as f64;
        counts
            .iter()
            .map(|count| {
                let probability = *count as f64 / length;
                -probability * probability.log2()
            })
            .sum()
    }
}
