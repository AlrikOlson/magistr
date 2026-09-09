use super::*;

fn render_to_string(
    f: impl FnOnce(&SilentRenderer, &mut Vec<u8>) -> std::io::Result<()>,
) -> String {
    let renderer = SilentRenderer::new();
    let mut buf = Vec::new();
    f(&renderer, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn success_produces_no_output() {
    let output = render_to_string(|r, w| r.success(w, "done"));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn error_produces_no_output() {
    let output = render_to_string(|r, w| r.error(w, "failed"));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn warning_produces_no_output() {
    let output = render_to_string(|r, w| r.warning(w, "watch out"));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn info_produces_no_output() {
    let output = render_to_string(|r, w| r.info(w, "note"));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn table_produces_no_output() {
    let output = render_to_string(|r, w| r.table(w, &["Name".into()], &[vec!["value".into()]]));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn json_produces_no_output() {
    let data = serde_json::json!({"key": "value"});
    let output = render_to_string(|r, w| r.json(w, &data));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn kv_produces_no_output() {
    let output = render_to_string(|r, w| r.kv(w, &[("key".into(), "value".into())]));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn list_produces_no_output() {
    let output = render_to_string(|r, w| r.list(w, &["alpha".into(), "beta".into()]));
    assert!(output.is_empty(), "expected empty, got: {output}");
}

#[test]
fn render_command_result_produces_no_output() {
    use super::super::CommandResult;
    let renderer = SilentRenderer::new();
    let mut buf = Vec::new();

    let results = vec![
        CommandResult::Success("done".into()),
        CommandResult::Error("fail".into()),
        CommandResult::Warning("warn".into()),
        CommandResult::Info("info".into()),
        CommandResult::Empty,
        CommandResult::Json(serde_json::json!({})),
        CommandResult::KeyValues(vec![]),
        CommandResult::Table {
            headers: vec![],
            rows: vec![],
        },
        CommandResult::List(vec!["item".into()]),
    ];

    for result in &results {
        renderer.render(&mut buf, result).unwrap();
    }

    assert!(buf.is_empty(), "SilentRenderer should produce no output");
}
