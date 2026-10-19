use std::{collections::BTreeMap, io::Read, path::Path};

use super::*;
use crate::{diagnostics::DoctorOptions, index::cache::IndexCache};

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::write(dir.path().join("a.ts"), "export const a = 1;\n").expect("source");
    dir
}

fn seeded_run(root: &Path) -> DoctorRun {
    let state = root.join(".magistr/workspace");
    std::fs::create_dir_all(&state).expect("state dir");
    drop(IndexCache::open(&state, "WAL").expect("native index"));
    std::fs::write(
        state.join("quarantine.jsonl"),
        format!(
            "{}\n",
            serde_json::json!({ "path": "weird.bin", "strikes": 3, "quarantined": true, "lastCrash": "SIGSEGV", "at": 1755000000000_u64 })
        ),
    )
    .expect("quarantine");
    DoctorRun::observe(
        &DoctorOptions::new(root)
            .with_tool_version("0.0.0-test")
            .with_now_unix_ms(1_787_356_800_000),
    )
}

/// Independent minimal ustar reader — deliberately NOT the writer's inverse.
fn read_tar(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut members = BTreeMap::new();
    let mut offset = 0;
    while offset + 512 <= bytes.len() {
        let header = &bytes[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            break;
        }
        let field = |from: usize, len: usize| -> String {
            let raw = &header[from..from + len];
            let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
            String::from_utf8_lossy(&raw[..end]).into_owned()
        };
        let name = field(0, 100);
        let size = usize::from_str_radix(field(124, 12).trim(), 8).expect("size");
        let stored = u64::from_str_radix(field(148, 8).trim_end_matches(' '), 8).expect("checksum");
        let sum: u64 = header
            .iter()
            .enumerate()
            .map(|(index, byte)| {
                if (148..156).contains(&index) {
                    0x20
                } else {
                    u64::from(*byte)
                }
            })
            .sum();
        assert_eq!(sum, stored, "checksum of {name}");
        assert_eq!(&header[257..262], b"ustar");
        members.insert(name, bytes[offset + 512..offset + 512 + size].to_vec());
        offset += 512 + size.div_ceil(512) * 512;
    }
    members
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .expect("gzip");
    out
}

#[test]
fn ustar_conformance_via_an_independent_reader_and_the_manifest_lists_every_member() {
    let dir = workspace();
    let bundle = DoctorBundle::build(&seeded_run(dir.path())).expect("bundle");
    let members = read_tar(&gunzip(&bundle.bytes));
    let mut names: Vec<_> = members.keys().cloned().collect();
    names.sort();
    let mut listed = bundle.members.clone();
    listed.sort();
    assert_eq!(names, listed);
    let manifest: serde_json::Value = serde_json::from_slice(
        members
            .get("workspace-doctor/manifest.json")
            .expect("manifest"),
    )
    .expect("json");
    assert_eq!(manifest["bundleFormatVersion"], 1);
    assert_eq!(manifest["workspaceLabel"], "<workspace>");
    let mut manifest_members: Vec<_> = manifest["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| member.as_str().unwrap_or_default().to_owned())
        .collect();
    manifest_members.sort();
    assert_eq!(names, manifest_members);
    assert!(names.contains(&"workspace-doctor/index.json".to_owned()));
    assert!(names.contains(&"workspace-doctor/report.txt".to_owned()));
}

#[test]
fn privacy_a_sentinel_never_leaks_and_the_workspace_root_is_redacted() {
    let dir = workspace();
    let sentinel = "SENTINEL-do-not-leak-9f3a";
    std::fs::write(
        dir.path().join("secret.ts"),
        format!("export const s = \"{sentinel}\";\n"),
    )
    .expect("secret");
    let bundle = DoctorBundle::build(&seeded_run(dir.path())).expect("bundle");
    let members = read_tar(&gunzip(&bundle.bytes));
    let root = dir.path().display().to_string();
    for (name, data) in &members {
        let body = String::from_utf8_lossy(data);
        assert!(!body.contains(sentinel), "{name} leaks the sentinel");
        assert!(!body.contains(&root), "{name} leaks the workspace root");
        assert!(
            !body.contains(&root.replace('\\', "/")),
            "{name} leaks the root"
        );
        assert!(name.starts_with("workspace-doctor/"));
    }
    let report =
        String::from_utf8_lossy(members.get("workspace-doctor/report.txt").expect("report"));
    assert!(report.contains("<workspace>"));
}

#[test]
fn a_support_engineer_can_derive_the_remediation_from_the_members_alone() {
    let dir = workspace();
    let state = dir.path().join(".magistr/workspace");
    std::fs::create_dir_all(&state).expect("state dir");
    std::fs::write(state.join("journal.db"), "durable-user-data-journal").expect("durable");
    std::fs::write(
        state.join("native-index.db"),
        "garbage-not-a-sqlite-database",
    )
    .expect("corrupt");
    let run = DoctorRun::observe(&DoctorOptions::new(dir.path()));
    assert!(run.report.findings_present);
    let bundle = DoctorBundle::build(&run).expect("bundle");
    let members = read_tar(&gunzip(&bundle.bytes));
    // ---- Support-engineer boundary: from here, ONLY the bundle. ----
    let index: serde_json::Value = serde_json::from_slice(
        members
            .get("workspace-doctor/index.json")
            .expect("index member"),
    )
    .expect("json");
    let finding = index["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|finding| finding["severity"] == "unhealthy")
        .expect("an unhealthy finding");
    assert_eq!(finding["store"], "native-index.db");
    assert_eq!(finding["dataClass"], "cache");
    let remediation = finding["remediation"].as_str().expect("remediation");
    assert!(remediation.contains("rebuild"));
    let store = remediation
        .split_whitespace()
        .find(|word| word.starts_with(".magistr/workspace/"))
        .expect("the store path");
    // ---- End boundary: the user applies the instruction locally. ----
    assert_eq!(store, ".magistr/workspace/native-index.db");
    std::fs::remove_file(dir.path().join(store)).expect("apply remediation");
    let after = DoctorRun::observe(&DoctorOptions::new(dir.path()));
    assert!(!after.report.findings_present);
    assert!(after.render().contains("no native index yet"));
    assert_eq!(
        std::fs::read(state.join("journal.db")).expect("durable"),
        b"durable-user-data-journal"
    );
}

#[test]
fn redact_reduces_foreign_absolute_paths_to_placeholders() {
    let out = redact_bundle_text(
        "seen at C:\\Users\\someone\\secrets\\token.txt and /home/someone/key.pem inside <ws>\"",
        "D:\\nowhere\\ws",
    );
    assert!(out.contains("<outside-workspace>/token.txt"), "{out}");
    assert!(out.contains("<outside-workspace>/key.pem"), "{out}");
    assert!(!out.contains("C:\\Users\\someone"));
    assert!(!out.contains("/home/someone"));
    assert_eq!(
        redact_bundle_text(
            "store .magistr/workspace/index.db under /tmp/ws/.magistr/workspace",
            "/tmp/ws"
        ),
        "store .magistr/workspace/index.db under <workspace>/.magistr/workspace"
    );
}

#[test]
fn the_tar_writer_round_trips_arbitrary_member_bytes() {
    let data: Vec<u8> = (0..700u32).map(|index| (index % 251) as u8).collect();
    let archive = tar_writer::tar_archive(
        &[tar_writer::TarMember {
            name: "workspace-doctor/x.bin".into(),
            data: data.clone(),
        }],
        1_700_000_000,
    )
    .expect("archive");
    let members = read_tar(&archive);
    assert_eq!(members.get("workspace-doctor/x.bin"), Some(&data));
    assert_eq!(archive.len() % 512, 0);
}
