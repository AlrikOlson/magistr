use super::*;

#[test]
fn glob_root_braces_and_nested_paths_match_retained_scope() {
    let matcher = SearchScope::glob("**/*.{rs,ts}").expect("valid glob");
    assert!(matcher.is_match("lib.rs"));
    assert!(matcher.is_match("src/nested/main.ts"));
    assert!(!matcher.is_match("src/main.py"));
    assert!(
        !SearchScope::glob("src/*.rs")
            .expect("valid glob")
            .is_match("src/nested/lib.rs")
    );
    assert!(SearchScope::glob("[").is_err());
}

#[test]
fn rust_nonshipping_paths_and_language_test_names_are_demoted() {
    for path in [
        "tests/a.rs",
        "x/benches/a.rs",
        "examples/a.rs",
        "a.test.ts",
        "a.spec.js",
        "a_test.go",
    ] {
        assert!(SearchScope::is_test(path), "{path}");
    }
    for path in ["src/test.rs", "testing/a.ts", "src/lib.rs"] {
        assert!(!SearchScope::is_test(path), "{path}");
    }
}
