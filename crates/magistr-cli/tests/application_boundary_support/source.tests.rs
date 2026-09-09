fn check_source(text: &str, crate_root: bool) -> Vec<String> {
    super::source::check_source_with_async(text, crate_root, true)
}

#[test]
fn source_guard_rejects_adapter_effect_paths() {
    for source in [
        "#[cfg(any())] fn hidden() { tokio::spawn(work()); }",
        "use magistr_core::Runtime;",
        "use magistr_engine::{self as native, NativeEngines};",
        "use ::rmcp::{self as protocol, ServerHandler};",
        "use magistr_protocol as wire;",
        "fn render() { ratatui::Terminal::new(backend); }",
        "fn wrapped() { assert!(reqwest::get(url).is_ok()); }",
        "async fn hidden() { async { tokio::spawn(work()); }.await; }",
        "async fn hidden() { reqwest::get(url).await; }",
        "fn pinned() { core::pin::pin!(std::fs::read(path)); }",
        "fn pinned() { pin!(async { tokio::spawn(work()); }); }",
    ] {
        assert!(syn::parse_file(source).is_ok(), "invalid fixture: {source}");
        assert!(
            !check_source(source, false).is_empty(),
            "accepted adapter effect: {source}"
        );
    }
}

#[test]
fn domain_mode_still_rejects_async_in_sources_and_trees() {
    for source in [
        "async fn effect() {}",
        "fn pinned() { core::pin::pin!(work()); }",
    ] {
        assert!(!super::source::check_source(source, false).is_empty());
    }
    let temp = tempfile::tempdir().expect("fixture");
    std::fs::create_dir(temp.path().join("src")).expect("source directory");
    std::fs::write(
        temp.path().join("src/lib.rs"),
        "#![no_std]\n#![forbid(unsafe_code)]\nasync fn read() {}",
    )
    .expect("crate root");
    assert!(!super::source::check_tree(temp.path()).is_empty());
    assert!(super::source::check_tree_with_async(temp.path(), true).is_empty());
}

#[test]
fn source_guard_accepts_runtime_independent_futures_and_fake_ports() {
    let source = r#"
        #![no_std]
        #![forbid(unsafe_code)]
        extern crate alloc;
        use alloc::boxed::Box;
        use core::{future::Future, pin::Pin};
        use magistr_domain::ProjectId;
        type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
        trait Store { fn read(&self) -> PortFuture<'_, ProjectId>; }
        async fn read(store: &impl Store) -> ProjectId { store.read().await }
        fn exercise(store: &impl Store) {
            let _future = core::pin::pin!(async { read(store).await });
            let _documentation = "tokio::spawn std::fs::read";
        }
    "#;
    assert_eq!(check_source(source, true), Vec::<String>::new());
}

#[test]
fn source_guard_rejects_unsafe_macros_and_missing_compiler_boundary() {
    for source in [
        "use std::{self as host, fs::File};",
        "extern crate std as host;",
        "unsafe fn hidden() {}",
        "unsafe impl Send for Hidden {}",
        "unsafe extern \"C\" { fn hidden(); }",
        "fn hidden() { custom_effect!(); }",
        "macro_rules! hidden { () => { tokio::spawn(work()) }; }",
        "const SECRET: &str = include_str!(\"secret\");",
        "#[cfg_attr(any(), tokio::main)] async fn hidden() {}",
        "#[path = \"../../adapter.rs\"] mod hidden;",
    ] {
        assert!(syn::parse_file(source).is_ok(), "invalid fixture: {source}");
        assert!(
            !check_source(source, false).is_empty(),
            "accepted escape: {source}"
        );
    }
    for source in [
        "struct Value;",
        "#![no_std]\nstruct Value;",
        "#![cfg_attr(any(), no_std)]\n#![forbid(unsafe_code)]",
    ] {
        assert!(!check_source(source, true).is_empty());
    }
}

#[test]
fn tree_guard_scans_unreferenced_test_sources_and_rejects_build_scripts() {
    let temp = tempfile::tempdir().expect("fixture");
    std::fs::create_dir(temp.path().join("src")).expect("source directory");
    std::fs::write(
        temp.path().join("src/lib.rs"),
        "#![no_std]\n#![forbid(unsafe_code)]",
    )
    .expect("crate root");
    std::fs::write(
        temp.path().join("src/hidden.tests.rs"),
        "#[cfg(any())] async fn hidden() { tokio::spawn(work()); }",
    )
    .expect("unreferenced source");
    assert!(
        super::source::check_tree_with_async(temp.path(), true)
            .iter()
            .any(|error| error.contains("hidden.tests.rs"))
    );
    std::fs::write(temp.path().join("build.rs"), "fn main() {}").expect("build script");
    assert!(
        super::source::check_tree_with_async(temp.path(), true)
            .iter()
            .any(|error| error.contains("build.rs"))
    );
}
