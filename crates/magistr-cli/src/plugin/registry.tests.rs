use super::super::types::{MANIFEST_FILENAME, PluginCompatibility, PluginKind};
use super::*;
use tempfile::TempDir;

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn create_detection_plugin(dir: &Path) {
    write_file(
        dir,
        MANIFEST_FILENAME,
        r#"
name = "test-detect"
version = "1.0.0"
kind = "detection"
"#,
    );
    write_file(
        dir,
        "detect.toml",
        r#"
[[rules]]
name = "my-framework"
file_patterns = ["framework.config.js"]
"#,
    );
}

fn create_template_plugin(dir: &Path) {
    write_file(
        dir,
        MANIFEST_FILENAME,
        r#"
name = "test-templates"
version = "1.0.0"
kind = "template"
"#,
    );
    write_file(
        dir,
        "templates.toml",
        r#"
[[files]]
src = "rules/team.md"
dest = "rules/team.md"
"#,
    );
    write_file(dir, "rules/team.md", "# Team Rules\n");
}

// ── PluginRegistry basic tests ─────────────────────────────

#[test]
fn new_registry_is_empty() {
    let dir = TempDir::new().unwrap();
    let registry = PluginRegistry::new(dir.path());
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert!(registry.list().is_empty());
}

#[test]
fn load_missing_registry_returns_empty() {
    let dir = TempDir::new().unwrap();
    let registry = PluginRegistry::load(dir.path(), OrgPluginConfig::default()).unwrap();
    assert!(registry.is_empty());
}

// ── add_local tests ────────────────────────────────────────

#[test]
fn add_local_copies_plugin() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let mut registry = PluginRegistry::new(base.path());
    let installed = registry.add_local(src.path()).unwrap();

    assert_eq!(installed.manifest.name, "test-detect");
    assert_eq!(installed.manifest.kind, PluginKind::Detection);
    assert!(registry.is_installed("test-detect"));

    // Verify files were copied
    let copied_manifest = base.path().join("test-detect").join(MANIFEST_FILENAME);
    assert!(copied_manifest.is_file());
}

#[test]
fn add_local_duplicate_errors() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let mut registry = PluginRegistry::new(base.path());
    registry.add_local(src.path()).unwrap();

    let result = registry.add_local(src.path());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("already installed")
    );
}

#[test]
fn add_local_nonexistent_dir_errors() {
    let base = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new(base.path());

    let result = registry.add_local(Path::new("/nonexistent/plugin"));
    assert!(result.is_err());
}

// ── remove tests ───────────────────────────────────────────

#[test]
fn remove_plugin() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let mut registry = PluginRegistry::new(base.path());
    registry.add_local(src.path()).unwrap();
    assert!(registry.is_installed("test-detect"));

    let removed = registry.remove("test-detect").unwrap();
    assert_eq!(removed.manifest.name, "test-detect");
    assert!(!registry.is_installed("test-detect"));

    // Verify directory was deleted
    assert!(!base.path().join("test-detect").exists());
}

#[test]
fn remove_nonexistent_errors() {
    let base = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new(base.path());

    let result = registry.remove("nope");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ── get tests ──────────────────────────────────────────────

#[test]
fn get_installed_plugin() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let mut registry = PluginRegistry::new(base.path());
    registry.add_local(src.path()).unwrap();

    let plugin = registry.get("test-detect");
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().manifest.name, "test-detect");
}

#[test]
fn get_missing_returns_none() {
    let base = TempDir::new().unwrap();
    let registry = PluginRegistry::new(base.path());
    assert!(registry.get("nope").is_none());
}

// ── save/load round trip ───────────────────────────────────

#[test]
fn save_and_load_round_trip() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let mut registry = PluginRegistry::new(base.path());
    registry.add_local(src.path()).unwrap();
    registry.save().unwrap();

    let loaded = PluginRegistry::load(base.path(), OrgPluginConfig::default()).unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded.is_installed("test-detect"));
    assert_eq!(loaded.get("test-detect").unwrap().manifest.version, "1.0.0");
}

// ── register tests ─────────────────────────────────────────

#[test]
fn register_directly() {
    let base = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new(base.path());

    let manifest = PluginManifest {
        name: "manual".to_string(),
        version: "0.1.0".to_string(),
        description: None,
        kind: PluginKind::Template,
        author: None,
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };

    registry
        .register(
            manifest,
            PluginSource::Local {
                path: "/tmp/manual".to_string(),
            },
        )
        .unwrap();

    assert!(registry.is_installed("manual"));
}

#[test]
fn register_duplicate_errors() {
    let base = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new(base.path());

    let manifest = PluginManifest {
        name: "dup".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::Detection,
        author: None,
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };

    registry
        .register(
            manifest.clone(),
            PluginSource::Local {
                path: "/a".to_string(),
            },
        )
        .unwrap();

    let result = registry.register(
        manifest,
        PluginSource::Local {
            path: "/b".to_string(),
        },
    );
    assert!(result.is_err());
}

// ── plugin_dir tests ───────────────────────────────────────

#[test]
fn plugin_dir_returns_expected_path() {
    let base = TempDir::new().unwrap();
    let registry = PluginRegistry::new(base.path());

    let dir = registry.plugin_dir("my-plugin");
    assert_eq!(dir, base.path().join("my-plugin"));
}

// ── copy_dir_recursive tests ───────────────────────────────

#[test]
fn copy_dir_recursive_copies_nested() {
    let src = TempDir::new().unwrap();
    let dest = TempDir::new().unwrap();

    write_file(src.path(), "a.txt", "hello");
    write_file(src.path(), "sub/b.txt", "world");
    write_file(src.path(), "sub/deep/c.txt", "nested");

    let dest_path = dest.path().join("output");
    copy_dir_recursive(src.path(), &dest_path).unwrap();

    assert_eq!(
        std::fs::read_to_string(dest_path.join("a.txt")).unwrap(),
        "hello"
    );
    assert_eq!(
        std::fs::read_to_string(dest_path.join("sub/b.txt")).unwrap(),
        "world"
    );
    assert_eq!(
        std::fs::read_to_string(dest_path.join("sub/deep/c.txt")).unwrap(),
        "nested"
    );
}

// ── multiple plugins ───────────────────────────────────────

#[test]
fn add_multiple_plugins() {
    let base = TempDir::new().unwrap();
    let src1 = TempDir::new().unwrap();
    let src2 = TempDir::new().unwrap();

    create_detection_plugin(src1.path());
    create_template_plugin(src2.path());

    let mut registry = PluginRegistry::new(base.path());
    registry.add_local(src1.path()).unwrap();
    registry.add_local(src2.path()).unwrap();

    assert_eq!(registry.len(), 2);
    assert!(registry.is_installed("test-detect"));
    assert!(registry.is_installed("test-templates"));
}

// ── find_plugin_root tests ─────────────────────────────────

#[test]
fn find_plugin_root_manifest_at_root() {
    let dir = TempDir::new().unwrap();
    write_file(
        dir.path(),
        MANIFEST_FILENAME,
        "name = \"test\"\nversion = \"1.0.0\"\nkind = \"detection\"",
    );
    let root = find_plugin_root(dir.path()).unwrap();
    assert_eq!(root, dir.path());
}

#[test]
fn find_plugin_root_manifest_in_subdir() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("my-plugin");
    std::fs::create_dir_all(&sub).unwrap();
    write_file(
        &sub,
        MANIFEST_FILENAME,
        "name = \"test\"\nversion = \"1.0.0\"\nkind = \"detection\"",
    );
    let root = find_plugin_root(dir.path()).unwrap();
    assert_eq!(root, sub);
}

#[test]
fn find_plugin_root_no_manifest_errors() {
    let dir = TempDir::new().unwrap();
    write_file(dir.path(), "random.txt", "not a plugin");
    let result = find_plugin_root(dir.path());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("magistr-plugin.toml")
    );
}

// ── PluginSource::Registry serialization ───────────────────

#[test]
fn plugin_source_registry_display() {
    let source = PluginSource::Registry {
        name: "openai".into(),
        version: "1.0.0".into(),
    };
    assert_eq!(source.to_string(), "registry:openai@1.0.0");
}

#[test]
fn plugin_source_registry_serde_roundtrip() {
    let source = PluginSource::Registry {
        name: "my-gate".into(),
        version: "2.1.0".into(),
    };
    let json = serde_json::to_string(&source).unwrap();
    let parsed: PluginSource = serde_json::from_str(&json).unwrap();
    assert_eq!(source, parsed);
}

// ── OrgPluginConfig tests ──────────────────────────────────

#[test]
fn org_config_default_is_unrestricted() {
    let config = OrgPluginConfig::default();
    assert!(config.index_url.is_none());
    assert!(config.allowlist.is_empty());
    assert!(config.signing_key.is_none());
    assert!(!config.has_allowlist());
    assert!(config.is_allowed("any-plugin"));
}

#[test]
fn org_config_allowlist_enforcement() {
    let config = OrgPluginConfig {
        index_url: None,
        allowlist: vec!["license-gate".to_string(), "sonarqube-gate".to_string()],
        signing_key: None,
    };

    assert!(config.has_allowlist());
    assert!(config.is_allowed("license-gate"));
    assert!(config.is_allowed("sonarqube-gate"));
    assert!(!config.is_allowed("random-plugin"));
    assert!(!config.is_allowed("malicious-plugin"));
}

#[test]
fn org_config_effective_index_url_default() {
    let config = OrgPluginConfig::default();
    assert_eq!(
        config.effective_index_url(),
        super::super::index::DEFAULT_INDEX_URL
    );
}

#[test]
fn org_config_effective_index_url_private() {
    let config = OrgPluginConfig {
        index_url: Some("https://plugins.myorg.com/index.json".to_string()),
        allowlist: Vec::new(),
        signing_key: None,
    };
    assert_eq!(
        config.effective_index_url(),
        "https://plugins.myorg.com/index.json"
    );
}

#[test]
fn org_config_serde_roundtrip() {
    let config = OrgPluginConfig {
        index_url: Some("https://private.index/plugins.json".to_string()),
        allowlist: vec!["a".to_string(), "b".to_string()],
        signing_key: Some("ed25519-pubkey".to_string()),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: OrgPluginConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, parsed);
}

#[test]
fn org_config_save_and_load() {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join(".config/magistr");
    std::fs::create_dir_all(&config_dir).unwrap();

    let config = OrgPluginConfig {
        index_url: Some("https://private.example.com/index.json".to_string()),
        allowlist: vec!["approved-plugin".to_string()],
        signing_key: None,
    };

    let path = config_dir.join("org-plugins.json");
    let content = serde_json::to_string_pretty(&config).unwrap();
    std::fs::write(&path, content).unwrap();

    let loaded: OrgPluginConfig =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(config, loaded);
}

#[test]
fn org_config_allowed_set() {
    let config = OrgPluginConfig {
        index_url: None,
        allowlist: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        signing_key: None,
    };

    let set = config.allowed_set();
    assert_eq!(set.len(), 3);
    assert!(set.contains("a"));
    assert!(set.contains("b"));
    assert!(set.contains("c"));
    assert!(!set.contains("d"));
}

// ── Allowlist enforcement in registry ────────────────────────

#[test]
fn registry_with_allowlist_blocks_unlisted_plugin() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path()); // creates "test-detect"

    let org_config = OrgPluginConfig {
        index_url: None,
        allowlist: vec!["approved-only".to_string()], // test-detect NOT in list
        signing_key: None,
    };

    let mut registry = PluginRegistry::with_org_config(base.path(), org_config);
    let result = registry.add_local(src.path());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not in the organization allowlist")
    );
}

#[test]
fn registry_with_allowlist_allows_listed_plugin() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path()); // creates "test-detect"

    let org_config = OrgPluginConfig {
        index_url: None,
        allowlist: vec!["test-detect".to_string()], // test-detect IS in list
        signing_key: None,
    };

    let mut registry = PluginRegistry::with_org_config(base.path(), org_config);
    let result = registry.add_local(src.path());
    assert!(result.is_ok());
    assert!(registry.is_installed("test-detect"));
}

#[test]
fn registry_without_allowlist_allows_everything() {
    let base = TempDir::new().unwrap();
    let src = TempDir::new().unwrap();
    create_detection_plugin(src.path());

    let org_config = OrgPluginConfig::default(); // empty allowlist = no restrictions
    let mut registry = PluginRegistry::with_org_config(base.path(), org_config);

    let result = registry.add_local(src.path());
    assert!(result.is_ok());
}

#[test]
fn registry_register_blocked_by_allowlist() {
    let base = TempDir::new().unwrap();
    let org_config = OrgPluginConfig {
        index_url: None,
        allowlist: vec!["approved".to_string()],
        signing_key: None,
    };

    let mut registry = PluginRegistry::with_org_config(base.path(), org_config);

    let manifest = PluginManifest {
        name: "blocked-plugin".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        kind: PluginKind::Detection,
        author: None,
        entrypoint: None,
        permissions: Vec::new(),
        dependencies: Vec::new(),
        compatibility: PluginCompatibility::default(),
    };

    let result = registry.register(
        manifest,
        PluginSource::Local {
            path: "/tmp".to_string(),
        },
    );
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not in the organization allowlist")
    );
}
