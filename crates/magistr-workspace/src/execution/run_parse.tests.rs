use super::*;

#[test]
fn program_basename_strips_paths_and_exe_suffixes() {
    assert_eq!(program_basename("C:\\tools\\grep.exe"), "grep");
    assert_eq!(program_basename("/usr/bin/sed"), "sed");
    assert_eq!(program_basename("Cargo"), "cargo");
}

#[test]
fn tokens_claim_by_basename() {
    assert!(has_token("npx vitest run", "vitest"));
    assert!(has_token("/usr/local/bin/cargo +nightly test", "cargo"));
    assert!(!has_token("bun test", "cat"));
}

#[test]
fn trim_trace_drops_blank_padding_and_caps_lines() {
    let lines: Vec<String> = (0..12)
        .map(|index| {
            if index % 3 == 0 {
                "   ".into()
            } else {
                format!("line {index}  ")
            }
        })
        .collect();
    let trimmed = trim_trace(&lines);
    assert_eq!(trimmed.lines().count(), 8);
    assert!(trimmed.starts_with("line 1\nline 2\nline 4"));
    assert!(!trimmed.contains("  \n"));
}

#[test]
fn split_lines_normalizes_windows_endings() {
    assert_eq!(split_lines("a\r\nb\nc"), vec!["a", "b", "c"]);
    assert_eq!(normalize_path("src\\main.rs"), "src/main.rs");
    assert_eq!(
        collapse_whitespace(" Tests  1 failed | 3 passed "),
        " Tests 1 failed | 3 passed "
    );
}
