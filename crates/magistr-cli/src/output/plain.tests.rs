use super::*;

fn render_to_string(f: impl FnOnce(&PlainRenderer, &mut Vec<u8>) -> std::io::Result<()>) -> String {
    let renderer = PlainRenderer::new();
    let mut buf = Vec::new();
    f(&renderer, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn success_contains_message() {
    let output = render_to_string(|r, w| r.success(w, "all good"));
    assert!(output.contains("all good"), "got: {output}");
}

#[test]
fn error_contains_message() {
    let output = render_to_string(|r, w| r.error(w, "something broke"));
    assert!(output.contains("something broke"), "got: {output}");
}

#[test]
fn warning_contains_message() {
    let output = render_to_string(|r, w| r.warning(w, "heads up"));
    assert!(output.contains("heads up"), "got: {output}");
}

#[test]
fn info_contains_message() {
    let output = render_to_string(|r, w| r.info(w, "fyi"));
    assert!(output.contains("fyi"), "got: {output}");
}

#[test]
fn table_renders_headers_and_rows() {
    let output = render_to_string(|r, w| {
        r.table(
            w,
            &["Name".into(), "Value".into()],
            &[
                vec!["language".into(), "rust".into()],
                vec!["edition".into(), "2024".into()],
            ],
        )
    });
    assert!(output.contains("Name"), "missing header: {output}");
    assert!(output.contains("language"), "missing row data: {output}");
    assert!(output.contains("rust"), "missing row data: {output}");
    assert!(output.contains("2024"), "missing row data: {output}");
}

#[test]
fn table_empty_headers_produces_nothing() {
    let output = render_to_string(|r, w| r.table(w, &[], &[]));
    assert!(output.is_empty(), "expected empty output, got: {output}");
}

#[test]
fn json_pretty_prints() {
    let data = serde_json::json!({"language": "rust"});
    let output = render_to_string(|r, w| r.json(w, &data));
    assert!(output.contains("language"), "got: {output}");
    assert!(output.contains("rust"), "got: {output}");
    // Pretty-printed means it should have newlines
    assert!(output.contains('\n'), "expected pretty-printed JSON");
}

#[test]
fn kv_renders_pairs() {
    let output = render_to_string(|r, w| {
        r.kv(
            w,
            &[
                ("language".into(), "rust".into()),
                ("edition".into(), "2024".into()),
            ],
        )
    });
    assert!(output.contains("language"), "got: {output}");
    assert!(output.contains("rust"), "got: {output}");
    assert!(output.contains("edition"), "got: {output}");
}

#[test]
fn list_renders_items() {
    let output =
        render_to_string(|r, w| r.list(w, &["alpha".into(), "beta".into(), "gamma".into()]));
    assert!(output.contains("alpha"), "got: {output}");
    assert!(output.contains("beta"), "got: {output}");
    assert!(output.contains("gamma"), "got: {output}");
}

#[test]
fn list_empty_produces_nothing() {
    let output = render_to_string(|r, w| r.list(w, &[]));
    assert!(output.is_empty(), "expected empty output, got: {output}");
}
