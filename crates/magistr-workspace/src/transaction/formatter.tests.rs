use super::*;

fn fixture() -> tempfile::TempDir {
    tempfile::tempdir().expect("temporary workspace")
}

#[cfg(unix)]
fn script(root: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).expect("executable");
    path
}

#[tokio::test]
async fn empty_documents_stay_empty_without_starting_a_formatter() {
    let root = fixture();
    let formatter = NativeFormatter::new(
        root.path().into(),
        FormatterOptions {
            rustfmt: Some(root.path().join("missing")),
            biome: Some(root.path().join("missing")),
            ..Default::default()
        },
    )
    .unwrap();
    for path in ["new.rs", "new.ts", "new.json", "new.md"] {
        assert_eq!(formatter.format(path, "").await.unwrap(), "");
    }
    assert!(formatter.format("../escape.rs", "").await.is_err());
}

#[tokio::test]
async fn unsupported_files_do_not_start_a_formatter() {
    let root = fixture();
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            rustfmt: Some(root.path().join("missing")),
            biome: Some(root.path().join("missing")),
            ..Default::default()
        },
    )
    .expect("formatter");
    assert_eq!(
        formatter
            .format("notes.md", "raw text")
            .await
            .expect("passthrough"),
        "raw text"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn rustfmt_receives_nearest_edition_and_source_parent_directory() {
    let root = fixture();
    let parent = root.path().join("crate/src");
    std::fs::create_dir_all(&parent).expect("source directory");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[workspace.package]\nedition='2021'\n",
    )
    .expect("workspace manifest");
    std::fs::write(
        root.path().join("crate/Cargo.toml"),
        "[package]\nname='test'\nedition='2024'\n",
    )
    .expect("package manifest");
    let binary = script(
        root.path(),
        "fake-rustfmt",
        "printf '%s\\n' \"$PWD\" \"$@\"\ncat",
    );
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            rustfmt: Some(binary),
            ..Default::default()
        },
    )
    .expect("formatter");
    let result = formatter
        .format("crate/src/lib.rs", "fn input() {}")
        .await
        .expect("format");
    assert!(result.starts_with(&format!(
        "{}\n",
        parent.canonicalize().expect("parent").display()
    )));
    assert!(result.contains("--edition\n2024\n"), "{result}");
    assert!(result.ends_with("fn input() {}"));
}

#[test]
fn inherited_edition_and_reference_default_are_preserved() {
    let root = fixture();
    std::fs::create_dir_all(root.path().join("member/src")).expect("source directory");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[workspace.package]\nedition='2024'\n",
    )
    .expect("workspace manifest");
    std::fs::write(
        root.path().join("member/Cargo.toml"),
        "[package]\nname='member'\nedition.workspace=true\n",
    )
    .expect("package manifest");
    assert_eq!(
        edition_for(
            &root.path().canonicalize().expect("root"),
            &root
                .path()
                .join("member/src")
                .canonicalize()
                .expect("parent")
        )
        .expect("edition"),
        "2024"
    );
    let empty = fixture();
    assert_eq!(
        edition_for(empty.path(), empty.path()).expect("default edition"),
        "2021"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn biome_routes_all_supported_extensions_and_uses_workspace_cwd() {
    let root = fixture();
    let binary = script(
        root.path(),
        "fake-biome",
        "printf '%s\\n' \"$PWD\" \"$@\"\ncat",
    );
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            biome: Some(binary),
            ..Default::default()
        },
    )
    .expect("formatter");
    for extension in [
        "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "json", "jsonc",
    ] {
        let path = format!("new/nested/file.{extension}");
        let result = formatter.format(&path, "input").await.expect("format");
        let expected = format!(
            "{}\nformat\n--stdin-file-path={path}\ninput",
            root.path().canonicalize().expect("root").display()
        );
        assert_eq!(result, expected);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn failed_formatter_returns_bounded_diagnostics() {
    let root = fixture();
    let binary = script(
        root.path(),
        "failure",
        "cat >/dev/null\nprintf 'invalid source' >&2\nexit 7",
    );
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            biome: Some(binary),
            ..Default::default()
        },
    )
    .expect("formatter");
    let error = formatter
        .format("file.ts", "input")
        .await
        .expect_err("failed formatter");
    assert!(
        error.contains("7") && error.contains("invalid source"),
        "{error}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn output_limits_reject_stdout_and_stderr_without_waiting_for_eof() {
    for redirection in ["", " >&2"] {
        let root = fixture();
        let binary = script(
            root.path(),
            "flood",
            &format!("while :; do printf '12345678901234567890'{redirection}; done"),
        );
        let formatter = NativeFormatter::new(
            root.path().to_path_buf(),
            FormatterOptions {
                biome: Some(binary),
                max_output_bytes: 32,
                timeout: std::time::Duration::from_secs(2),
                ..Default::default()
            },
        )
        .expect("formatter");
        let started = std::time::Instant::now();
        let error = formatter
            .format("file.ts", "input")
            .await
            .expect_err("output cap");
        assert!(error.contains("output limit"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_kills_and_reaps_formatter_even_when_it_does_not_read_stdin() {
    let root = fixture();
    let binary = script(
        root.path(),
        "hang",
        "printf '%s' \"$$\" > child.pid\nwhile :; do :; done",
    );
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            biome: Some(binary),
            timeout: std::time::Duration::from_millis(100),
            ..Default::default()
        },
    )
    .expect("formatter");
    let error = formatter
        .format("file.ts", &"x".repeat(128 * 1024))
        .await
        .expect_err("timeout");
    assert!(error.contains("timed out"), "{error}");
    let pid: i32 = std::fs::read_to_string(root.path().join("child.pid"))
        .expect("child pid")
        .parse()
        .expect("pid");
    // SAFETY: signal zero probes existence of the recorded child without delivering a signal.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "formatter must be reaped"
    );
}

#[tokio::test]
async fn invalid_paths_and_oversized_input_are_rejected() {
    let root = fixture();
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            max_output_bytes: 4,
            ..Default::default()
        },
    )
    .expect("formatter");
    assert!(
        formatter
            .format("../outside.rs", "x")
            .await
            .expect_err("path")
            .contains("relative")
    );
    assert!(
        formatter
            .format("inside.rs", "12345")
            .await
            .expect_err("input")
            .contains("input limit")
    );
}

#[test]
fn toolchain_pin_detection_includes_parent_directories() {
    let root = fixture();
    std::fs::create_dir_all(root.path().join("nested")).expect("nested");
    assert!(!has_toolchain_file(root.path()));
    for filename in ["rust-toolchain.toml", "rust-toolchain"] {
        std::fs::write(root.path().join(filename), "missing-toolchain").expect("toolchain pin");
        assert!(has_toolchain_file(&root.path().join("nested")));
        std::fs::remove_file(root.path().join(filename)).expect("remove test pin");
    }
}

#[tokio::test]
async fn unavailable_pinned_toolchain_cannot_fall_back_to_path_rustfmt() {
    let root = fixture();
    std::fs::write(
        root.path().join("rust-toolchain.toml"),
        "[toolchain]\nchannel='magistr-formatter-test-unavailable-toolchain'\n",
    )
    .expect("missing toolchain pin");
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            timeout: std::time::Duration::from_secs(2),
            ..Default::default()
        },
    )
    .expect("formatter");
    let error = formatter
        .format("file.rs", "fn main() {}")
        .await
        .expect_err("missing pinned formatter");
    assert!(error.contains("pinned workspace toolchain"), "{error}");
}

#[cfg(unix)]
#[tokio::test]
async fn successful_formatter_output_must_be_utf8() {
    let root = fixture();
    let binary = script(
        root.path(),
        "invalid-utf8",
        "cat >/dev/null\nprintf '\\377'",
    );
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            biome: Some(binary),
            ..Default::default()
        },
    )
    .expect("formatter");
    let error = formatter
        .format("file.ts", "input")
        .await
        .expect_err("invalid UTF-8");
    assert!(error.contains("not UTF-8"), "{error}");
}

#[cfg(unix)]
#[tokio::test]
async fn formatter_cannot_use_an_escaping_symlink_parent() {
    let root = fixture();
    let outside = fixture();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).expect("symlink");
    let formatter = NativeFormatter::new(root.path().to_path_buf(), FormatterOptions::default())
        .expect("formatter");
    let error = formatter
        .format("escape/file.ts", "input")
        .await
        .expect_err("symlink escape");
    assert!(error.contains("outside the workspace"), "{error}");
}
