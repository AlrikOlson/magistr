use super::*;

#[test]
fn parse_index_entry_from_json() {
    let json = r#"{
        "name": "sonar-gate",
        "version": "1.0.0",
        "kind": "quality-gate",
        "description": "SonarQube quality gate",
        "download_url": "https://plugins.magistr.dev/sonar-gate-1.0.0.tar.gz",
        "signature": "base64signature==",
        "author": "magistr-team"
    }"#;

    let entry: IndexEntry = serde_json::from_str(json).expect("parse entry");
    assert_eq!(entry.name, "sonar-gate");
    assert_eq!(entry.version, "1.0.0");
    assert_eq!(entry.kind, PluginKind::QualityGate);
    assert_eq!(entry.description, Some("SonarQube quality gate".into()));
    assert_eq!(
        entry.download_url,
        "https://plugins.magistr.dev/sonar-gate-1.0.0.tar.gz"
    );
    assert_eq!(entry.signature, Some("base64signature==".into()));
    assert_eq!(entry.author, Some("magistr-team".into()));
}

#[test]
fn parse_index_entry_minimal() {
    let json = r#"{
        "name": "my-plugin",
        "version": "0.1.0",
        "kind": "detection",
        "download_url": "https://example.com/my-plugin.tar.gz"
    }"#;

    let entry: IndexEntry = serde_json::from_str(json).expect("parse entry");
    assert_eq!(entry.name, "my-plugin");
    assert!(entry.description.is_none());
    assert!(entry.signature.is_none());
    assert!(entry.author.is_none());
}

#[test]
fn serialize_index_entry_roundtrip() {
    let entry = IndexEntry {
        name: "test-plugin".into(),
        version: "2.0.0".into(),
        kind: PluginKind::QualityGate,
        description: Some("A test gate".into()),
        download_url: "https://example.com/test.tar.gz".into(),
        signature: None,
        author: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: IndexEntry = serde_json::from_str(&json).expect("parse");
    assert_eq!(entry, parsed);
}

#[test]
fn parse_registry_index() {
    let json = r#"{
        "version": 1,
        "plugins": [
            {
                "name": "alpha",
                "version": "1.0.0",
                "kind": "detection",
                "download_url": "https://example.com/alpha.tar.gz"
            },
            {
                "name": "beta",
                "version": "0.5.0",
                "kind": "template",
                "download_url": "https://example.com/beta.tar.gz"
            }
        ]
    }"#;

    let index = RegistryIndex::from_json(json).expect("parse index");
    assert_eq!(index.version, 1);
    assert_eq!(index.plugins.len(), 2);
    assert_eq!(index.plugins[0].name, "alpha");
    assert_eq!(index.plugins[1].name, "beta");
}

#[test]
fn search_by_name_finds_plugin() {
    let index = RegistryIndex {
        version: 1,
        plugins: vec![
            IndexEntry {
                name: "openai".into(),
                version: "1.0.0".into(),
                kind: PluginKind::QualityGate,
                description: None,
                download_url: "https://example.com/openai.tar.gz".into(),
                signature: None,
                author: None,
            },
            IndexEntry {
                name: "sonar".into(),
                version: "2.0.0".into(),
                kind: PluginKind::QualityGate,
                description: None,
                download_url: "https://example.com/sonar.tar.gz".into(),
                signature: None,
                author: None,
            },
        ],
    };

    let found = index.find("sonar");
    assert!(found.is_some());
    assert_eq!(found.expect("found").version, "2.0.0");

    assert!(index.find("nonexistent").is_none());
}

#[test]
fn search_is_case_sensitive() {
    let index = RegistryIndex {
        version: 1,
        plugins: vec![IndexEntry {
            name: "MyPlugin".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Detection,
            description: None,
            download_url: "https://example.com/my.tar.gz".into(),
            signature: None,
            author: None,
        }],
    };

    assert!(index.find("MyPlugin").is_some());
    assert!(index.find("myplugin").is_none());
}

#[test]
fn serialize_registry_index_roundtrip() {
    let index = RegistryIndex {
        version: 1,
        plugins: vec![IndexEntry {
            name: "test".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Template,
            description: Some("desc".into()),
            download_url: "https://example.com/test.tar.gz".into(),
            signature: Some("sig".into()),
            author: Some("author".into()),
        }],
    };

    let json = index.to_json().expect("serialize");
    let parsed = RegistryIndex::from_json(&json).expect("parse");
    assert_eq!(index, parsed);
}
