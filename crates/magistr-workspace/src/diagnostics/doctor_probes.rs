//! Each probe reads one shipped artifact or environment fact, read-only, and
//! yields a section. A probe never repairs, never takes a lock and never
//! creates a file — a missing or unreadable store is a finding, not an error.
use std::{path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{Value, json};

use super::{DoctorDataClass, DoctorFinding, DoctorOptions, DoctorSection, DoctorSectionId};
use crate::{
    config::{ResolveOptions, ResolvedConfig},
    contracts::ErrorKind,
    index::watch::NetworkDetection,
};

/// The native cache schema this binary reads; the doctor test pins it
/// against a freshly opened cache.
const NATIVE_INDEX_SCHEMA: i64 = 1;
const REBUILD_INDEX: &str = "delete .workbench/native-index.db and its -wal/-shm sidecars; the index is rebuildable cache data (SDD §10.2) and the next index start rebuilds it — no user data is involved";

fn section(
    id: DoctorSectionId,
    source: &str,
    findings: Vec<DoctorFinding>,
    detail: Value,
) -> DoctorSection {
    DoctorSection {
        id,
        source: source.to_owned(),
        findings,
        detail,
    }
}

pub(super) fn environment(options: &DoctorOptions) -> DoctorSection {
    let detection = NetworkDetection::detect(
        &options.root,
        options.env.get("WORKBENCH_NETWORK_FS").map(String::as_str),
    );
    let findings = vec![
        DoctorFinding::healthy(
            "environment",
            format!(
                "filesystem {} (via {}); engine native (supervised worker)",
                if detection.network_fs {
                    "network"
                } else {
                    "local"
                },
                detection.method
            ),
        ),
        DoctorFinding::healthy(
            "watcher",
            "watcher status is in memory: a live composition reports it through status; no watcher record is published to disk",
        ),
    ];
    let detail = json!({
        "networkFs": { "detected": detection.network_fs, "method": detection.method, "fstype": detection.fstype },
        "expectedJournalMode": detection.journal_mode(),
        "engine": { "native": true, "worker": "supervised native worker (magistr --workspace-worker)" },
        "watcher": { "state": "in-memory" },
    });
    section(
        DoctorSectionId::Environment,
        "filesystem detection + native worker role + in-memory watcher status",
        findings,
        detail,
    )
}

struct IndexProbe {
    user_version: i64,
    quick_check: String,
    generation: Option<i64>,
    tier: Option<i64>,
    files: Option<i64>,
    symbols: Option<i64>,
}

fn probe_index(path: &Path) -> Result<IndexProbe, rusqlite::Error> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    db.busy_timeout(Duration::from_millis(100))?;
    let user_version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if user_version > NATIVE_INDEX_SCHEMA {
        return Ok(IndexProbe {
            user_version,
            quick_check: "skipped".into(),
            generation: None,
            tier: None,
            files: None,
            symbols: None,
        });
    }
    let mut statement = db.prepare("PRAGMA quick_check")?;
    let quick_check = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?
        .join("; ");
    let meta = |key: &str| -> Option<i64> {
        db.query_row("SELECT value FROM meta WHERE key=?", [key], |row| {
            row.get(0)
        })
        .ok()
    };
    let count = |table: &str| -> Option<i64> {
        db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .ok()
    };
    Ok(IndexProbe {
        user_version,
        quick_check,
        generation: meta("generation"),
        tier: meta("tier"),
        files: count("files"),
        symbols: count("symbols"),
    })
}

pub(super) fn index(state_dir: &Path) -> DoctorSection {
    let source = ".workbench/native-index.db (read-only; native-index.lock is never taken)";
    // Resolve legitimate ancestor aliases (macOS /var) the way the cache
    // does before asking SQLite to refuse symlinks in the database path;
    // canonicalizing never creates anything.
    let state_dir = state_dir
        .canonicalize()
        .unwrap_or_else(|_| state_dir.to_path_buf());
    let path = state_dir.join("native-index.db");
    if !path.exists() {
        return section(
            DoctorSectionId::Index,
            source,
            vec![DoctorFinding::info(
                "native-index.db",
                DoctorDataClass::Cache,
                "no native index yet (fresh workspace): Tier-0 lexical answers only until a composition builds it",
            )],
            json!({ "present": false }),
        );
    }
    match probe_index(&path) {
        Err(error) => section(
            DoctorSectionId::Index,
            source,
            vec![DoctorFinding::unhealthy(
                "native-index.db",
                DoctorDataClass::Cache,
                format!("unreadable as a database: {error}"),
                REBUILD_INDEX,
            )],
            json!({ "present": true }),
        ),
        Ok(probe) if probe.user_version > NATIVE_INDEX_SCHEMA => section(
            DoctorSectionId::Index,
            source,
            vec![DoctorFinding::unhealthy(
                "native-index.db",
                DoctorDataClass::Cache,
                format!(
                    "index schema {} is newer than this binary supports ({NATIVE_INDEX_SCHEMA})",
                    probe.user_version
                ),
                "upgrade magistr; a newer binary wrote this index and this one refuses to misread it (SDD §10.2)",
            )],
            json!({ "present": true, "userVersion": probe.user_version }),
        ),
        Ok(probe) => {
            let detail = json!({
                "present": true,
                "userVersion": probe.user_version,
                "generation": probe.generation,
                "tier": probe.tier,
                "fileCount": probe.files,
                "symbolCount": probe.symbols,
                "quickCheck": probe.quick_check,
            });
            let finding = if probe.quick_check == "ok" {
                DoctorFinding::healthy(
                    "native-index.db",
                    format!("schema {}, quick_check ok", probe.user_version),
                )
            } else {
                DoctorFinding::unhealthy(
                    "native-index.db",
                    DoctorDataClass::Cache,
                    format!("integrity check failed: {}", probe.quick_check),
                    REBUILD_INDEX,
                )
            };
            section(DoctorSectionId::Index, source, vec![finding], detail)
        }
    }
}

pub(super) fn config(options: &DoctorOptions) -> (DoctorSection, String) {
    let source =
        "five-layer resolution (defaults → org → user → workbench.toml → session), SDD §7.2";
    let resolve = ResolveOptions {
        root: options.root.clone(),
        env: options.env.clone(),
        session_flags: options.session_flags.clone(),
    };
    match ResolvedConfig::resolve(&resolve) {
        Ok(config) => {
            let text = config.doctor(&options.root);
            let detail = json!({
                "trace": config.trace,
                "warnings": config.warnings,
                "violations": config.violations,
            });
            let finding = DoctorFinding::healthy(
                "config",
                format!(
                    "{} keys resolved; {} warning(s)",
                    config.trace.len(),
                    config.warnings.len()
                ),
            );
            (
                section(DoctorSectionId::Config, source, vec![finding], detail),
                text,
            )
        }
        Err(error) => {
            // A config layer from a newer binary is a finding in the full report.
            let remediation = if error.kind == ErrorKind::UnsupportedVersion {
                "upgrade magistr; a config layer was written by a newer binary and this one fails closed rather than misreading it (SDD §10.2)"
            } else {
                "fix the named config layer file; resolution failed before any key was decided"
            };
            let finding = DoctorFinding::unhealthy(
                "config",
                DoctorDataClass::Durable,
                error.message,
                remediation,
            );
            (
                section(DoctorSectionId::Config, source, vec![finding], json!({})),
                String::new(),
            )
        }
    }
}

#[cfg(feature = "sandbox")]
pub(super) fn plugins(root: &Path, state_dir: &Path) -> DoctorSection {
    use crate::extensions::{PluginHealthRecord, PluginInventory};
    let source = ".workbench/plugins.json + .workbench/plugin-health.json";
    let mut findings = Vec::new();
    let mut inventory = Vec::new();
    match PluginInventory::load(state_dir) {
        Ok(records) => {
            for record in &records {
                let manifest_present = record
                    .dir
                    .as_ref()
                    .map(|dir| root.join(dir).join("plugin.toml").exists());
                if manifest_present == Some(false) {
                    findings.push(DoctorFinding::unhealthy(
                        "plugins.json",
                        DoctorDataClass::Durable,
                        format!(
                            "plugin {}: inventory points at {} but no plugin.toml is there",
                            record.name,
                            record.dir.as_deref().unwrap_or("?")
                        ),
                        "reinstall the plugin, or remove its entry from .workbench/plugins.json",
                    ));
                }
                inventory.push(json!({
                    "name": record.name,
                    "trust": record.tier.map_or(record.source.as_str(), |tier| tier.as_str()),
                    "source": record.source.as_str(),
                    "dir": record.dir,
                    "manifestPresent": manifest_present,
                }));
            }
        }
        Err(error) => findings.push(DoctorFinding::unhealthy(
            "plugins.json",
            DoctorDataClass::Durable,
            format!("unreadable inventory: {error}"),
            "fix or remove .workbench/plugins.json (durable data: set it aside rather than deleting if unsure)",
        )),
    }
    let mut unhealthy: Vec<PluginHealthRecord> = Vec::new();
    let health_path = state_dir.join("plugin-health.json");
    if health_path.exists() {
        let parsed = std::fs::read_to_string(&health_path)
            .map_err(|error| error.to_string())
            .and_then(|raw| serde_json::from_str::<Value>(&raw).map_err(|error| error.to_string()));
        match parsed {
            Ok(parsed) => {
                for entry in parsed.get("breaches").and_then(Value::as_array).into_iter().flatten() {
                    if let Ok(breach) = serde_json::from_value::<PluginHealthRecord>(entry.clone()) {
                        findings.push(DoctorFinding::unhealthy(
                            "plugin-health.json",
                            DoctorDataClass::Durable,
                            format!(
                                "plugin {} breakered since {}: {}",
                                breach.plugin, breach.at, breach.reason
                            ),
                            "update or reinstall the plugin — a changed component hash clears the breaker (SDD §8.1)",
                        ));
                        unhealthy.push(breach);
                    }
                }
            }
            Err(error) => findings.push(DoctorFinding::unhealthy(
                "plugin-health.json",
                DoctorDataClass::Durable,
                format!("unreadable: {error}"),
                "set .workbench/plugin-health.json aside; the breaker record rebuilds on the next breach",
            )),
        }
    }
    if findings.is_empty() {
        findings.push(DoctorFinding::healthy(
            "plugins",
            format!(
                "{} installed plugin(s); no breaker records",
                inventory.len()
            ),
        ));
    }
    section(
        DoctorSectionId::Plugins,
        source,
        findings,
        json!({ "inventory": inventory, "unhealthy": unhealthy }),
    )
}

#[cfg(not(feature = "sandbox"))]
pub(super) fn plugins(_root: &Path, _state_dir: &Path) -> DoctorSection {
    section(
        DoctorSectionId::Plugins,
        "extension host not compiled into this build (sandbox feature off)",
        vec![DoctorFinding::info(
            "plugins",
            DoctorDataClass::NotApplicable,
            "extension host not compiled into this build; plugins.json and plugin-health.json were not inspected",
        )],
        json!({ "inventory": [], "unhealthy": [], "compiled": false }),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct QuarantineEntry {
    path: String,
    strikes: u64,
    quarantined: bool,
    last_crash: String,
    at: u64,
}

pub(super) fn quarantine(state_dir: &Path) -> (DoctorSection, Vec<QuarantineEntry>) {
    let source = ".workbench/quarantine.jsonl (retained durable record; the native worker's own ledger is per-session, in memory)";
    let path = state_dir.join("quarantine.jsonl");
    if !path.exists() {
        return (
            section(
                DoctorSectionId::Quarantine,
                source,
                vec![DoctorFinding::healthy(
                    "quarantine.jsonl",
                    "no quarantine records",
                )],
                json!({ "entries": [], "malformedLines": 0 }),
            ),
            Vec::new(),
        );
    }
    let mut entries = Vec::new();
    let mut malformed_lines = 0;
    for line in std::fs::read_to_string(&path)
        .unwrap_or_default()
        .split('\n')
    {
        if line.trim().is_empty() {
            continue;
        }
        // Append-only logs can tear; a bad line is counted, never fatal.
        match serde_json::from_str::<Value>(line) {
            Ok(parsed) if parsed.get("path").and_then(Value::as_str).is_some() => {
                entries.push(QuarantineEntry {
                    path: parsed["path"].as_str().unwrap_or_default().to_owned(),
                    strikes: parsed.get("strikes").and_then(Value::as_u64).unwrap_or(0),
                    quarantined: parsed.get("quarantined").and_then(Value::as_bool) == Some(true),
                    last_crash: parsed
                        .get("lastCrash")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_owned(),
                    at: parsed.get("at").and_then(Value::as_u64).unwrap_or(0),
                });
            }
            _ => malformed_lines += 1,
        }
    }
    let mut findings: Vec<DoctorFinding> = entries
        .iter()
        .filter(|entry| entry.quarantined)
        .map(|entry| {
            DoctorFinding::unhealthy(
                "quarantine.jsonl",
                DoctorDataClass::NotApplicable,
                format!(
                    "{} quarantined after {} strike(s); last crash: {}",
                    entry.path, entry.strikes, entry.last_crash
                ),
                "inspect, fix, or remove the file — INPUT_QUARANTINED clears when the file changes (SDD §6.2)",
            )
        })
        .collect();
    if findings.is_empty() {
        findings.push(DoctorFinding::healthy(
            "quarantine.jsonl",
            format!("{} record(s), none currently quarantined", entries.len()),
        ));
    }
    let detail = json!({ "entries": entries, "malformedLines": malformed_lines });
    (
        section(DoctorSectionId::Quarantine, source, findings, detail),
        entries,
    )
}

pub(super) fn crashes(entries: &[QuarantineEntry]) -> DoctorSection {
    // Honest sourcing: quarantine strikes are the only persisted crash
    // evidence; the worker's crash ledger is in memory and per session.
    let evidence: Vec<Value> = entries
        .iter()
        .map(|entry| json!({ "path": entry.path, "lastCrash": entry.last_crash, "at": entry.at }))
        .collect();
    let finding = if evidence.is_empty() {
        DoctorFinding::healthy("crashes", "no engine-crash evidence recorded")
    } else {
        DoctorFinding::info(
            "crashes",
            DoctorDataClass::NotApplicable,
            format!(
                "{} crash strike record(s); see quarantine section for the affected inputs",
                evidence.len()
            ),
        )
    };
    section(
        DoctorSectionId::Crashes,
        "derived from .workbench/quarantine.jsonl strikes (no standalone crash ledger on disk)",
        vec![finding],
        json!({
            "evidence": evidence,
            "note": "crash evidence is derived from quarantine strikes; the native worker's ledger is in memory and per session",
        }),
    )
}
