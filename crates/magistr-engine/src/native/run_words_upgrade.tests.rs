use super::*;
use crate::native::{NativeReader, ProjectAuthority, WriterRole};
use serde_json::Value;

/// Real records the previous writer saved for this project, trimmed: a run
/// document (version 2: `objective`, `tasks`, `actions`, `blocked`,
/// `completed`), one check result targeting a task, plan chunks referencing
/// `task:` and `action:`, and decision notes whose `execution_ref` uses them.
const RUN: &[u8] = include_bytes!("fixtures/before-run-words-run.json");
const CHECK_RESULT: &[u8] = include_bytes!("fixtures/before-run-words-check-result.json");
const PLAN: &[u8] = include_bytes!("fixtures/before-run-words-roadmap.json");
const DECISIONS: &[u8] = include_bytes!("fixtures/before-run-words-think.json");
const PROJECT: &str = "magistr-5a5bc6";
const EXECUTION: &str = "execution-key";

fn store() -> (tempfile::TempDir, NativeConfig) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir_all(root.join(".magistr")).unwrap();
    fs::write(
        root.join(".magistr/project.json"),
        format!(r#"{{"project":{{"id":"{PROJECT}"}}}}"#),
    )
    .unwrap();
    let config =
        NativeConfig::from_lookup(&root, |_| None).with_data_dir(dir.path().join("data/workflow"));
    let data = config.data_dir().to_path_buf();
    let check_result_id = json(CHECK_RESULT)["id"].as_str().unwrap().to_owned();
    for (relative, bytes) in [
        (format!("ship/sessions/{PROJECT}.json"), RUN),
        (
            format!("ship/executions/{EXECUTION}/sessions/{PROJECT}.json"),
            RUN,
        ),
        (
            format!("ship/executions/{EXECUTION}/receipts/{PROJECT}/{check_result_id}.json"),
            CHECK_RESULT,
        ),
        (
            format!(
                "ship/executions/{EXECUTION}/receipts/{PROJECT}/runs-v1/run/{check_result_id}.ref"
            ),
            b"".as_slice(),
        ),
        (format!("ship/sessions/{PROJECT}.json.lock"), b"".as_slice()),
        (format!("roadmap/sessions/{PROJECT}.json"), PLAN),
        (format!("think/sessions/{PROJECT}.json"), DECISIONS),
    ] {
        let path = data.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    (dir, config)
}

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

fn read(path: PathBuf) -> Value {
    json(&fs::read(path).unwrap())
}

#[test]
fn the_run_store_moves_once_and_keeps_its_original() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    RunWordsUpgrade::apply(&config, PROJECT).unwrap();

    assert!(!data.join("ship").exists());
    let run = read(data.join(format!(
        "run/executions/{EXECUTION}/sessions/{PROJECT}.json"
    )));
    assert_eq!(run["schema_version"], 3);
    assert_eq!(run["goal"]["status"], "finished");
    assert!(run.get("objective").is_none() && run.get("tasks").is_none());
    assert_eq!(run["steps"][0]["status"], "done");
    let saved = run["saved_cycles"][0]["steps"].as_array().unwrap();
    assert!(saved.iter().any(|step| step["status"] == "waiting"));
    assert_eq!(
        read(data.join(format!("run/sessions/{PROJECT}.json"))),
        run,
        "both copies of the document take the same words"
    );

    let check_dir = data.join(format!(
        "run/executions/{EXECUTION}/check_results/{PROJECT}"
    ));
    let id = json(CHECK_RESULT)["id"].as_str().unwrap().to_owned();
    let check = read(check_dir.join(format!("{id}.json")));
    assert_eq!(
        check["target"]["step_id"],
        json(CHECK_RESULT)["target"]["task_id"]
    );
    assert!(check_dir.join(format!("runs-v1/run/{id}.ref")).exists());
    assert!(
        !data
            .join(format!("run/sessions/{PROJECT}.json.lock"))
            .exists(),
        "lock files are not records"
    );

    let backups: Vec<_> = fs::read_dir(data.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ship-before-run-words-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::read(backups[0].join(format!("sessions/{PROJECT}.json"))).unwrap(),
        RUN,
        "the original is recoverable byte for byte"
    );

    let after = fs::read(data.join(format!("run/sessions/{PROJECT}.json"))).unwrap();
    RunWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert_eq!(
        fs::read(data.join(format!("run/sessions/{PROJECT}.json"))).unwrap(),
        after,
        "a second pass changes nothing"
    );
}

#[test]
fn plan_and_decision_references_take_the_run_prefixes() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    RunWordsUpgrade::apply(&config, PROJECT).unwrap();

    let plan = read(data.join(format!("roadmap/sessions/{PROJECT}.json")));
    let refs: Vec<&str> = plan["chunks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|chunk| chunk["cross_refs"].as_array().unwrap())
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(refs.contains(&"step:verify-gates"), "{refs:?}");
    assert!(refs.contains(&"progress:4"), "{refs:?}");
    assert!(refs.contains(&"think:834"), "{refs:?}");
    assert!(
        !refs
            .iter()
            .any(|r| r.starts_with("task:") || r.starts_with("action:"))
    );

    let decisions = read(data.join(format!("think/sessions/{PROJECT}.json")));
    assert_eq!(decisions["schema_version"], 3);
    let execution_refs: Vec<&str> = decisions["histories"][PROJECT]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step["execution_ref"].as_str())
        .collect();
    assert_eq!(
        execution_refs,
        ["progress:1", "step:ts1-native", "objective:shipped"],
        "recorded free text that is not a reference keeps its words"
    );

    for (family, original) in [("roadmap", PLAN), ("think", DECISIONS)] {
        assert_eq!(
            fs::read(data.join(format!("backups/before-run-words/{family}/{PROJECT}.json")))
                .unwrap(),
            original
        );
    }
    let converted = fs::read(data.join(format!("think/sessions/{PROJECT}.json"))).unwrap();
    RunWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert_eq!(
        fs::read(data.join(format!("think/sessions/{PROJECT}.json"))).unwrap(),
        converted
    );
}

#[test]
fn readers_wait_for_the_upgrade_and_admission_finishes_it() {
    let (_dir, config) = store();
    let reader = NativeReader::open(&config);
    let refused = reader.roadmap_chunks().unwrap_err();
    assert!(refused.contains("one-time upgrade"), "{refused}");
    assert!(
        config.data_dir().join("ship").exists(),
        "a reader never writes"
    );

    let owner = ProjectAuthority::admit(&config, WriterRole::Host).unwrap();
    let reader = NativeReader::open(&config);
    assert_eq!(reader.roadmap_chunks().unwrap().len(), 2);
    drop(owner);
}

#[test]
fn an_interrupted_move_is_rebuilt_from_the_original() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let staging = data.join(STAGING_DIR).join("sessions");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join(format!("{PROJECT}.json")), b"half-written").unwrap();
    fs::create_dir_all(data.join("run/sessions")).unwrap();

    RunWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join(STAGING_DIR).exists());
    assert_eq!(
        read(data.join(format!("run/sessions/{PROJECT}.json")))["schema_version"],
        3
    );
}

#[test]
fn records_already_in_the_new_store_are_never_overwritten() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let current = data.join(format!("run/sessions/{PROJECT}.json"));
    fs::create_dir_all(current.parent().unwrap()).unwrap();
    fs::write(&current, b"newer work").unwrap();

    assert!(RunWordsUpgrade::apply(&config, PROJECT).is_err());
    assert_eq!(fs::read(&current).unwrap(), b"newer work");
    assert_eq!(
        fs::read(data.join(format!("ship/sessions/{PROJECT}.json"))).unwrap(),
        RUN
    );
}

#[test]
fn a_live_owner_of_another_project_holds_the_move_back() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let lock_path = data.join("roadmap/sessions/other-project.project-owner.lock");
    let held = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();
    let refused = RunWordsUpgrade::apply(&config, PROJECT).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::WouldBlock);
    assert!(refused.to_string().contains("other-project"), "{refused}");
    assert!(data.join("ship").exists() && !data.join("run").exists());

    drop(held);
    RunWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("ship").exists());
}
