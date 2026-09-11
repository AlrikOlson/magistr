use std::{collections::BTreeSet, sync::LazyLock};

use regex::Regex;

/// Remaining absolute paths (POSIX or drive-letter, raw or JSON-escaped):
/// keep only the basename behind a placeholder. Anchored to a boundary so a
/// mid-word slash (`.workbench/index.db`) is never mistaken for a path root.
static ABSOLUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)(^|[\s"'`(=\[])((?:[A-Za-z]:(?:\\\\|\\|/)|/)(?:[^\s"'`|<>]*(?:\\\\|\\|/))*[^\s"'`|<>\\/]+)"#,
    )
    .expect("fixed bundle redaction pattern compiles")
});

/// The bundle redaction choke point, applied to every member's text: the
/// workspace root (native, forward-slash and JSON-escaped spellings) becomes
/// `<workspace>`, and any remaining absolute path is reduced to an
/// `<outside-workspace>/basename` placeholder. Credential redaction is a
/// separate pass on top of this one.
#[must_use]
pub fn redact_bundle_text(text: &str, workspace_root: &str) -> String {
    let mut out = text.to_owned();
    let spellings: BTreeSet<String> = [
        workspace_root.to_owned(),
        workspace_root.replace('\\', "/"),
        workspace_root.replace('/', "\\"),
        // Inside JSON members backslashes are escaped.
        workspace_root.replace('\\', "\\\\"),
        workspace_root.replace('/', "\\\\"),
    ]
    .into_iter()
    .filter(|spelling| !spelling.is_empty())
    .collect();
    for spelling in &spellings {
        out = out.replace(spelling, "<workspace>");
    }
    ABSOLUTE
        .replace_all(&out, |captures: &regex::Captures<'_>| {
            let full = &captures[2];
            if full.contains("<workspace>") {
                return captures[0].to_owned();
            }
            let base = full.rsplit(['\\', '/']).next().unwrap_or_default();
            format!("{}<outside-workspace>/{base}", &captures[1])
        })
        .into_owned()
}
