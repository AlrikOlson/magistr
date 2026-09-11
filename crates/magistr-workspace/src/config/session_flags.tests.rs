use super::*;
use serde_json::json;

#[test]
fn flags_coerce_reference_forms_and_preserve_error_order() {
    let flags = SessionFlags::parse(&[
        "n=-20",
        "fraction=2.5",
        "redaction=false",
        "paths=[\"a/**\"]",
        "literal=1e5",
        "bad=[oops",
        "oops",
        "=value",
        "n=9000",
    ]);
    assert_eq!(flags.flags["n"], json!(9000));
    assert_eq!(flags.flags["fraction"], json!(2.5));
    assert_eq!(flags.flags["redaction"], json!(false));
    assert_eq!(flags.flags["paths"], json!(["a/**"]));
    assert_eq!(flags.flags["literal"], json!("1e5"));
    assert_eq!(flags.flags["bad"], json!("[oops"));
    assert_eq!(
        flags.errors,
        [
            "--config expects key=value, got 'oops'",
            "--config expects key=value, got '=value'"
        ]
    );
}
