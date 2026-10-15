use super::*;

#[test]
fn task_lists_project_authored_completion_without_mutable_controls() {
    let doc = Document::parse("- [x] Save the draft\n- [ ] Review changes\n");
    let items = &doc.nodes[0].children;
    assert_eq!(items[0].children[0].tag, "task");
    assert_eq!(items[0].children[0].checked, Some(true));
    assert_eq!(items[1].children[0].checked, Some(false));
    assert_eq!(items[1].children[1].text, "Review changes");
}

#[test]
fn tables_preserve_headers_alignment_and_inline_content() {
    let doc = Document::parse(
        "| Name | Count | Status |\n| :--- | ---: | :---: |\n| **Drafts** | 42 | saved |\n",
    );
    let table = &doc.nodes[0];
    assert_eq!(table.tag, "table");
    let head = &table.children[0];
    assert_eq!(head.tag, "thead");
    assert_eq!(head.children[0].tag, "tr");
    assert_eq!(head.children[0].children[1].tag, "th");
    assert_eq!(head.children[0].children[1].alignment, Some("right"));
    let row = &table.children[1];
    assert_eq!(row.tag, "tr");
    assert_eq!(row.children[0].children[0].tag, "strong");
    assert_eq!(row.children[2].alignment, Some("center"));
}

#[test]
fn commonmark_decodes_text_and_keeps_semantic_reading_structure() {
    let source = "# 日本 &amp; café\r\n\r\n- **one**\n- two\n\n\x60\x60\x60rust\nlet x = \"<script>\";\n\x60\x60\x60\n\n[read](https://example.com/)";
    let doc = Document::parse(source);
    assert!(!doc.limited);
    assert_eq!(
        doc.nodes.iter().map(|node| node.tag).collect::<Vec<_>>(),
        ["h1", "ul", "pre", "p"]
    );
    let json = serde_json::to_string(&doc).unwrap();
    assert_eq!(
        doc.nodes[0]
            .children
            .iter()
            .map(|node| node.text.as_str())
            .collect::<String>(),
        "日本 & café"
    );
    assert!(json.contains("https://example.com/"));
    assert!(json.contains("strong"));
    assert_eq!(doc.nodes[2].language.as_deref(), Some("rust"));
}

#[test]
fn adjacent_parser_text_fragments_form_one_native_reading_run() {
    let doc = Document::parse(
        "[scripted-fast] echo: café &amp; 日本\ncontinued **emphasis** then [literal]",
    );
    let children = &doc.nodes[0].children;
    assert_eq!(children.len(), 3);
    assert_eq!(children[0].tag, "text");
    assert_eq!(
        children[0].text,
        "[scripted-fast] echo: café & 日本\ncontinued "
    );
    assert_eq!(children[0].start, 0);
    assert_eq!(children[1].tag, "strong");
    assert_eq!(children[2].text, " then [literal]");
}

#[test]
fn html_images_and_active_or_local_links_never_become_privileged_nodes() {
    let doc = Document::parse(
        "<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n![alt](https://tracking.example/pixel) [bad](javascript:alert%281%29) [file](file:///etc/passwd)",
    );
    let json = serde_json::to_value(doc).unwrap();
    fn inspect(node: &serde_json::Value) {
        if let Some(object) = node.as_object() {
            if let Some(tag) = object.get("tag").and_then(serde_json::Value::as_str) {
                assert!(!["script", "img", "iframe", "object", "style"].contains(&tag));
            }
            if let Some(value) = object.get("destination") {
                assert!(value.is_null());
            }
            for value in object.values() {
                inspect(value);
            }
        }
        if let Some(array) = node.as_array() {
            for value in array {
                inspect(value);
            }
        }
    }
    inspect(&json);
}

#[test]
fn pathological_structure_falls_back_to_complete_selectable_source() {
    let source = "*x* ".repeat(MAX_NODES);
    let doc = Document::parse(&source);
    assert!(doc.limited);
    assert_eq!(doc.nodes[0].tag, "pre");
    assert_eq!(doc.nodes[0].text, source);
}
