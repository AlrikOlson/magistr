use super::*;

fn render_to_json(
    f: impl FnOnce(&JsonRenderer, &mut Vec<u8>) -> std::io::Result<()>,
) -> serde_json::Value {
    let renderer = JsonRenderer::new();
    let mut buf = Vec::new();
    f(&renderer, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[test]
fn success_produces_envelope() {
    let v = render_to_json(|r, w| r.success(w, "done"));
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["message"], "done");
    assert_eq!(v["data"]["level"], "success");
}

#[test]
fn error_produces_error_envelope() {
    let v = render_to_json(|r, w| r.error(w, "failed"));
    assert_eq!(v["status"], "error");
    assert_eq!(v["error"], "failed");
}

#[test]
fn warning_produces_envelope_with_level() {
    let v = render_to_json(|r, w| r.warning(w, "watch out"));
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["level"], "warning");
    assert_eq!(v["data"]["message"], "watch out");
}

#[test]
fn info_produces_envelope_with_level() {
    let v = render_to_json(|r, w| r.info(w, "note"));
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["level"], "info");
}

#[test]
fn table_produces_array_of_objects() {
    let v = render_to_json(|r, w| {
        r.table(
            w,
            &["name".into(), "value".into()],
            &[
                vec!["lang".into(), "rust".into()],
                vec!["ed".into(), "2024".into()],
            ],
        )
    });
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_array(), "data should be array: {data}");
    assert_eq!(data[0]["name"], "lang");
    assert_eq!(data[0]["value"], "rust");
    assert_eq!(data[1]["name"], "ed");
}

#[test]
fn json_wraps_in_envelope() {
    let input = serde_json::json!({"language": "rust"});
    let v = render_to_json(|r, w| r.json(w, &input));
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["language"], "rust");
}

#[test]
fn kv_produces_object() {
    let v = render_to_json(|r, w| {
        r.kv(
            w,
            &[
                ("language".into(), "rust".into()),
                ("edition".into(), "2024".into()),
            ],
        )
    });
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["language"], "rust");
    assert_eq!(v["data"]["edition"], "2024");
}

#[test]
fn list_produces_array() {
    let v = render_to_json(|r, w| r.list(w, &["alpha".into(), "beta".into()]));
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_array(), "data should be array: {data}");
    assert_eq!(data[0], "alpha");
    assert_eq!(data[1], "beta");
}
