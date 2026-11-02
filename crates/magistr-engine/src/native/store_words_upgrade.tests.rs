use super::*;
use crate::native::{NativeReader, ProjectAuthority, WriterRole};
use serde_json::Value;

/// Real records an earlier writer saved for this project, trimmed: a run
/// document (version 2: `objective`, `tasks`, `actions`, `blocked`,
/// `completed`), one check result targeting a task, plan chunks referencing
/// `task:` and `action:`, and decision notes whose `execution_ref` uses them.
const RUN: &[u8] = include_bytes!("fixtures/before-run-words-run.json");
const CHECK_RESULT: &[u8] = include_bytes!("fixtures/before-run-words-check-result.json");
const PLAN: &[u8] = include_bytes!("fixtures/before-run-words-roadmap.json");
const DECISIONS: &[u8] = include_bytes!("fixtures/before-run-words-think.json");
/// Real plan records saved before the plan words, trimmed: chunks in every
/// stored state with groups, blockers, obsoleted reasons, `chunk:`
/// references and a lane's focus. The signal keeps a real record's shape and
/// references; its free text is replaced because it came from another project.
const PLAN_WORDS: &[u8] = include_bytes!("fixtures/before-plan-words-plan.json");
const SIGNALS: &[u8] = include_bytes!("fixtures/before-plan-words-signal.json");
const PROJECT: &str = "magistr-5a5bc6";
const EXECUTION: &str = "execution-key";

fn store() -> (tempfile::TempDir, NativeConfig) {
    store_with(PLAN)
}

fn store_with(plan: &[u8]) -> (tempfile::TempDir, NativeConfig) {
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
        (format!("roadmap/sessions/{PROJECT}.json"), plan),
        (format!("think/sessions/{PROJECT}.json"), DECISIONS),
        (format!("signal/sessions/{PROJECT}.json"), SIGNALS),
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
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();

    assert!(!data.join("ship").exists());
    let run = read(data.join(format!(
        "run/executions/{EXECUTION}/sessions/{PROJECT}.json"
    )));
    assert_eq!(run["schema_version"], 4);
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
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert_eq!(
        fs::read(data.join(format!("run/sessions/{PROJECT}.json"))).unwrap(),
        after,
        "a second pass changes nothing"
    );
}

#[test]
fn plan_and_decision_references_saved_before_the_run_words_take_both_vocabularies() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();

    assert!(!data.join("roadmap").exists());
    let plan = read(data.join(format!("plan/sessions/{PROJECT}.json")));
    assert_eq!(plan["schema_version"], 2);
    let refs: Vec<&str> = plan["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|task| task["refs"].as_array().unwrap())
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
    assert_eq!(decisions["schema_version"], 4);
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

    assert_eq!(
        fs::read(data.join(format!(
            "backups/before-plan-words/think/sessions/{PROJECT}.json"
        )))
        .unwrap(),
        DECISIONS
    );
    let moved = backup_of(&data, "roadmap-before-plan-words-");
    assert_eq!(
        fs::read(moved.join(format!("sessions/{PROJECT}.json"))).unwrap(),
        PLAN,
        "the original plan store is recoverable byte for byte"
    );
    let converted = fs::read(data.join(format!("think/sessions/{PROJECT}.json"))).unwrap();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert_eq!(
        fs::read(data.join(format!("think/sessions/{PROJECT}.json"))).unwrap(),
        converted
    );
}

#[test]
fn the_plan_store_moves_once_and_takes_the_plan_words() {
    let (_dir, config) = store_with(PLAN_WORDS);
    let data = config.data_dir().to_path_buf();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();

    let plan = read(data.join(format!("plan/sessions/{PROJECT}.json")));
    let original = json(PLAN_WORDS);
    assert_eq!(plan["schema_version"], 2);
    assert!(plan.get("chunks").is_none());
    let tasks = plan["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), original["chunks"].as_array().unwrap().len());
    let status = |id: &str| {
        tasks.iter().find(|t| t["id"] == id).unwrap()["status"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(status("audit-260912-36-system-contracts"), "idea");
    assert_eq!(status("audit-260912-41-reference-floor"), "to_do");
    assert_eq!(status("p2"), "retired");
    assert_eq!(status("e0"), "done");
    for (id, reason) in [
        ("ts3b-gui-techtree-chunks", "person"),
        ("acp-protocol-v2", "outside"),
    ] {
        let task = tasks.iter().find(|t| t["id"] == id).unwrap();
        assert_eq!(task["waiting_on"]["kind"], reason, "{id}");
        assert!(task["waiting_on"]["waiting_since"].is_string(), "{id}");
        assert!(task.get("blocked_by").is_none(), "{id}");
    }
    let retired = tasks.iter().find(|t| t["id"] == "p2").unwrap();
    assert!(retired["retired_reason"].is_string());
    assert!(retired.get("obsoleted_reason").is_none());
    let floor = tasks
        .iter()
        .find(|t| t["id"] == "audit-260912-41-reference-floor")
        .unwrap();
    assert_eq!(floor["feature"], "Workspace capabilities");
    assert!(floor.get("group").is_none());
    let refs: Vec<&str> = tasks
        .iter()
        .flat_map(|task| task["refs"].as_array().unwrap())
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(refs.contains(&"task:arch-14c10b-workspace-port-acceptance"));
    assert!(!refs.iter().any(|r| r.starts_with("chunk:")), "{refs:?}");
    assert_eq!(
        plan["focuses"][0]["feature"],
        original["focuses"][0]["group"]
    );

    let signals = read(data.join(format!("signal/sessions/{PROJECT}.json")));
    assert_eq!(signals["schema_version"], 2);
    assert_eq!(
        signals["signals"][0]["refs"],
        serde_json::json!(["task:net-prediction", "think:56", "think:57", "think:58"])
    );
    assert_eq!(
        fs::read(data.join(format!(
            "backups/before-plan-words/signal/sessions/{PROJECT}.json"
        )))
        .unwrap(),
        SIGNALS
    );

    // Converted plan tasks now spell `task:`. A second admission must leave
    // them alone, not read them as run steps from before the run words.
    let after = fs::read(data.join(format!("plan/sessions/{PROJECT}.json"))).unwrap();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert_eq!(
        fs::read(data.join(format!("plan/sessions/{PROJECT}.json"))).unwrap(),
        after,
        "a second pass changes nothing"
    );
}

fn backup_of(data: &Path, prefix: &str) -> PathBuf {
    let backups: Vec<_> = fs::read_dir(data.join("backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    assert_eq!(backups.len(), 1, "{prefix}");
    backups[0].clone()
}

#[test]
fn readers_wait_for_the_upgrade_and_admission_finishes_it() {
    let (_dir, config) = store();
    let reader = NativeReader::open(&config);
    let refused = reader.plan_tasks().unwrap_err();
    assert!(refused.contains("one-time upgrade"), "{refused}");
    assert!(
        config.data_dir().join("ship").exists(),
        "a reader never writes"
    );

    let owner = ProjectAuthority::admit(&config, WriterRole::Host).unwrap();
    let reader = NativeReader::open(&config);
    assert_eq!(reader.plan_tasks().unwrap().len(), 2);
    drop(owner);
}

#[test]
fn an_interrupted_move_keeps_exact_staged_records_and_rebuilds_the_rest() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let staging = data.join("run.upgrading").join("sessions");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join(format!("{PROJECT}.json")), b"half-written").unwrap();
    // An interrupted write's leftover is never published.
    fs::write(staging.join(".staging-abc123"), b"partial").unwrap();
    fs::create_dir_all(data.join("run/sessions")).unwrap();

    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("run.upgrading").exists());
    assert!(!data.join("run/sessions/.staging-abc123").exists());
    assert_eq!(
        read(data.join(format!("run/sessions/{PROJECT}.json")))["schema_version"],
        4
    );
}

#[test]
fn records_already_in_the_new_store_are_never_overwritten() {
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let current = data.join(format!("run/sessions/{PROJECT}.json"));
    fs::create_dir_all(current.parent().unwrap()).unwrap();
    fs::write(&current, b"newer work").unwrap();

    assert!(StoreWordsUpgrade::apply(&config, PROJECT).is_err());
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
    let refused = StoreWordsUpgrade::apply(&config, PROJECT).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::WouldBlock);
    assert!(refused.to_string().contains("other-project"), "{refused}");
    assert!(data.join("ship").exists() && !data.join("run").exists());
    assert!(data.join("roadmap").exists() && !data.join("plan").exists());

    drop(held);
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("ship").exists() && !data.join("roadmap").exists());
}

#[test]
fn an_older_owner_of_this_project_also_holds_the_move_back() {
    // Owners older than the plan words lock under the old plan directory,
    // a different file from the caller's own lease.
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let held = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join(format!("roadmap/sessions/{PROJECT}.project-owner.lock")))
        .unwrap();
    held.lock().unwrap();
    let refused = StoreWordsUpgrade::apply(&config, PROJECT).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::WouldBlock);
    assert!(data.join("ship").exists());
}

#[test]
fn a_partly_published_plan_move_finishes_on_retry() {
    let (_dir, config) = store_with(PLAN_WORDS);
    let data = config.data_dir().to_path_buf();
    // The first attempt published the converted plan, then stopped before
    // the original moved to the backups.
    let converted =
        magistr_workflow::plan::PlanEngine::upgrade_document(PLAN_WORDS, PROJECT).unwrap();
    let published = data.join(format!("plan/sessions/{PROJECT}.json"));
    fs::create_dir_all(published.parent().unwrap()).unwrap();
    fs::write(&published, &converted).unwrap();

    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("roadmap").exists());
    assert_eq!(fs::read(&published).unwrap(), converted);
}

#[test]
fn an_old_directory_without_records_is_not_waiting_for_anything() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir_all(root.join(".magistr")).unwrap();
    let config =
        NativeConfig::from_lookup(&root, |_| None).with_data_dir(dir.path().join("data/workflow"));
    let data = config.data_dir().to_path_buf();
    // An earlier build opened the store: directories and a lock, no records.
    fs::create_dir_all(data.join("roadmap/sessions")).unwrap();
    fs::write(data.join("roadmap/sessions/other.project-owner.lock"), b"").unwrap();
    StoreWordsUpgrade::ready(&config, PROJECT).unwrap();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("roadmap").exists());
}

#[cfg(unix)]
#[test]
fn owner_endpoint_sockets_are_not_records() {
    // Real stores keep each owner's endpoint socket beside its lock.
    let (_dir, config) = store();
    let data = config.data_dir().to_path_buf();
    let socket = data.join(format!(
        "roadmap/sessions/{PROJECT}.project-owner.lock.sock"
    ));
    // Socket paths are short; bind nearby, then move the socket in place.
    let short = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
    let bound = short.path().join("s");
    let _listener = std::os::unix::net::UnixListener::bind(&bound).unwrap();
    fs::rename(&bound, &socket).unwrap();
    StoreWordsUpgrade::apply(&config, PROJECT).unwrap();
    assert!(!data.join("roadmap").exists());
    assert!(
        !data
            .join(format!("plan/sessions/{PROJECT}.project-owner.lock.sock"))
            .exists()
    );
}
