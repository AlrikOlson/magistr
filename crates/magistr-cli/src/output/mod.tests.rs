use super::*;

#[test]
fn create_renderer_json_flag() {
    let r = create_renderer(true, false);
    // Verify it behaves as JSON renderer
    let mut buf = Vec::new();
    r.success(&mut buf, "test").unwrap();
    let s = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["status"], "ok");
}

#[test]
fn create_renderer_silent_flag() {
    let r = create_renderer(false, true);
    let mut buf = Vec::new();
    r.success(&mut buf, "test").unwrap();
    assert!(buf.is_empty(), "silent should produce no output");
}

#[test]
fn create_renderer_default_is_plain() {
    let r = create_renderer(false, false);
    let mut buf = Vec::new();
    r.success(&mut buf, "test").unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("test"), "plain should contain the message");
    // Plain output is NOT valid JSON
    assert!(serde_json::from_str::<serde_json::Value>(&s).is_err());
}

#[test]
fn json_takes_priority_over_silent() {
    let r = create_renderer(true, true);
    let mut buf = Vec::new();
    r.success(&mut buf, "test").unwrap();
    let s = String::from_utf8(buf).unwrap();
    // Should be JSON, not silent
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["status"], "ok");
}

#[test]
fn renderer_is_object_safe() {
    // Verify the trait can be used as a trait object
    fn accepts_renderer(_r: &dyn OutputRenderer) {}
    let plain = PlainRenderer::new();
    let json = JsonRenderer::new();
    let silent = SilentRenderer::new();
    accepts_renderer(&plain);
    accepts_renderer(&json);
    accepts_renderer(&silent);
}

#[test]
fn command_result_renders_through_plain() {
    let renderer = PlainRenderer::new();
    let mut buf = Vec::new();
    let result = CommandResult::Success("all done".into());
    renderer.render(&mut buf, &result).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("all done"));
}

#[test]
fn command_result_renders_through_json() {
    let renderer = JsonRenderer::new();
    let mut buf = Vec::new();
    let result = CommandResult::KeyValues(vec![("lang".into(), "rust".into())]);
    renderer.render(&mut buf, &result).unwrap();
    let s = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["data"]["lang"], "rust");
}

#[test]
fn command_result_list_renders_through_plain() {
    let renderer = PlainRenderer::new();
    let mut buf = Vec::new();
    let result = CommandResult::List(vec!["alpha".into(), "beta".into()]);
    renderer.render(&mut buf, &result).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("alpha"), "got: {s}");
    assert!(s.contains("beta"), "got: {s}");
}

#[test]
fn command_result_list_renders_through_json() {
    let renderer = JsonRenderer::new();
    let mut buf = Vec::new();
    let result = CommandResult::List(vec!["alpha".into(), "beta".into()]);
    renderer.render(&mut buf, &result).unwrap();
    let s = String::from_utf8(buf).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"].is_array());
    assert_eq!(v["data"][0], "alpha");
}

#[test]
fn command_result_empty_renders_nothing_through_all() {
    let renderers: Vec<Box<dyn OutputRenderer>> = vec![
        Box::new(PlainRenderer::new()),
        Box::new(JsonRenderer::new()),
        Box::new(SilentRenderer::new()),
    ];
    for r in &renderers {
        let mut buf = Vec::new();
        r.render(&mut buf, &CommandResult::Empty).unwrap();
        assert!(buf.is_empty(), "Empty should produce no output");
    }
}
