use super::source::{check_source, check_tree};

#[test]
fn source_guard_rejects_effect_paths_and_import_aliases() {
    for source in [
        "use std::fs::read;",
        "use std::{net::TcpStream, process::Command};",
        "use ::std::{self as platform, fs::{File as Handle}};",
        "extern crate std as platform;",
        "fn read() { std::fs::read(\"file\"); }",
        "fn connect() { ::std::net::TcpStream::connect(\"host\"); }",
        "fn spawn() { std::process::Command::new(\"sh\"); }",
        "fn runtime() { std::thread::spawn(|| ()); }",
        "#[cfg(any())] fn hidden() { std::env::var(\"TOKEN\"); }",
        "fn wrapped() { assert!(std::fs::read(\"file\").is_ok()); }",
        "fn wrapped() { format!(\"{}\", std::process::id()); }",
        "fn wrapped() { vec![std::env::var(\"TOKEN\")]; }",
        "fn wrapped() { matches!(std::env::var(\"TOKEN\"), Ok(_)); }",
        "fn wrapped() { matches!(value, Some(_) if std::env::var(\"TOKEN\").is_ok()); }",
        "#[serde(default = \"std::process::id\")] struct Hidden;",
    ] {
        assert!(
            syn::parse_file(source).is_ok(),
            "invalid negative fixture: {source}"
        );
        assert!(
            !check_source(source, false).is_empty(),
            "accepted effect source: {source}"
        );
    }
}

#[test]
fn source_guard_rejects_uninspectable_and_unsafe_escapes() {
    for source in [
        "macro_rules! hidden { () => { std::fs::read(\"file\") }; }",
        "fn hidden() { include!(\"generated.rs\"); }",
        "const HIDDEN: &str = include_str!(\"secret\");",
        "const HIDDEN: &str = env!(\"TOKEN\");",
        "const HIDDEN: Option<&str> = option_env!(\"TOKEN\");",
        "fn hidden() { custom_effect!(); }",
        "fn hidden() { println!(\"effect\"); }",
        "fn hidden() { core::arch::asm!(\"nop\"); }",
        "#[derive(CustomEffect)] struct Hidden;",
        "#[custom::effect] fn hidden() {}",
        "#[cfg_attr(any(), custom::effect)] fn hidden() {}",
        "#[path = \"../../host.rs\"] mod hidden;",
        "#[path = \"/host.rs\"] mod hidden;",
        "#[doc = include_str!(\"secret\")] struct Hidden;",
        "unsafe extern \"C\" { fn effect(); }",
        "extern \"C\" fn effect() {}",
        "unsafe fn effect() {}",
        "fn effect() { unsafe { effect(); } }",
        "unsafe impl Send for Hidden {}",
        "unsafe trait Hidden {}",
        "async fn effect() {}",
        "fn effect() { async { work().await }; }",
    ] {
        assert!(
            syn::parse_file(source).is_ok(),
            "invalid negative fixture: {source}"
        );
        assert!(
            !check_source(source, false).is_empty(),
            "accepted escape: {source}"
        );
    }
}

#[test]
fn source_guard_requires_the_crate_level_compiler_boundary() {
    assert!(!check_source("struct Value;", true).is_empty());
    assert!(!check_source("#![no_std]\nstruct Value;", true).is_empty());
    assert!(!check_source("#![cfg_attr(any(), no_std)]\n#![forbid(unsafe_code)]", true).is_empty());
    assert!(
        check_source(
            "#![no_std]\n#![forbid(unsafe_code)]\nextern crate alloc;",
            true
        )
        .is_empty()
    );
}

#[test]
fn source_guard_accepts_pure_code_and_ignores_comment_and_string_words() {
    let source = r#"
        #![no_std]
        #![forbid(unsafe_code)]
        extern crate alloc;
        use alloc::{string::{String, ToString}, vec::Vec};
        use core::fmt;
        use serde::{Deserialize, Serialize};
        // std::fs::read; tokio::spawn; macro_rules! are documentation here.
        #[derive(Clone, Debug, Deserialize, Serialize)]
        #[serde(rename_all = "snake_case")]
        struct Value { text: String }
        fn pure() {
            let text = "std::process::Command::new include!(runtime)";
            let _ = format!("{text}");
            let _ = vec![1, 2];
            let _ = alloc::vec![0; 3];
            assert_eq!(text, "std::process::Command::new include!(runtime)");
            assert!(matches!(Some(1), Some(1 | 2)));
        }
        #[cfg(test)]
        #[path = "value.tests.rs"]
        mod tests;
    "#;
    assert_eq!(check_source(source, true), Vec::<String>::new());
}

#[test]
fn tree_guard_scans_tests_and_rejects_build_scripts() {
    let temp = tempfile::tempdir().expect("fixture");
    std::fs::create_dir(temp.path().join("src")).expect("source directory");
    std::fs::write(
        temp.path().join("src/lib.rs"),
        "#![no_std]\n#![forbid(unsafe_code)]",
    )
    .expect("crate root");
    std::fs::write(
        temp.path().join("src/value.tests.rs"),
        "#[test] fn effect() { std::fs::read(\"file\"); }",
    )
    .expect("test source");
    assert!(
        check_tree(temp.path())
            .iter()
            .any(|error| error.contains("value.tests.rs"))
    );
    std::fs::write(temp.path().join("build.rs"), "fn main() {}").expect("build script");
    assert!(
        check_tree(temp.path())
            .iter()
            .any(|error| error.contains("build.rs"))
    );
}
