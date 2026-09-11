use super::super::{WorkerRequest, WorkerResponse};
use super::*;

#[test]
fn validates_json_values_and_rejects_malformed_or_trailing_content() {
    assert_eq!(
        validate_data(
            WorkerDataFormat::Json,
            r#"{"enabled":true,"values":[1,null,"ok"]}"#
        ),
        None
    );
    for source in [r#"{"key":}"#, "[1,]", "true false", ""] {
        assert!(
            validate_data(WorkerDataFormat::Json, source).is_some(),
            "{source}"
        );
    }
}

#[test]
fn validates_toml_documents_and_rejects_duplicate_keys() {
    assert_eq!(
        validate_data(
            WorkerDataFormat::Toml,
            "[package]\nname='test'\nversion='1.0.0'\n"
        ),
        None
    );
    for source in ["name=", "value=1\nvalue=2", "[unfinished"] {
        assert!(
            validate_data(WorkerDataFormat::Toml, source).is_some(),
            "{source}"
        );
    }
}

#[test]
fn validates_every_yaml_document_and_rejects_parser_errors() {
    assert_eq!(
        validate_data(
            WorkerDataFormat::Yaml,
            "---\nenabled: true\nvalues: [1, 2]\n---\nmessage: done\n"
        ),
        None
    );
    for source in [
        "items: [1, 2",
        "---\nvalid: true\n---\ninvalid: [",
        "value: *undefined",
    ] {
        assert!(
            validate_data(WorkerDataFormat::Yaml, source).is_some(),
            "{source}"
        );
    }
}

#[test]
fn yaml_aliases_are_validated_as_events_without_expansion() {
    let mut source = "level0: &a0 [value]\n".to_owned();
    for level in 1..24 {
        source.push_str(&format!(
            "level{level}: &a{level} [*a{}, *a{}, *a{}, *a{}]\n",
            level - 1,
            level - 1,
            level - 1,
            level - 1
        ));
    }
    assert_eq!(validate_data(WorkerDataFormat::Yaml, &source), None);
}

#[test]
fn data_validation_dispatch_and_wire_round_trip_preserve_diagnostics() {
    let request = WorkerRequest::ValidateData {
        path: "settings.toml".to_owned(),
        format: WorkerDataFormat::Toml,
        source: "key=".to_owned(),
    };
    let wire = serde_json::to_string(&request).expect("request encoding");
    assert!(wire.contains("\"operation\":\"validate_data\""));
    assert!(wire.contains("\"format\":\"toml\""));
    let decoded: WorkerRequest = serde_json::from_str(&wire).expect("request decoding");
    let response = decoded.execute().expect("worker dispatch");
    assert!(
        matches!(&response, WorkerResponse::DataValidated { diagnostic: Some(message) } if !message.is_empty())
    );
    let encoded = serde_json::to_string(&response).expect("response encoding");
    assert_eq!(
        serde_json::from_str::<WorkerResponse>(&encoded).expect("response decoding"),
        response
    );
}

#[test]
fn data_validation_quarantine_tracks_path_format_and_content() {
    let request = |path: &str, format, source: &str| WorkerRequest::ValidateData {
        path: path.to_owned(),
        format,
        source: source.to_owned(),
    };
    let original = request("input.json", WorkerDataFormat::Json, "{}")
        .input_key()
        .expect("input key");
    assert!(original.starts_with("input.json:"));
    for changed in [
        request("other.json", WorkerDataFormat::Json, "{}"),
        request("input.json", WorkerDataFormat::Yaml, "{}"),
        request("input.json", WorkerDataFormat::Json, "[]"),
    ] {
        assert_ne!(changed.input_key().expect("changed input key"), original);
    }
}
