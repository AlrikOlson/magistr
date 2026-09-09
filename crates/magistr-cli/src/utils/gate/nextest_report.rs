//! Where a `cargo nextest` command leaves its junit report.

/// The junit report a `cargo nextest` command writes when its profile
/// configures one (`[profile.<p>.junit] path = "junit.xml"` →
/// `target/nextest/<p>/junit.xml`), relative to the project root. `None`
/// for any other command. The file may not exist — both recording paths treat a
/// missing report as "not parsed", never as a failure.
pub(crate) fn nextest_junit_report_path(command: &str) -> Option<String> {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    let cargo = tokens
        .iter()
        .position(|t| t.rsplit('/').next() == Some("cargo"))?;
    if !tokens[cargo + 1..].contains(&"nextest") {
        return None;
    }
    let mut profile = "default";
    let mut iter = tokens.iter().peekable();
    while let Some(tok) = iter.next() {
        if *tok == "-P" || *tok == "--profile" {
            if let Some(p) = iter.next() {
                profile = p;
            }
        } else if let Some(p) = tok.strip_prefix("--profile=") {
            profile = p;
        } else if let Some(p) = tok.strip_prefix("-P")
            && !p.is_empty()
        {
            profile = p;
        }
    }
    Some(format!("target/nextest/{profile}/junit.xml"))
}
