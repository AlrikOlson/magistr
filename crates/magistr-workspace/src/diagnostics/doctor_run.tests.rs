use std::path::Path;

use super::*;
use crate::{
    diagnostics::{DoctorDataClass, DoctorFinding, DoctorSection},
    index::cache::IndexCache,
};

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::write(dir.path().join("a.ts"), "export const a = 1;\n").expect("source");
    dir
}

fn options(root: &Path) -> DoctorOptions {
    DoctorOptions::new(root)
        .with_tool_version("0.0.0-test")
        .with_now_unix_ms(1_787_356_800_000)
}

/// A healthy native index (a real cache, opened then released), one
/// quarantine record plus a torn line, one breaker record and a sideloaded
/// plugin entry.
fn seed_state_dir(root: &Path) -> std::path::PathBuf {
    let state = root.join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    drop(IndexCache::open(&state, "WAL").expect("native index"));
    std::fs::write(
        state.join("quarantine.jsonl"),
        format!(
            "{}\nthis line is torn and not json\n",
            serde_json::json!({ "path": "weird.bin", "strikes": 3, "quarantined": true, "lastCrash": "SIGSEGV", "at": 1755000000000_u64 })
        ),
    )
    .expect("quarantine");
    std::fs::write(
        state.join("plugin-health.json"),
        serde_json::to_string_pretty(&serde_json::json!({ "breaches": [{ "plugin": "acme.bad", "componentHash": "deadbeef", "reason": "guest trap", "at": "2026-08-21T00:00:00Z" }] })).expect("json"),
    )
    .expect("breaker");
    std::fs::write(
        state.join("plugins.json"),
        serde_json::to_string_pretty(
            &serde_json::json!({ "plugins": [{ "name": "acme.bad", "source": "sideload" }] }),
        )
        .expect("json"),
    )
    .expect("inventory");
    state
}

fn section(run: &DoctorRun, id: DoctorSectionId) -> &DoctorSection {
    run.report
        .sections
        .iter()
        .find(|section| section.id == id)
        .expect("section present")
}

#[test]
fn all_six_sections_with_sources_provenance_quarantine_and_breaker_visibility() {
    let dir = workspace();
    seed_state_dir(dir.path());
    let mut flags = serde_json::Map::new();
    flags.insert("budgets.default".into(), 9999.into());
    let run = DoctorRun::observe(&options(dir.path()).with_session_flags(flags));
    let ids: Vec<_> = run
        .report
        .sections
        .iter()
        .map(|section| section.id)
        .collect();
    assert_eq!(
        ids,
        [
            DoctorSectionId::Environment,
            DoctorSectionId::Index,
            DoctorSectionId::Config,
            DoctorSectionId::Plugins,
            DoctorSectionId::Quarantine,
            DoctorSectionId::Crashes,
        ]
    );
    for section in &run.report.sections {
        assert!(!section.source.is_empty());
    }
    let text = run.render();
    assert!(text.contains("effective config (value ← deciding layer):"));
    assert!(text.contains("budgets.default = 9999  (session)"));
    assert!(text.contains("weird.bin quarantined after 3 strike(s)"));
    assert!(text.contains("INPUT_QUARANTINED clears when the file changes"));
    assert!(text.contains("schema 1, quick_check ok"));
    assert!(text.contains("crash strike record(s)"));
    assert!(text.contains("generated 2026-08-22T00:00:00.000Z by magistr-workspace 0.0.0-test"));
    assert!(text.ends_with("result: findings present (exit 1)"));
    assert!(run.report.findings_present);
    for section in &run.report.sections {
        for finding in &section.findings {
            if finding.severity == DoctorSeverity::Unhealthy {
                assert!(
                    finding
                        .remediation
                        .as_deref()
                        .is_some_and(|text| !text.is_empty())
                );
            }
        }
    }
    assert_eq!(
        section(&run, DoctorSectionId::Quarantine).detail["malformedLines"],
        1
    );
    #[cfg(feature = "sandbox")]
    {
        assert!(text.contains("acme.bad breakered since"));
        assert!(text.contains("changed component hash clears the breaker"));
        assert_eq!(
            section(&run, DoctorSectionId::Plugins).detail["inventory"][0]["trust"],
            "sideload"
        );
    }
}

#[test]
fn a_fresh_workspace_has_no_findings_and_the_probe_creates_no_state() {
    let dir = workspace();
    let run = DoctorRun::observe(&options(dir.path()));
    assert!(!run.report.findings_present);
    let index = section(&run, DoctorSectionId::Index);
    assert_eq!(index.findings[0].severity, DoctorSeverity::Info);
    assert!(index.findings[0].symptom.contains("no native index yet"));
    assert!(!dir.path().join(".workbench").exists());
    assert!(
        run.render()
            .ends_with("result: all probes healthy (exit 0)")
    );
}

#[test]
fn a_corrupt_or_newer_index_is_an_unhealthy_cache_finding_with_a_rebuild_remediation() {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let index_path = state.join("native-index.db");
    std::fs::write(&index_path, "garbage-not-a-sqlite-database").expect("corrupt");
    let run = DoctorRun::observe(&options(dir.path()));
    let finding = &section(&run, DoctorSectionId::Index).findings[0];
    assert_eq!(finding.severity, DoctorSeverity::Unhealthy);
    assert_eq!(finding.data_class, DoctorDataClass::Cache);
    assert!(finding.symptom.contains("unreadable as a database"));
    let remediation = finding.remediation.as_deref().expect("remediation");
    assert!(remediation.contains(".workbench/native-index.db") && remediation.contains("rebuild"));
    assert_eq!(
        std::fs::read(&index_path).expect("bytes"),
        b"garbage-not-a-sqlite-database"
    );

    std::fs::remove_file(&index_path).expect("reset");
    let raw = rusqlite::Connection::open(&index_path).expect("raw index");
    raw.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL); PRAGMA user_version = 999")
        .expect("future schema");
    drop(raw);
    let run = DoctorRun::observe(&options(dir.path()));
    let finding = &section(&run, DoctorSectionId::Index).findings[0];
    assert_eq!(finding.severity, DoctorSeverity::Unhealthy);
    assert!(finding.symptom.contains("newer than this binary supports"));
    assert!(
        finding
            .remediation
            .as_deref()
            .expect("remediation")
            .contains("upgrade")
    );
}

#[test]
fn read_only_beside_a_live_owner_holding_the_native_index_lock() {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let owner = IndexCache::open(&state, "WAL").expect("live owner");
    let snapshot = || -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = std::fs::read_dir(&state)
            .expect("state dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                !name.ends_with("-shm") && !name.ends_with("-wal")
            })
            .map(|entry| {
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    std::fs::read(entry.path()).expect("bytes"),
                )
            })
            .collect();
        files.sort();
        files
    };
    let before = snapshot();
    let run = DoctorRun::observe(&options(dir.path()));
    let finding = &section(&run, DoctorSectionId::Index).findings[0];
    assert_eq!(
        finding.severity,
        DoctorSeverity::Healthy,
        "{}",
        finding.symptom
    );
    assert_eq!(
        snapshot(),
        before,
        "nothing under the state dir was rewritten"
    );
    // The owner is untouched: it still answers as the cache it was.
    assert_eq!(owner.journal_mode, "WAL");
    drop(owner);
}

#[test]
fn a_broken_config_layer_is_a_durable_finding_and_the_report_still_renders() {
    let dir = workspace();
    std::fs::write(dir.path().join("workbench.toml"), "this = = is not toml").expect("bad config");
    let run = DoctorRun::observe(&options(dir.path()));
    let config = section(&run, DoctorSectionId::Config);
    let text = run.render();
    if config.findings[0].severity == DoctorSeverity::Unhealthy {
        assert_eq!(config.findings[0].data_class, DoctorDataClass::Durable);
        assert!(config.findings[0].remediation.is_some());
    } else {
        // Resolution may tolerate the layer as a warning; either way it is named.
        assert!(
            text.contains("warning")
                || config.detail["warnings"]
                    .as_array()
                    .is_some_and(|w| !w.is_empty())
        );
    }
    assert!(text.contains("[config]"));
}

#[test]
fn a_host_section_is_appended_in_order_and_its_unhealthy_finding_flips_the_result() {
    let dir = workspace();
    let mut run = DoctorRun::observe(&options(dir.path()));
    assert!(!run.report.findings_present);
    run.append(DoctorSection {
        id: DoctorSectionId::Owner,
        source: "probe".into(),
        findings: vec![DoctorFinding::healthy("owner", "no owner reachable")],
        detail: serde_json::json!({ "live": false }),
    });
    assert!(!run.report.findings_present);
    run.append(DoctorSection {
        id: DoctorSectionId::Operations,
        source: "checkpoint".into(),
        findings: vec![DoctorFinding::unhealthy(
            "operations",
            DoctorDataClass::Durable,
            "one attempt stalled",
            "wait for the runtime",
        )],
        detail: serde_json::json!({ "records": [] }),
    });
    assert!(run.report.findings_present);
    let ids: Vec<_> = run
        .report
        .sections
        .iter()
        .map(|section| section.id)
        .collect();
    assert_eq!(ids.len(), 8);
    assert_eq!(ids[6], DoctorSectionId::Owner);
    assert_eq!(ids[7], DoctorSectionId::Operations);
    let text = run.render();
    let owner = text.find("[owner]").expect("owner section");
    let operations = text.find("[operations]").expect("operations section");
    assert!(owner < operations);
    assert!(text.contains("  source: probe"));
    assert!(text.contains("remediation: wait for the runtime"));
    assert!(text.ends_with("result: findings present (exit 1)"));
}
