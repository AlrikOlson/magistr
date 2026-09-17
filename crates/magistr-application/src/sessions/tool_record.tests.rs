use alloc::{borrow::ToOwned, format, vec};

use super::*;

fn record() -> ToolRecord {
    ToolRecord {
        kind: Some(ToolVerb::Read),
        input: Some(ToolPayload::Json(r#"{"file_path":"src/main.rs"}"#.into())),
        output: Some(ToolPayload::Text("fn main() {}".into())),
        text: vec!["fn main() {}".to_owned()],
        locations: vec![(String::from("src/main.rs"), Some(1))],
        terminal: None,
        truncated_from: None,
    }
}

#[test]
fn an_empty_record_is_empty_and_a_kind_alone_is_not() {
    assert!(ToolRecord::default().is_empty());
    assert!(
        !ToolRecord {
            kind: Some(ToolVerb::Run),
            ..ToolRecord::default()
        }
        .is_empty()
    );
    assert!(!record().is_empty());
}

#[test]
fn a_record_inside_the_cap_is_untouched_and_measures_its_bytes() {
    let record = record();
    let size = record.size();
    assert_eq!(size, r#"{"file_path":"src/main.rs"}"#.len() + 12 + 12);
    assert_eq!(record.clone().bounded(), record);
}

#[test]
fn an_oversize_string_output_is_cut_at_the_cap_and_remembers_its_size() {
    let output = "x".repeat(ToolRecord::CAP + 100);
    let record = ToolRecord {
        output: Some(ToolPayload::Text(output)),
        ..ToolRecord::default()
    }
    .bounded();
    assert_eq!(record.truncated_from, Some(ToolRecord::CAP + 100));
    assert_eq!(
        record
            .output
            .as_ref()
            .and_then(|v| v.as_str())
            .map(str::len),
        Some(ToolRecord::CAP)
    );
    assert_eq!(record.size(), ToolRecord::CAP);
}

#[test]
fn an_oversize_object_becomes_the_head_of_its_json() {
    let record = ToolRecord {
        input: Some(ToolPayload::Json(format!(
            r#"{{"content":"{}"}}"#,
            "y".repeat(ToolRecord::CAP)
        ))),
        ..ToolRecord::default()
    }
    .bounded();
    let input = record
        .input
        .as_ref()
        .and_then(|v| v.as_str())
        .expect("a string now");
    assert!(input.starts_with(r#"{"content":"yyy"#), "{}", &input[..20]);
    assert_eq!(input.len(), ToolRecord::CAP);
    assert!(record.truncated_from.expect("noted") > ToolRecord::CAP);
}

#[test]
fn the_input_takes_the_budget_first_and_the_text_is_cut_on_a_char_boundary() {
    let record = ToolRecord {
        input: Some(ToolPayload::Text("i".repeat(ToolRecord::CAP - 6))),
        output: Some(ToolPayload::Text("out".into())),
        text: vec!["éé".to_owned(), "gone".to_owned()],
        ..ToolRecord::default()
    }
    .bounded();
    assert_eq!(
        record.input.as_ref().and_then(|v| v.as_str()).map(str::len),
        Some(ToolRecord::CAP - 6)
    );
    assert_eq!(record.output, Some(ToolPayload::Text("out".into())));
    // Three bytes were left for "éé" (four bytes): one char fits, the
    // cut never lands inside the second one.
    assert_eq!(record.text, ["é".to_owned(), String::new()]);
    assert_eq!(record.size(), ToolRecord::CAP - 1);
}

#[test]
fn an_update_replaces_what_it_carries_and_keeps_what_it_left_out() {
    let mut record = record();
    record.absorb(ToolRecord {
        kind: None,
        input: None,
        output: Some(ToolPayload::Json(r#"{"exit":0}"#.into())),
        text: vec!["done".to_owned()],
        locations: Vec::new(),
        terminal: Some("term-1".to_owned()),
        truncated_from: None,
    });
    assert_eq!(record.kind, Some(ToolVerb::Read));
    assert_eq!(
        record.input,
        Some(ToolPayload::Json(r#"{"file_path":"src/main.rs"}"#.into()))
    );
    assert_eq!(
        record.output,
        Some(ToolPayload::Json(r#"{"exit":0}"#.into()))
    );
    assert_eq!(record.text, ["done".to_owned()]);
    assert_eq!(record.locations, [(String::from("src/main.rs"), Some(1))]);
    assert_eq!(record.terminal.as_deref(), Some("term-1"));
    assert_eq!(record.truncated_from, None);
}

#[test]
fn absorbing_past_the_cap_cuts_the_whole_and_keeps_the_largest_size_seen() {
    let mut record = ToolRecord {
        input: Some(ToolPayload::Text("i".repeat(ToolRecord::CAP / 2))),
        truncated_from: Some(ToolRecord::CAP * 3),
        ..ToolRecord::default()
    };
    record.absorb(ToolRecord {
        output: Some(ToolPayload::Text("o".repeat(ToolRecord::CAP))),
        ..ToolRecord::default()
    });
    assert_eq!(record.size(), ToolRecord::CAP);
    assert_eq!(record.truncated_from, Some(ToolRecord::CAP * 3));
    assert_eq!(
        record
            .output
            .as_ref()
            .and_then(|v| v.as_str())
            .map(str::len),
        Some(ToolRecord::CAP / 2)
    );
}
