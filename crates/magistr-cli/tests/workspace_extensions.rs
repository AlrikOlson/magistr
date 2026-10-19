//! Native extension acceptance through the real Magistr worker: the plugin
//! TCK over the SDK examples and every bundled plugin, the plugin host over
//! real WAT components (capability denial, breaker, core continuity, the
//! blessed-only subprocess grant), lang-pack promotion lanes, the ranking
//! signal through search, the resolver lane over a spawned language server,
//! and the runner adapter over the retained golden fixtures.
mod workspace_extensions_support;

use std::{path::PathBuf, sync::Arc, time::Duration};

use magistr_workspace::{
    contracts::{ErrorKind, WorkspaceResponse},
    extensions::{
        LangPackBoot, LanguageServerSpec, PluginHealth, PluginHost, PluginResolver, RankingSignals,
        RunnerAdapter, RunnerInput,
        tck::{TckChapterId, TckFuzz, run_plugin_tck},
    },
    index::{IndexOptions, WorkspaceIndex, watch::WatchMode},
    query::{SearchInput, SearchMode, WorkspaceQueries},
    worker::{WorkerRequest, WorkerResponse},
};
use serde_json::json;
use workspace_extensions_support::{
    Fixture, lsp_fixture_binary, ok, record, reference_dir, refused,
};

const GOOD_TOML: &str = r#"
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

[limits]
fuel = 100000
memory_bytes = 1048576
"#;

/// Echo tool component: invoke(payload) returns the payload verbatim.
const ECHO_TOOL_WAT: &str = r#"
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "echo") (param i32 i32) (result i32)
      (i32.store (i32.const 16) (local.get 0))
      (i32.store (i32.const 20) (local.get 1))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "echo") (memory $i "mem") (realloc (func $i "realloc"))))
)
"#;

/// Requires an import no grant covers: denial happens before any guest code.
const NEEDY_TOOL_WAT: &str = r#"
(component
  (import "observe" (func $obs (param "n" u32) (result u32)))
  (core func $obs_core (canon lower (func $obs)))
  (core module $m
    (import "cap" "observe" (func $obs (param i32) (result i32)))
    (func (export "run") (result i32)
      (call $obs (i32.const 7))))
  (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
  (func (export "invoke") (result u32) (canon lift (core func $i "run")))
)
"#;

/// Traps as soon as it is invoked.
const CRASH_TOOL_WAT: &str = r#"
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "boom") (param i32 i32) (result i32) unreachable))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "boom") (memory $i "mem") (realloc (func $i "realloc"))))
)
"#;

fn without_imports(toml: &str) -> String {
    toml.replace("[capabilities]\nimports = [\"observe\"]\n", "")
}

#[tokio::test]
async fn tck_passes_the_sdk_examples_and_every_bundled_plugin_unmodified() {
    let fixture = Fixture::new();
    let lane = fixture.lane();
    let cases: [(&str, &str, bool); 6] = [
        ("examples/plugins/echo", "magistr.example-echo", false),
        (
            "examples/plugins/lang-pack-example",
            "magistr.example-lang-pack",
            true,
        ),
        ("plugins/lsp-resolver", "magistr.lsp-resolver", false),
        ("plugins/runner-go", "magistr.runner-go", false),
        ("plugins/lang-pack-gleam", "magistr.lang-pack-gleam", true),
        (
            "plugins/ranking-visibility",
            "magistr.ranking-visibility",
            false,
        ),
    ];
    for (dir, label, lang_pack) in cases {
        let report = run_plugin_tck(&reference_dir().join(dir), &lane, TckFuzz::default()).await;
        assert_eq!(report.plugin.as_deref(), Some(label));
        assert!(report.passed, "{label}: {report:#?}");
        for id in [
            TckChapterId::Interface,
            TckChapterId::Capabilities,
            TckChapterId::Resources,
        ] {
            assert!(report.chapter(id).unwrap().passed, "{label} {id:?}");
        }
        let fuzz = report.chapter(TckChapterId::LangPackFuzz).unwrap();
        assert_eq!(fuzz.applicable, lang_pack, "{label}");
        assert!(fuzz.passed);
        let resources = report.chapter(TckChapterId::Resources).unwrap();
        assert!(resources.probe("environment-fuel-kill").unwrap().passed);
        assert!(resources.probe("environment-memory-kill").unwrap().passed);
    }
    fixture.shutdown().await;
}

#[tokio::test]
async fn tck_lands_a_real_capability_violation_in_the_right_chapter() {
    let fixture = Fixture::new();
    let lane = fixture.lane();
    let needy = tempfile::tempdir().unwrap();
    std::fs::write(
        needy.path().join("plugin.toml"),
        without_imports(GOOD_TOML).replace("guest.wasm", "guest.wat"),
    )
    .unwrap();
    std::fs::write(needy.path().join("guest.wat"), NEEDY_TOOL_WAT).unwrap();
    let red = run_plugin_tck(needy.path(), &lane, TckFuzz::default()).await;
    assert!(!red.passed);
    let capabilities = red.chapter(TckChapterId::Capabilities).unwrap();
    assert!(
        !capabilities
            .probe("declared-imports-suffice")
            .unwrap()
            .passed
    );

    let honest = tempfile::tempdir().unwrap();
    std::fs::write(
        honest.path().join("plugin.toml"),
        GOOD_TOML.replace("guest.wasm", "guest.wat"),
    )
    .unwrap();
    std::fs::write(honest.path().join("guest.wat"), NEEDY_TOOL_WAT).unwrap();
    let report = run_plugin_tck(honest.path(), &lane, TckFuzz::default()).await;
    // Capabilities are conformant (declared exactly what it needs) even
    // though its integer-shaped export fails the interface chapter.
    assert!(report.chapter(TckChapterId::Capabilities).unwrap().passed);
    assert!(!report.chapter(TckChapterId::Interface).unwrap().passed);
    assert!(!report.passed);
    fixture.shutdown().await;
}

fn hello(fixture: &Fixture, toml: &str, component: &str, grants: Option<&[&str]>) {
    fixture.write("plugins/hello/plugin.toml", toml);
    fixture.write("plugins/hello/guest.wasm", component);
    fixture.install(vec![record(
        "acme-hello",
        "sideload",
        None,
        "plugins/hello",
        grants,
    )]);
}

#[tokio::test]
async fn plugin_host_denies_breakers_and_wraps_over_real_components() {
    let fixture = Fixture::new();
    fixture.write("src/app.ts", "export const alive = 1;\n");
    let lane = fixture.lane();
    let host = PluginHost::new(&fixture.policy, fixture.state_dir(), &lane);

    hello(&fixture, GOOD_TOML, ECHO_TOOL_WAT, None);
    let (rows, notes) = ok(host
        .run_tool("acme.hello/greet", &json!({"who": "world"}), 4000)
        .await
        .unwrap());
    assert_eq!(
        rows[0],
        json!({"v": 1, "tool": "greet", "args": {"who": "world"}})
    );
    assert!(notes[0].contains("host-wrapped"));

    hello(&fixture, GOOD_TOML, NEEDY_TOOL_WAT, None);
    let (kind, message, _) = refused(
        host.run_tool("acme.hello/greet", &json!({}), 4000)
            .await
            .unwrap(),
    );
    assert_eq!(kind, ErrorKind::PluginCapDenied);
    assert!(message.contains("\"observe\""));
    assert!(PluginHealth::new(&fixture.state_dir()).read().is_empty());

    // Granting the declared import opens the door; the integer-shaped export
    // is then refused by the calling convention as an honest PLUGIN_CRASHED.
    hello(&fixture, GOOD_TOML, NEEDY_TOOL_WAT, Some(&["observe"]));
    let (kind, _, _) = refused(
        host.run_tool("acme.hello/greet", &json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(matches!(kind, ErrorKind::PluginCrashed { .. }));

    hello(&fixture, GOOD_TOML, CRASH_TOOL_WAT, None);
    let (kind, _, _) = refused(
        host.run_tool("acme.hello/greet", &json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(matches!(kind, ErrorKind::PluginCrashed { .. }));
    let health: serde_json::Value = serde_json::from_str(
        &fixture
            .read(".magistr/workspace/plugin-health.json")
            .expect("durable breaker"),
    )
    .unwrap();
    assert_eq!(health["breaches"].as_array().unwrap().len(), 2);
    let (_, message, _) = refused(
        host.run_tool("acme.hello/greet", &json!({}), 4000)
            .await
            .unwrap(),
    );
    assert!(message.contains("circuit-breakered"));

    // Core tools never consulted any of it.
    let queries = WorkspaceQueries::new(&fixture.policy, &fixture.worker, None).unwrap();
    let mut search = SearchInput::new("alive");
    search.mode = SearchMode::Lexical;
    let (groups, _) = ok(queries.search(&search).await);
    assert_eq!(groups[0].file, "src/app.ts");
    fixture.shutdown().await;
}

#[tokio::test]
async fn subprocess_capability_is_blessed_only_over_the_real_resolver_guest() {
    let fixture = Fixture::new();
    let dir = fixture.copy_plugin("lsp-resolver");
    let lane = fixture.lane();
    let host = PluginHost::new(&fixture.policy, fixture.state_dir(), &lane);
    let granted = ["magistr:subprocess/subprocess@0.2.0"];
    for (source, tier) in [("sideload", None), ("registry", Some("verified"))] {
        fixture.install(vec![record(
            "magistr.lsp-resolver",
            source,
            tier,
            &dir,
            Some(&granted),
        )]);
        let (kind, _, notes) = refused(
            host.run_tool("magistr.lsp-resolver/status", &json!({}), 4000)
                .await
                .unwrap(),
        );
        assert_eq!(kind, ErrorKind::PluginCapDenied);
        assert!(notes.iter().any(|n| n.contains("reserved to blessed")));
    }
    assert!(PluginHealth::new(&fixture.state_dir()).read().is_empty());
    fixture.install(vec![record(
        "magistr.lsp-resolver",
        "registry",
        Some("blessed"),
        &dir,
        Some(&granted),
    )]);
    let (rows, _) = ok(host
        .run_tool("magistr.lsp-resolver/status", &json!({}), 4000)
        .await
        .unwrap());
    assert_eq!(rows[0]["resolver"], json!("magistr.lsp-resolver"));
    fixture.shutdown().await;
}

const GLEAM_FIXTURE: &str = "import gleam/io\n\npub fn main() {\n  io.println(greet(\"world\"))\n}\n\nfn greet(name: String) -> String {\n  \"hello\"\n}\n\npub type Season {\n  Spring\n  Summer\n}\n\npub const answer = 42\n";

async fn extract_gleam(fixture: &Fixture) -> WorkerResponse {
    fixture
        .worker
        .call(
            WorkerRequest::Extract {
                path: "src/main.gleam".into(),
                language: "gleam".into(),
                source: GLEAM_FIXTURE.into(),
            },
            Duration::from_secs(30),
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("registered gleam extracts through the worker")
}

#[tokio::test]
async fn lang_pack_boot_selects_the_lane_by_tier_and_extraction_is_identical_across_lanes() {
    let fixture = Fixture::new();
    let dir = fixture.copy_plugin("lang-pack-gleam");
    let lane = fixture.lane();
    let host = PluginHost::new(&fixture.policy, fixture.state_dir(), &lane);
    let mut extractions = Vec::new();
    for (tier, expected_lane) in [
        ("verified", "wasm"),
        ("blessed", "native"),
        ("verified", "wasm"),
    ] {
        fixture.install(vec![record(
            "magistr.lang-pack-gleam",
            "registry",
            Some(tier),
            &dir,
            None,
        )]);
        let statuses = LangPackBoot::new(&host).boot().await;
        assert_eq!(statuses.len(), 1);
        let status = &statuses[0];
        assert!(status.ok, "{status:?}");
        assert_eq!(status.language.as_deref(), Some("gleam"));
        assert_eq!(
            status.extensions.as_deref(),
            Some(&["gleam".to_owned()][..])
        );
        assert_eq!(status.lane.as_deref(), Some(expected_lane));
        assert_eq!(status.queries_ok, Some(true));
        extractions.push(extract_gleam(&fixture).await);
    }
    let WorkerResponse::Extract {
        symbols, imports, ..
    } = &extractions[0]
    else {
        panic!("extraction variant");
    };
    let names: Vec<(&str, &str)> = symbols
        .iter()
        .map(|s| (s.name.as_str(), s.kind.as_str()))
        .collect();
    assert!(names.contains(&("main", "function")));
    assert!(names.contains(&("greet", "function")));
    assert!(names.contains(&("Season", "type")));
    assert!(names.contains(&("answer", "const")));
    assert!(imports.iter().any(|i| i == "gleam/io"));
    assert_eq!(
        extractions[0], extractions[1],
        "wasm and native lanes agree"
    );
    assert_eq!(extractions[1], extractions[2]);
    fixture.shutdown().await;
}

#[tokio::test]
async fn ranking_signal_scores_through_the_real_guest_and_promotion_reorders_search() {
    let fixture = Fixture::new();
    fixture.write("src/a-internal.ts", "function needle() {}\n");
    fixture.write("src/z-exported.ts", "export function needle() {}\n");
    let dir = fixture.copy_plugin("ranking-visibility");
    fixture.install(vec![record(
        "magistr.ranking-visibility",
        "registry",
        Some("verified"),
        &dir,
        None,
    )]);
    let signals = RankingSignals::new(&fixture.state_dir(), Arc::new(fixture.lane()));
    let statuses = signals.boot(&fixture.policy, &fixture.state_dir());
    assert!(statuses[0].healthy && !statuses[0].promoted);
    let queries = WorkspaceQueries::new(&fixture.policy, &fixture.worker, None)
        .unwrap()
        .with_ranking(&signals);
    let mut input = SearchInput::new("needle");
    input.mode = SearchMode::Lexical;
    let files = |groups: Vec<magistr_workspace::query::SearchGroup>| {
        groups.into_iter().map(|g| g.file).collect::<Vec<_>>()
    };
    let (groups, _) = ok(queries.search(&input).await);
    assert_eq!(
        files(groups),
        vec!["src/a-internal.ts", "src/z-exported.ts"]
    );
    tokio::time::timeout(Duration::from_secs(20), async {
        while signals.statuses()[0].cached_hits < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the shadow lane scored the real guest");
    assert!(
        signals.promoted_features().is_none(),
        "shadow never applies"
    );
    let log = fixture
        .read(".magistr/workspace/ranking-shadow.jsonl")
        .unwrap();
    assert!(log.contains("\"ranking-visibility/exported\""));

    signals.promote("ranking-visibility").unwrap();
    let (groups, _) = ok(queries.search(&input).await);
    assert_eq!(
        files(groups),
        vec!["src/z-exported.ts", "src/a-internal.ts"]
    );
    signals.demote("ranking-visibility").unwrap();
    let (groups, _) = ok(queries.search(&input).await);
    assert_eq!(
        files(groups),
        vec!["src/a-internal.ts", "src/z-exported.ts"]
    );
    fixture.shutdown().await;
}

async fn warm_index(fixture: &Fixture) -> WorkspaceIndex {
    let index = WorkspaceIndex::start(
        fixture.root.clone(),
        fixture.directory.path().join("state"),
        Arc::clone(&fixture.worker),
        IndexOptions {
            watch_mode: WatchMode::Off,
            poll_interval: Duration::from_millis(50),
            ..IndexOptions::default()
        },
    )
    .expect("native index");
    tokio::time::timeout(Duration::from_secs(20), async {
        while index.status().building || index.status().generation == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("index warmed");
    index
}

#[tokio::test]
async fn resolver_lane_upgrades_references_through_the_real_guest_and_a_spawned_server() {
    let fixture = Fixture::new();
    fixture.write("lib.rs", "fn target() {}\n\n\n\n  target();\n");
    let dir = fixture.copy_plugin("lsp-resolver");
    fixture.install(vec![record(
        "magistr.lsp-resolver",
        "registry",
        Some("blessed"),
        &dir,
        Some(&["magistr:subprocess/subprocess@0.2.0"]),
    )]);
    let log = fixture.directory.path().join("lsp.log");
    let spec = LanguageServerSpec {
        language_id: "fixture".into(),
        bin: lsp_fixture_binary().display().to_string(),
        args: vec![log.display().to_string(), "echo".into()],
    };
    let lane = fixture.lane();
    let host = PluginHost::new(&fixture.policy, fixture.state_dir(), &lane);
    // The fixture server reports file:///workspace/lib.rs; the guest
    // relativizes against the root the host hands it.
    let resolver =
        PluginResolver::new(&host, "/workspace", vec![spec]).with_budget(Duration::from_secs(20));
    let edges = resolver.resolve_references("target", true).await;
    assert_eq!(edges.len(), 1);
    assert_eq!(
        (edges[0].path.as_str(), edges[0].line, edges[0].column),
        ("lib.rs", 5, 3)
    );
    assert!(PluginHealth::new(&fixture.state_dir()).read().is_empty());

    let mut index = warm_index(&fixture).await;
    let queries = WorkspaceQueries::new(&fixture.policy, &fixture.worker, Some(&index))
        .unwrap()
        .with_resolver(&resolver);
    let mut input = SearchInput::new("target");
    input.mode = SearchMode::References;
    let (groups, notes) = ok(queries.search(&input).await);
    let hits: Vec<_> = groups
        .iter()
        .flat_map(|g| {
            g.hits
                .iter()
                .map(|h| (h.line, h.provenance.clone(), h.confidence))
        })
        .collect();
    assert!(hits.iter().any(|(line, provenance, confidence)| *line == 5
        && provenance.as_deref() == Some("resolver-lsp")
        && *confidence == Some(magistr_workspace::query::SearchConfidence::High)));
    // SC-003: the definition row stays tier1-symbols; nothing unconfirmed is relabeled.
    assert!(
        hits.iter()
            .any(|(line, provenance, _)| *line == 1
                && provenance.as_deref() == Some("tier1-symbols"))
    );
    assert!(
        notes
            .iter()
            .any(|n| n.contains("resolver plugin upgraded references"))
    );
    index.shutdown().await.expect("index shutdown");
    fixture.shutdown().await;
}

fn runner_input(command: &str, exit_code: i32, stdout: &[&str], stderr: &[&str]) -> RunnerInput {
    RunnerInput {
        command: command.into(),
        exit_code,
        stdout: stdout.join("\n"),
        stderr: stderr.join("\n"),
    }
}

#[tokio::test]
async fn runner_lane_parses_the_golden_fixtures_and_claims_through_the_real_guest() {
    let fixture = Fixture::new();
    let lane = fixture.lane();
    let guest = std::fs::read(reference_dir().join("plugins/runner-go/guest.wasm")).unwrap();
    let adapter = RunnerAdapter::new("go", guest, &lane);
    let single = adapter
        .parse(runner_input(
            "go test ./...",
            1,
            &[
                "--- FAIL: TestAdd (0.00s)",
                "    math_test.go:12: expected 4, got 5",
                "    math_test.go:13: extra context",
                "FAIL",
                "FAIL\texample.com/m\t0.004s",
                "",
            ],
            &[],
        ))
        .await
        .unwrap()
        .expect("structured report");
    assert_eq!(single.failures.len(), 1);
    assert_eq!(single.failures[0].test_name, "TestAdd");
    assert_eq!(single.failures[0].file.as_deref(), Some("math_test.go"));
    assert_eq!(single.failures[0].assertion, "expected 4, got 5");
    assert_eq!(
        single.failures[0].trace,
        "math_test.go:12: expected 4, got 5\nmath_test.go:13: extra context"
    );
    assert_eq!(
        single.summary.as_deref(),
        Some("FAIL\texample.com/m\t0.004s")
    );

    let stderr_only = adapter
        .parse(runner_input(
            "go test",
            1,
            &[],
            &[
                "--- FAIL: TestEnv (2.30s)",
                "    env_test.go:99: missing VAR",
                "FAIL",
            ],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stderr_only.failures[0].test_name, "TestEnv");
    let passing = adapter
        .parse(runner_input(
            "go test ./...",
            0,
            &["ok  \texample.com/m\t0.020s", ""],
            &[],
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(passing.failures.is_empty());
    assert_eq!(
        passing.summary.as_deref(),
        Some("ok  \texample.com/m\t0.020s")
    );
    assert_eq!(
        adapter
            .parse(runner_input(
                "go test ./...",
                2,
                &["build failed: syntax error in main.go", ""],
                &[]
            ))
            .await
            .unwrap(),
        None
    );
    let big: Vec<String> = (1..=12)
        .map(|i| format!("    big_test.go:{i}: line {i}"))
        .collect();
    let mut lines = vec!["--- FAIL: TestBig (0.10s)".to_owned()];
    lines.extend(big);
    lines.push("FAIL\texample.com/big\t0.5s".into());
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let trimmed = adapter
        .parse(runner_input("go test", 1, &refs, &[]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(trimmed.failures[0].trace.lines().count(), 8);

    for (line, claimed) in [
        ("/usr/local/go/bin/go test", true),
        ("C:\\go\\bin\\go.exe test -v", true),
        ("GO=1 go vet", false),
        ("cargo test", false),
        ("gotestsum ./...", false),
        ("go build", false),
    ] {
        let result = adapter.parse(runner_input(line, 0, &[], &[])).await;
        assert_eq!(result.is_ok(), claimed, "{line}");
        if !claimed {
            assert_eq!(result.unwrap_err().kind, ErrorKind::CommandNotDeclared);
        }
    }
    fixture.shutdown().await;
}

#[tokio::test]
async fn detection_probes_the_real_host_without_failing() {
    let root = PathBuf::from(tempfile::tempdir().unwrap().path());
    assert!(LanguageServerSpec::detect(&root).is_empty());
    let _ = WorkspaceResponse::<()>::Failure {
        version: magistr_workspace::contracts::EnvelopeVersion::LATEST,
        error: magistr_workspace::contracts::WorkspaceError::new(ErrorKind::EngineUnavailable, ""),
        notes: None,
    };
}
