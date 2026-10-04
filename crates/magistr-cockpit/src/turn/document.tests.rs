use super::*;

#[test]
fn compact_preview_keeps_spans_and_signals_full_content() {
    let doc = TextDocument::markdown("**Saved** result\nMore findings", ThemeMode::Dark);
    let row = doc.summary(20.0, &mut |text, _| text.chars().count() as f32);
    assert_eq!(row.text, "Saved result…");
    assert!(row.spans.iter().any(|span| span.style == SpanStyle::Bold));
    assert_eq!(doc.source(), "**Saved** result\nMore findings");
}

#[test]
fn incomplete_syntax_and_indented_code_remain_copyable() {
    for source in [
        "# Finding\n**unfinished",
        "```rust\n    let result = 7;",
        "[unfinished](https://example.com",
    ] {
        let doc = TextDocument::markdown(source, ThemeMode::Light);
        assert_eq!(doc.source(), source);
        assert!(
            !doc.rows(10.0, &mut |text, _| text.chars().count() as f32)
                .is_empty()
        );
    }
}

#[test]
fn markdown_keeps_source_and_styles_and_maps_wrapped_text_to_source() {
    let source = "# Findings\n\n**Keep** the `receipt` and [docs](https://example.com).\n> A quoted result\n1. A numbered requirement that wraps across several rows\n- [x] An actual checklist value\n\n```rust\n    let receipt = 7;\n```\n```diff\n-old\n+new\n```";
    let doc = TextDocument::markdown(source, ThemeMode::Dark);
    assert_eq!(doc.source(), source);
    let wide = doc.rows(100.0, &mut |s, _| s.chars().count() as f32);
    let narrow = doc.rows(22.0, &mut |s, _| s.chars().count() as f32);
    assert!(narrow.len() > wide.len());
    assert!(wide.iter().any(|r| r.row.kind == RowKind::Heading));
    assert!(
        wide.iter()
            .any(|r| r.row.spans.iter().any(|s| s.style == SpanStyle::Bold))
    );
    assert!(
        wide.iter()
            .any(|r| r.row.kind == RowKind::DiffAdd && r.row.text == "+new")
    );
    assert!(
        wide.iter()
            .any(|r| r.row.kind == RowKind::Code && r.row.text.starts_with("    let"))
    );
    let held = narrow
        .iter()
        .find(|r| r.row.text.contains("numbered"))
        .unwrap()
        .anchor;
    let at = TextDocument::row_of(&wide, held);
    assert_eq!(wide[at].anchor.line, held.line);
    assert!(doc.blocks().iter().any(|b| b.heading == 1));
}

#[test]
fn narrow_tables_name_every_cell_and_keep_marked_content() {
    let doc = TextDocument::markdown(
        "| Check | Result |\n| --- | --- |\n| Recovery | **Passed** |\n| Restart | Needs work |",
        ThemeMode::Light,
    );
    let rows = doc.rows(18.0, &mut |s, _| s.chars().count() as f32);
    let text = rows
        .iter()
        .map(|r| r.row.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for value in [
        "Check:",
        "Result:",
        "Recovery",
        "Passed",
        "Restart",
        "Needs work",
    ] {
        assert!(text.contains(value), "{text}");
    }
    assert!(
        rows.iter()
            .any(|r| r.row.spans.iter().any(|s| s.style == SpanStyle::Bold))
    );
    assert_eq!(doc.source().lines().count(), 4);
}

#[test]
fn raw_text_is_never_reinterpreted_as_markdown_or_discarded() {
    let raw = "  **literal**\n\n\t[bytes](command)\n";
    let doc = TextDocument::code(raw, "", ThemeMode::Dark);
    assert_eq!(doc.source(), raw);
    let rows = doc.rows(10.0, &mut |s, _| s.chars().count() as f32);
    assert!(rows.iter().any(|r| r.row.text.contains("**")));
    assert!(rows.iter().any(|r| r.row.text.is_empty()));
}

#[test]
fn table_columns_share_widths_and_headers_have_no_overlapping_spans() {
    let doc = TextDocument::markdown(
        "| **Item** | Result |\n| --- | --- |\n| A | Good |\n| Long item | Good |",
        ThemeMode::Dark,
    );
    let rows = doc.rows(80.0, &mut |s, _| s.chars().count() as f32);
    let data = rows
        .iter()
        .filter(|r| r.row.text.contains("Good"))
        .collect::<Vec<_>>();
    assert_eq!(data[0].row.text.find('|'), data[1].row.text.find('|'));
    for row in rows {
        for pair in row.row.spans.windows(2) {
            assert!(pair[0].end <= pair[1].start, "{:?}", row.row.spans);
        }
    }
}
