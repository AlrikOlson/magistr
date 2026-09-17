//! The cockpit knows no terminal (cockpit-crate-cutover): no file under
//! `crates/magistr-cockpit` may name a terminal protocol — `Protocol`,
//! `Passthrough`, `Kitty`, `Sixel` — reach into magistr-tui, or mention
//! the modules that stayed there; and the crate root keeps its
//! `deny(dead_code, unused)`.
//!
//! The first rule is what makes the capability vocabulary
//! (`capability::Mode`) the only way a surface tells the core what it
//! can show: a renderer that folded `Mode::Graphics(Protocol::Kitty(..))`
//! to say "I have pixels" is the impersonation this crate was carved out
//! to retire, and it cannot come back without tripping this walk.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
            out.push(path);
        }
    }
}

/// Spelled so this file does not itself trip the check.
fn terminal_names() -> Vec<String> {
    [
        ["Protocol", "::"],
        ["Msg::", "Reply"],
        ["fn ", "from_kitty"],
        ["fn ", "from_sgr"],
        ["fn ", "keyboard_flags"],
        ["Passthrough", "::"],
        ["Kitty", "("],
        ["Sixel", "Compositor"],
        ["magistr", "_tui"],
        ["crate::", "graphics"],
        ["crate::", "screenshot"],
        ["crate::", "runtime"],
        ["crate::", "term"],
        ["crate::", "flat"],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect()
}

fn offenders(needles: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    assert!(files.len() >= 100, "walked only {} files", files.len());
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let mut out = Vec::new();
    for file in files {
        if file.file_name().map(|n| n.to_os_string()) == this_file {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for needle in needles {
                if line.contains(needle.as_str()) {
                    out.push(format!("{}:{}: {needle}", file.display(), idx + 1));
                }
            }
        }
    }
    out
}

#[test]
fn no_cockpit_surface_names_a_terminal_protocol_or_the_terminal_crate() {
    let offenders = offenders(&terminal_names());
    assert!(
        offenders.is_empty(),
        "the terminal reached into the cockpit:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn concrete_service_clients_belong_to_the_host() {
    let mut files = Vec::new();
    walk(&crate_root().join("src"), &mut files);
    let forbidden = [
        "enum AgentEvent",
        "struct SessionOptions",
        "trait SessionPolicy",
        "struct ToolRecord",
        "magistr_engine::",
        "magistr_workflow::",
        "magistr_workspace::",
        "magistr_acp::",
        "agent_client_protocol::",
        "struct EngineClient",
        "struct NativeEnginePort",
        "struct AttachedEnginePort",
        "struct McpClient",
        "struct McpWorkbench",
        "ProjectAuthority::admit",
        "OwnerClient::attach",
        "std::process::",
        "tokio::",
        "struct Jobs",
        "struct SessionJobs",
        "struct Inbox",
        "struct Commands",
        "struct Arrivals",
        "fn spawn_session",
    ];
    let mut found = Vec::new();
    for file in files {
        if file.to_string_lossy().ends_with(".tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read production source");
        for (line, text) in source.lines().enumerate() {
            if text.trim_start().starts_with("//") {
                continue;
            }
            for token in forbidden {
                if text.contains(token) {
                    found.push(format!("{}:{}: {token}", file.display(), line + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "host adapters in cockpit:\n{}",
        found.join("\n")
    );
}

#[test]
fn cockpit_declares_no_async_runtime_dependency() {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml")).expect("manifest");
    for line in manifest.lines() {
        assert!(
            ![
                "tokio",
                "magistr-engine",
                "magistr-workflow",
                "magistr-workspace",
                "magistr-process",
                "magistr-acp"
            ]
            .iter()
            .any(|name| line.trim_start().starts_with(&format!("{name} ="))),
            "runtime dependency in cockpit: {line}"
        );
    }
}

#[test]
fn the_crate_root_denies_dead_and_unused_code() {
    let lib = std::fs::read_to_string(crate_root().join("src/lib.rs")).expect("read lib.rs");
    assert!(
        lib.contains("#![deny(dead_code, unused)]"),
        "lib.rs lost its deny"
    );
}

#[test]
fn no_file_allows_dead_or_unused_code() {
    let needles: Vec<String> = [
        ["allow(", "dead_code"],
        ["allow(", "unused"],
        ["expect(", "dead_code"],
        ["expect(", "unused"],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect();
    let offenders = offenders(&needles);
    assert!(
        offenders.is_empty(),
        "allow blankets:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn shared_view_dependency_closure_contains_no_effectful_host() {
    let output = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .current_dir(crate_root())
        .output()
        .expect("Cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).expect("metadata");
    let packages = metadata["packages"].as_array().expect("packages");
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved dependencies");
    let forbidden = [
        "magistr-engine",
        "magistr-workflow",
        "magistr-workspace",
        "magistr-process",
        "magistr-acp",
        "magistr-acp-agent",
        "magistr-cli",
        "magistr-tui",
        "tokio",
        "async-std",
        "smol",
        "reqwest",
        "rusqlite",
        "rmcp",
        "agent-client-protocol",
    ];
    for root_name in ["magistr-cockpit", "magistr-workflow-contracts"] {
        let root = packages
            .iter()
            .find(|p| p["name"] == root_name)
            .expect("shared crate");
        let mut pending = vec![root["id"].as_str().expect("id")];
        let mut visited = std::collections::BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let package = packages.iter().find(|p| p["id"] == id).expect("package");
            let name = package["name"].as_str().expect("package name");
            assert!(!forbidden.contains(&name), "{root_name} reaches {name}");
            // Declarations catch renamed, disabled optional and target-specific host dependencies.
            for dependency in package["dependencies"].as_array().expect("declarations") {
                if dependency["kind"] == "dev" {
                    continue;
                }
                let child = dependency["name"].as_str().expect("dependency name");
                assert!(
                    !forbidden.contains(&child),
                    "{root_name}: {name} declares {child}"
                );
            }
            let node = nodes.iter().find(|n| n["id"] == id).expect("resolved node");
            for edge in node["deps"].as_array().expect("edges") {
                if edge["dep_kinds"]
                    .as_array()
                    .expect("kinds")
                    .iter()
                    .any(|kind| kind["kind"].is_null())
                {
                    pending.push(edge["pkg"].as_str().expect("dependency id"));
                }
            }
        }
    }
    let contracts = packages
        .iter()
        .find(|p| p["name"] == "magistr-workflow-contracts")
        .expect("contracts");
    assert!(
        contracts["features"]
            .as_object()
            .expect("features")
            .is_empty(),
        "contract purity cannot depend on feature selection"
    );
    for dependency in contracts["dependencies"]
        .as_array()
        .expect("contract dependencies")
    {
        let name = dependency["name"].as_str().expect("name");
        assert!(
            [
                "magistr-domain",
                "serde",
                "serde_json",
                "schemars",
                "sha2",
                "chrono"
            ]
            .contains(&name),
            "unreviewed contract dependency: {name}"
        );
        assert!(
            dependency["kind"].is_null()
                && dependency["optional"] == false
                && dependency["rename"].is_null()
                && dependency["target"].is_null(),
            "contract dependencies cannot hide behind aliases or optional/target gates"
        );
        if name == "chrono" {
            assert_eq!(dependency["uses_default_features"], false);
            assert_eq!(dependency["features"], serde_json::json!(["std", "serde"]));
        }
    }
}

#[test]
fn canonical_contract_sources_own_no_ambient_effects() {
    let root = crate_root().join("../magistr-workflow-contracts");
    assert!(
        !root.join("build.rs").exists(),
        "contracts cannot run a build script"
    );
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    assert!(files.len() > 20, "canonical source tree must be present");
    for file in files {
        if file.to_string_lossy().ends_with(".tests.rs") {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("contract source");
        let syntax = syn::parse_file(&source).expect("contract syntax");
        let mut guard = ContractEffects::default();
        syn::visit::Visit::visit_file(&mut guard, &syntax);
        assert!(guard.0.is_empty(), "{}: {:?}", file.display(), guard.0);
    }
}

#[derive(Default)]
struct ContractEffects(Vec<String>);

impl ContractEffects {
    fn name(&mut self, ident: &syn::Ident) {
        let name = ident.to_string();
        if [
            "fs",
            "env",
            "process",
            "thread",
            "net",
            "tokio",
            "magistr_engine",
            "magistr_workflow",
            "magistr_workspace",
        ]
        .contains(&name.as_str())
        {
            self.0.push(name);
        }
    }
}

impl<'ast> syn::visit::Visit<'ast> for ContractEffects {
    fn visit_path_segment(&mut self, segment: &'ast syn::PathSegment) {
        self.name(&segment.ident);
        syn::visit::visit_path_segment(self, segment);
    }

    fn visit_use_path(&mut self, path: &'ast syn::UsePath) {
        self.name(&path.ident);
        syn::visit::visit_use_path(self, path);
    }

    fn visit_use_name(&mut self, name: &'ast syn::UseName) {
        self.name(&name.ident);
    }

    fn visit_use_rename(&mut self, rename: &'ast syn::UseRename) {
        self.name(&rename.ident);
    }

    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        if signature.asyncness.is_some() {
            self.0.push("async".into());
        }
        syn::visit::visit_signature(self, signature);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && path.path.segments.last().is_some_and(|s| s.ident == "now")
        {
            self.0.push("ambient clock".into());
        }
        syn::visit::visit_expr_call(self, call);
    }
}
