use magistr_workspace::file_content::FileContent;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    bytes: Vec<u8>,
    kind: String,
    hash: String,
}

#[test]
fn content_matches_pinned_reference_vectors() {
    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(
        "../../../subsystems/workspace/conformance/content.json"
    ))
    .unwrap();
    for fixture in fixtures {
        let content = FileContent::decode(fixture.bytes.clone());
        let kind = match &content {
            FileContent::Text(_) => "text",
            FileContent::Binary(_) => "binary",
        };
        assert_eq!(kind, fixture.kind);
        assert_eq!(content.as_bytes(), fixture.bytes);
        assert_eq!(content.content_hash(), fixture.hash);
        assert_eq!(content, FileContent::Binary(fixture.bytes));
    }
}
