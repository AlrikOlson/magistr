use super::*;
use std::fs;

fn setup_git_project() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Initial commit
    fs::write(dir.path().join("README.md"), "# Test\n").unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    dir
}

#[test]
fn skipped_when_no_format_command() {
    let dir = setup_git_project();
    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: String::new(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(result.skipped);
    assert_eq!(
        result.skip_reason.as_deref(),
        Some("no format command configured")
    );
}

#[test]
fn skipped_when_command_is_none() {
    let dir = setup_git_project();
    let config = FormatConfig {
        extensions: vec![],
        command: "none".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(result.skipped);
}

#[test]
fn skipped_when_no_changed_files() {
    let dir = setup_git_project();
    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: "echo formatted".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(result.skipped);
    assert_eq!(result.skip_reason.as_deref(), Some("no changed files"));
}

#[test]
fn formats_changed_files_with_matching_extension() {
    let dir = setup_git_project();

    // Create a changed file with matching extension
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: "echo formatted {file}".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(!result.skipped);
    assert_eq!(result.files_formatted.len(), 1);
    assert_eq!(result.files_formatted[0], PathBuf::from("main.rs"));
    assert!(result.files_failed.is_empty());
}

#[test]
fn skips_files_with_non_matching_extension() {
    let dir = setup_git_project();

    // Create files with different extensions
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.path().join("readme.txt"), "hello\n").unwrap();

    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: "echo formatted {file}".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(!result.skipped);
    // Only the .rs file should be formatted
    assert_eq!(result.files_formatted.len(), 1);
    assert_eq!(result.files_formatted[0], PathBuf::from("main.rs"));
}

#[test]
fn reports_failed_files() {
    let dir = setup_git_project();
    fs::write(dir.path().join("test.rs"), "bad code\n").unwrap();

    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: "false".into(), // `false` always exits 1
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(!result.skipped);
    assert!(result.files_formatted.is_empty());
    assert_eq!(result.files_failed.len(), 1);
    assert_eq!(result.files_failed[0].0, PathBuf::from("test.rs"));
}

#[test]
fn env_var_skips_formatting() {
    let dir = setup_git_project();
    fs::write(dir.path().join("test.rs"), "code\n").unwrap();

    let config = FormatConfig {
        extensions: vec!["rs".into()],
        command: "echo fmt".into(),
        exclusions: vec![],
    };

    let mut formatter = Formatter::new(config, dir.path());
    formatter.no_autoformat = true;
    let result = formatter.run();

    assert!(result.skipped);
    assert_eq!(
        result.skip_reason.as_deref(),
        Some("MAGISTR_NO_AUTOFORMAT=1")
    );
}

#[test]
fn formats_all_extensions_when_empty() {
    let dir = setup_git_project();
    fs::write(dir.path().join("main.rs"), "code\n").unwrap();
    fs::write(dir.path().join("script.py"), "code\n").unwrap();

    let config = FormatConfig {
        extensions: vec![], // Empty = format everything
        command: "echo formatted {file}".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(!result.skipped);
    assert_eq!(result.files_formatted.len(), 2);
}

#[test]
fn total_processed_counts_both() {
    let result = FormatResult {
        files_formatted: vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
        files_failed: vec![(PathBuf::from("c.rs"), "error".into())],
        skipped: false,
        skip_reason: None,
    };

    assert_eq!(result.total_processed(), 3);
}

#[test]
fn skipped_result_constructor() {
    let result = FormatResult::skipped("test reason");
    assert!(result.skipped);
    assert_eq!(result.skip_reason.as_deref(), Some("test reason"));
    assert!(result.files_formatted.is_empty());
    assert!(result.files_failed.is_empty());
}

#[test]
fn matches_extension_filter() {
    let dir = tempfile::TempDir::new().unwrap();
    let config = FormatConfig {
        extensions: vec!["rs".into(), "toml".into()],
        command: "echo".into(),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());

    assert!(formatter.matches_extension(Path::new("src/main.rs")));
    assert!(formatter.matches_extension(Path::new("Cargo.toml")));
    assert!(!formatter.matches_extension(Path::new("readme.md")));
    assert!(!formatter.matches_extension(Path::new("no_extension")));
}

#[test]
fn command_with_file_placeholder() {
    let dir = setup_git_project();
    // Create a file that we can verify was touched by the formatter
    let test_file = dir.path().join("test.rs");
    fs::write(&test_file, "original\n").unwrap();

    let config = FormatConfig {
        extensions: vec!["rs".into()],
        // Use a command that writes to a marker file to prove {file} was substituted
        // Spelled with `/`: the command runs under `sh`, where a backslash
        // is an escape, not a separator.
        command: format!(
            "touch {}/format-ran",
            dir.path().to_string_lossy().replace('\\', "/")
        ),
        exclusions: vec![],
    };
    let formatter = Formatter::new(config, dir.path());
    let result = formatter.run();

    assert!(!result.skipped);
    assert_eq!(result.files_formatted.len(), 1);
    // Verify the command actually ran
    assert!(dir.path().join("format-ran").exists());
}
