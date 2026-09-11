use super::*;

pub(crate) const GOOD_TOML: &str = r#"
[plugin]
publisher = "acme"
pkg = "hello"
version = "0.1.0"

[component]
path = "guest.wasm"

[[tools]]
name = "greet"
description = "Greets the payload"

[capabilities]
imports = ["observe"]
fs-read = ["src/**"]

[limits]
fuel = 500
memory_bytes = 1048576
"#;

#[test]
fn a_valid_manifest_parses_and_wire_names_are_derived_never_declared() {
    let parsed = PluginManifest::parse(GOOD_TOML).expect("valid manifest");
    assert!(parsed.warnings.is_empty());
    let manifest = parsed.manifest;
    assert_eq!(manifest.publisher, "acme");
    assert_eq!(
        manifest.tools,
        vec![PluginToolDecl {
            name: "greet".into(),
            description: "Greets the payload".into()
        }]
    );
    assert_eq!(manifest.imports, vec!["observe"]);
    assert_eq!(manifest.fs_read, vec!["src/**"]);
    assert_eq!(
        manifest.limits,
        PluginLimits {
            fuel: Some(500),
            memory_bytes: Some(1_048_576)
        }
    );
    assert_eq!(manifest.invoke_export, "invoke");
    assert_eq!(manifest.wire_tool_name("greet"), "acme.hello/greet");
    assert_eq!(manifest.label(), "acme.hello");
}

#[test]
fn every_reserved_core_tool_name_is_refused_as_a_tool_name() {
    for reserved in RESERVED_TOOL_NAMES {
        let source = GOOD_TOML.replace("name = \"greet\"", &format!("name = \"{reserved}\""));
        let errors = PluginManifest::parse(&source).expect_err("reserved name refused");
        if reserved != "workbench_status" {
            assert!(
                errors.join(" ").contains("reserved"),
                "{reserved}: {errors:?}"
            );
        }
    }
}

#[test]
fn identity_component_path_and_tools_are_fail_closed() {
    let red = [
        GOOD_TOML.replace("publisher = \"acme\"", "publisher = \"ACME!\""),
        GOOD_TOML.replace("pkg = \"hello\"", "pkg = \"has/slash\""),
        GOOD_TOML.replace("version = \"0.1.0\"", "version = \"\""),
        GOOD_TOML.replace("path = \"guest.wasm\"", "path = \"../../etc/passwd\""),
        GOOD_TOML.replace("path = \"guest.wasm\"", "path = \"/abs/guest.wasm\""),
        GOOD_TOML.replace("path = \"guest.wasm\"", "path = \"C:/guest.wasm\""),
        GOOD_TOML.replace(
            "[[tools]]\nname = \"greet\"\ndescription = \"Greets the payload\"\n",
            "",
        ),
        GOOD_TOML.replace("fuel = 500", "fuel = \"lots\""),
        GOOD_TOML.replace("fuel = 500", "fuel = 0"),
        GOOD_TOML.replace("imports = [\"observe\"]", "imports = [\"\"]"),
        "not = [valid toml".to_owned(),
    ];
    for source in red {
        assert!(PluginManifest::parse(&source).is_err(), "{source}");
    }
}

#[test]
fn a_duplicate_tool_name_is_refused_and_unknown_tables_warn_but_parse() {
    let duplicate = format!("{GOOD_TOML}\n[[tools]]\nname = \"greet\"\ndescription = \"again\"\n");
    let errors = PluginManifest::parse(&duplicate).expect_err("duplicate refused");
    assert!(errors.iter().any(|e| e.contains("duplicate tool name")));
    let unknown = format!("{GOOD_TOML}\n[surprise]\nkey = 1\n");
    let parsed = PluginManifest::parse(&unknown).expect("unknown tables warn");
    assert!(parsed.warnings.join(" ").contains("surprise"));
}

#[test]
fn the_bundled_manifests_declare_only_public_imports_and_the_lang_pack_convention() {
    let resolver = PluginManifest::parse(include_str!(
        "../../../../subsystems/workspace/reference/plugins/lsp-resolver/plugin.toml"
    ))
    .expect("bundled resolver manifest")
    .manifest;
    assert_eq!(resolver.label(), "workbench.lsp-resolver");
    assert_eq!(
        resolver.imports,
        vec!["workbench:subprocess/subprocess@0.2.0"]
    );
    for (path, label, imports_empty) in [
        (
            include_str!(
                "../../../../subsystems/workspace/reference/plugins/lang-pack-gleam/plugin.toml"
            ),
            "workbench.lang-pack-gleam",
            true,
        ),
        (
            include_str!(
                "../../../../subsystems/workspace/reference/plugins/ranking-visibility/plugin.toml"
            ),
            "workbench.ranking-visibility",
            true,
        ),
        (
            include_str!(
                "../../../../subsystems/workspace/reference/plugins/runner-go/plugin.toml"
            ),
            "workbench.runner-go",
            true,
        ),
    ] {
        let manifest = PluginManifest::parse(path)
            .expect("bundled manifest")
            .manifest;
        assert_eq!(manifest.label(), label);
        assert_eq!(manifest.imports.is_empty(), imports_empty);
    }
}
