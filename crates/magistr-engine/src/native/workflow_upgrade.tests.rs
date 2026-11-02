use super::*;
use crate::native::{NativeReader, ProjectAuthority, WriterRole};
use magistr_workflow::{roadmap::domain::ChunkStatus, run::domain::step::StepType};
use std::fs::File;

fn fixture() -> (tempfile::TempDir, NativeConfig, NativeConfig) {
    let (dir, current, old, _) = fixture_and_snapshot();
    (dir, current, old)
}

/// The older store plus the run it held, read before it took the older layout.
fn fixture_and_snapshot() -> (
    tempfile::TempDir,
    NativeConfig,
    NativeConfig,
    magistr_workflow_contracts::RunData,
) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir_all(root.join(".magistr")).unwrap();
    fs::write(
        root.join(".magistr/project.json"),
        r#"{"project":{"id":"saved-project"}}"#,
    )
    .unwrap();
    let current = NativeConfig::from_lookup(&root, |_| None)
        .with_data_dir(dir.path().join("data/magistr/workflow"));
    let old = current
        .clone()
        .with_data_dir(dir.path().join("data/think-and-ship"));
    let writer =
        ProjectAuthority::admit_for_project(&old, "saved-project".into(), WriterRole::Host)
            .unwrap()
            .open();
    writer
        .run_open("Keep this work".into(), vec![], vec![], "saved".into())
        .unwrap();
    writer
        .add_step("one".into(), "Saved step".into(), StepType::Implement, None)
        .unwrap();
    writer
        .roadmap_add_chunk(
            "saved".into(),
            "Saved plan".into(),
            String::new(),
            ChunkStatus::Pending,
            1,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .unwrap();
    drop(writer);
    let snapshot = NativeReader::open(&old).run_snapshot().unwrap();
    as_older_store(old.data_dir());
    (dir, current, old, snapshot)
}

/// Give a store written by the current writer the layout and spelling the
/// standalone writer used: the run family under `ship/` with its check results
/// under `receipts/`, and version 2 documents keyed `objective`, `tasks`,
/// `actions`.
fn as_older_store(data: &Path) {
    fn rename_dirs(dir: &Path) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rename_dirs(&path);
                if path.file_name().unwrap() == "check_results" {
                    fs::rename(&path, path.with_file_name("receipts")).unwrap();
                }
            } else if path.extension().is_some_and(|e| e == "json")
                && path.parent().unwrap().ends_with("sessions")
            {
                let mut doc: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                let text = serde_json::to_string(&doc).unwrap();
                let text = text
                    .replace("\"goal\":", "\"objective\":")
                    .replace("\"steps\":", "\"tasks\":")
                    .replace("\"progress\":", "\"actions\":")
                    .replace("\"step_id\":", "\"task_id\":")
                    .replace("\"next_progress_id\":", "\"next_action_id\":");
                doc = serde_json::from_str(&text).unwrap();
                doc["schema_version"] = serde_json::json!(2);
                fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
            }
        }
    }
    let run = data.join("run");
    if run.exists() {
        rename_dirs(&run);
        fs::rename(&run, data.join("ship")).unwrap();
    }
}

fn request(config: &NativeConfig) {
    let path = config.workflow_root().join(WorkflowUpgrade::REQUEST);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, WorkflowUpgrade::request("saved-project")).unwrap();
}

#[test]
fn admission_restores_saved_execution_and_plan_and_retains_originals_once() {
    let (_dir, current, old, snapshot) = fixture_and_snapshot();
    let paths = records(old.data_dir(), "saved-project", OLDER_FAMILIES).unwrap();
    let originals: Vec<_> = paths
        .iter()
        .map(|p| fs::read(old.data_dir().join(p)).unwrap())
        .collect();
    request(&current);
    assert!(
        NativeReader::open(&current).run_snapshot().is_err(),
        "pending upgrade never appears as an empty project"
    );
    let owner = ProjectAuthority::admit(&current, WriterRole::Host).unwrap();
    assert_eq!(
        NativeReader::open(&current).run_snapshot().unwrap(),
        snapshot
    );
    assert_eq!(
        NativeReader::open(&current).roadmap_chunks().unwrap().len(),
        1
    );
    for (path, original) in paths.iter().zip(originals.iter()) {
        assert_eq!(fs::read(old.data_dir().join(path)).unwrap(), *original);
        if !path.starts_with("ship") {
            assert_eq!(fs::read(current.data_dir().join(path)).unwrap(), *original);
        }
    }
    drop(owner);
    let next = ProjectAuthority::admit(&current, WriterRole::Host)
        .unwrap()
        .open();
    next.add_step("two".into(), "New work".into(), StepType::Implement, None)
        .unwrap();
    drop(next);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(
        NativeReader::open(&current).run_steps().unwrap().len(),
        2,
        "later work is never overwritten by another import"
    );
}

#[test]
fn a_partial_copy_retries_but_a_different_destination_is_never_overwritten() {
    let (_dir, current, old) = fixture();
    request(&current);
    let paths = records(old.data_dir(), "saved-project", OLDER_FAMILIES).unwrap();
    let first = &paths[0];
    publish(
        &current.data_dir().join(first),
        &fs::read(old.data_dir().join(first)).unwrap(),
        false,
    )
    .unwrap();
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(NativeReader::open(&current).run_steps().unwrap().len(), 1);
    // Simulate an unfinished upgrade that encounters independently changed work.
    request(&current);
    fs::write(current.data_dir().join(first), b"conflicting work").unwrap();
    let error = ProjectAuthority::admit(&current, WriterRole::Host).unwrap_err();
    assert!(
        error.to_string().contains("Both copies are intact"),
        "{error}"
    );
    assert_eq!(
        fs::read(current.data_dir().join(first)).unwrap(),
        b"conflicting work"
    );
}

#[test]
fn another_project_and_a_live_older_owner_are_left_alone() {
    let (_dir, current, old) = fixture();
    let other = old.data_dir().join("ship/sessions/other-project.json");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, "unrelated").unwrap();
    request(&current);
    let guard = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(
            old.data_dir()
                .join("roadmap/sessions/saved-project.project-owner.lock"),
        )
        .unwrap();
    guard.try_lock().unwrap();
    assert_eq!(
        ProjectAuthority::admit(&current, WriterRole::Host)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(
        records(current.data_dir(), "saved-project", CURRENT_FAMILIES)
            .unwrap()
            .is_empty()
    );
    drop(guard);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert!(
        !current
            .data_dir()
            .join("run/sessions/other-project.json")
            .exists()
    );
    assert_eq!(fs::read(other).unwrap(), b"unrelated");
}

#[test]
fn no_record_lock_outlives_the_upgrade_that_took_it() {
    let (_dir, current, old) = fixture();
    let paths = records(old.data_dir(), "saved-project", OLDER_FAMILIES).unwrap();
    request(&current);
    let owner = ProjectAuthority::admit(&current, WriterRole::Host).unwrap();
    for relative in &paths {
        let renamed = relative
            .to_string_lossy()
            .replacen("ship/", "run/", 1)
            .replace("/receipts/", "/check_results/");
        for (store, relative) in [
            (old.data_dir(), relative.clone()),
            (current.data_dir(), PathBuf::from(renamed)),
        ] {
            let lock = File::options()
                .read(true)
                .write(true)
                .open(store.join(&relative).with_extension("json.lock"))
                .unwrap();
            assert!(
                lock.try_lock().is_ok(),
                "{} stays locked after the upgrade",
                store.join(relative).display()
            );
        }
    }
    drop(owner);
}

#[test]
fn the_owner_that_ran_the_upgrade_can_save_new_work() {
    let (_dir, current, _old) = fixture();
    request(&current);
    let writer = ProjectAuthority::admit(&current, WriterRole::Host)
        .unwrap()
        .open();
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let saved = writer
            .add_step("two".into(), "New work".into(), StepType::Implement, None)
            .is_ok();
        let _ = done.send(saved);
    });
    assert_eq!(
        finished.recv_timeout(std::time::Duration::from_secs(10)),
        Ok(true),
        "a save under the upgrading owner never waits on its own record lock"
    );
}

#[test]
fn an_older_owner_is_fenced_even_before_its_first_record() {
    let (_dir, current, old) = fixture();
    for path in records(old.data_dir(), "saved-project", OLDER_FAMILIES).unwrap() {
        fs::remove_file(old.data_dir().join(path)).unwrap();
    }
    request(&current);
    let guard = File::options()
        .read(true)
        .write(true)
        .open(
            old.data_dir()
                .join("roadmap/sessions/saved-project.project-owner.lock"),
        )
        .unwrap();
    guard.try_lock().unwrap();
    assert_eq!(
        ProjectAuthority::admit(&current, WriterRole::Host)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    let pending: Request = serde_json::from_slice(
        &fs::read(current.workflow_root().join(WorkflowUpgrade::REQUEST)).unwrap(),
    )
    .unwrap();
    assert!(!pending.complete);
}

#[cfg(unix)]
#[test]
fn a_linked_destination_is_refused_before_copying() {
    let (dir, current, _old) = fixture();
    request(&current);
    fs::create_dir_all(current.data_dir()).unwrap();
    let external = dir.path().join("external");
    fs::create_dir(&external).unwrap();
    std::os::unix::fs::symlink(&external, current.data_dir().join("run")).unwrap();
    assert!(ProjectAuthority::admit(&current, WriterRole::Host).is_err());
    assert_eq!(fs::read_dir(external).unwrap().count(), 0);
}
const RUN_V1: &[u8] = include_bytes!("fixtures/older-run.json");
const THINK_V1: &[u8] = include_bytes!("fixtures/older-think.json");

fn older_fixture() -> (tempfile::TempDir, NativeConfig, NativeConfig) {
    let fixture = fixture();
    let (_, current, old) = &fixture;
    for path in records(old.data_dir(), "saved-project", OLDER_FAMILIES).unwrap() {
        if path.starts_with("ship") {
            fs::remove_file(old.data_dir().join(path)).unwrap();
        }
    }
    for (path, bytes) in [
        ("ship/sessions/saved-project.json", RUN_V1),
        ("think/sessions/saved-project.json", THINK_V1),
        (
            "roadmap/sessions/saved-project.json",
            include_bytes!("fixtures/older-roadmap.json").as_slice(),
        ),
        (
            "signal/sessions/saved-project.json",
            include_bytes!("fixtures/older-signal.json").as_slice(),
        ),
    ] {
        publish(&old.data_dir().join(path), bytes, true).unwrap();
    }
    request(current);
    fixture
}

#[test]
fn old_work_and_reasoning_are_converted_into_the_records_current_readers_use() {
    let (_dir, current, old) = older_fixture();
    let before = records(old.data_dir(), "saved-project", OLDER_FAMILIES)
        .unwrap()
        .into_iter()
        .map(|path| {
            let bytes = fs::read(old.data_dir().join(&path)).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    let owner = ProjectAuthority::admit(&current, WriterRole::Host).unwrap();
    let reader = NativeReader::open(&current);
    let work = reader.run_snapshot().unwrap();
    assert_eq!(
        reader.run_steps().unwrap()[0].progress[0].think_step,
        Some(7)
    );
    assert_eq!(reader.run_steps().unwrap()[0].id, "one");
    assert_eq!(reader.run_steps().unwrap()[0].checks[0].name, "Saved check");
    assert_eq!(
        reader.think_step(7).unwrap().unwrap().record_id.as_deref(),
        Some("retained-step")
    );
    assert_eq!(
        reader.roadmap_chunks().unwrap()[0].project_id.as_deref(),
        Some("saved-project")
    );
    assert_eq!(
        reader.signal_list().unwrap()[0].project_id.as_deref(),
        Some("saved-project")
    );
    assert!(
        !current
            .data_dir()
            .join("run/sessions/saved-project.json")
            .exists()
    );
    for (path, bytes) in &before {
        assert_eq!(fs::read(old.data_dir().join(path)).unwrap(), *bytes);
    }
    drop(owner);
    let next = ProjectAuthority::admit(&current, WriterRole::Host)
        .unwrap()
        .open();
    assert_eq!(NativeReader::open(&current).run_snapshot().unwrap(), work);
    next.add_step(
        "two".into(),
        "Continue saved work".into(),
        StepType::Implement,
        None,
    )
    .unwrap();
    drop(next);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(NativeReader::open(&current).run_steps().unwrap().len(), 2);
}

#[test]
fn interrupted_old_format_conversion_reuses_exact_published_identities() {
    let (_dir, current, old) = older_fixture();
    let source = records(old.data_dir(), "saved-project", OLDER_FAMILIES)
        .unwrap()
        .into_iter()
        .map(|path| {
            let bytes = fs::read(old.data_dir().join(&path)).unwrap();
            (path, bytes)
        })
        .collect();
    let planned = converted_records(&current, "saved-project", source).unwrap();
    let (relative, bytes) = planned
        .iter()
        .find(|(path, _)| path.starts_with("run"))
        .unwrap();
    publish(&current.data_dir().join(relative), bytes, false).unwrap();
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(fs::read(current.data_dir().join(relative)).unwrap(), *bytes);
    assert_eq!(NativeReader::open(&current).run_steps().unwrap().len(), 1);
    assert!(
        NativeReader::open(&current)
            .think_step(7)
            .unwrap()
            .is_some()
    );
}

#[test]
fn invalid_old_history_refuses_the_whole_copy_and_keeps_the_request_pending() {
    let (_dir, current, old) = older_fixture();
    let path = old.data_dir().join("think/sessions/saved-project.json");
    let mut invalid: serde_json::Value = serde_json::from_slice(THINK_V1).unwrap();
    invalid["schema_version"] = serde_json::json!(99);
    let bytes = serde_json::to_vec(&invalid).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(ProjectAuthority::admit(&current, WriterRole::Host).is_err());
    assert!(
        records(current.data_dir(), "saved-project", CURRENT_FAMILIES)
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    let pending: Request = serde_json::from_slice(
        &fs::read(current.workflow_root().join(WorkflowUpgrade::REQUEST)).unwrap(),
    )
    .unwrap();
    assert!(!pending.complete);
}

#[test]
fn a_live_older_document_owner_blocks_conversion_before_any_copy() {
    let (_dir, current, old) = older_fixture();
    let lock = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(old.data_dir().join("ship/sessions/saved-project.json.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        ProjectAuthority::admit(&current, WriterRole::Host)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(
        records(current.data_dir(), "saved-project", CURRENT_FAMILIES)
            .unwrap()
            .is_empty()
    );
    drop(lock);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
}

/// One user's environment naming two stores, as Git Bash and PowerShell do
/// on Windows: the launch uses the first, the other is ranked below it.
fn two_store_launch(dir: &Path) -> NativeConfig {
    let root = dir.join("project");
    fs::create_dir_all(root.join(".magistr")).unwrap();
    fs::write(
        root.join(".magistr/project.json"),
        r#"{"project":{"id":"saved-project"}}"#,
    )
    .unwrap();
    let home = dir.join("home").to_string_lossy().into_owned();
    let appdata = dir.join("appdata").to_string_lossy().into_owned();
    NativeConfig::from_lookup(&root, move |key| match key {
        "HOME" => Some(home.clone()),
        "APPDATA" => Some(appdata.clone()),
        _ => None,
    })
}

fn request_state(config: &NativeConfig) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(config.workflow_root().join(WorkflowUpgrade::REQUEST)).unwrap(),
    )
    .unwrap()
}

#[test]
fn work_saved_under_another_store_of_the_same_user_refuses_instead_of_completing() {
    let dir = tempfile::tempdir().unwrap();
    let launch = two_store_launch(dir.path());
    let elsewhere = launch.other_data_dirs[0].clone();
    let older = older_root(&elsewhere).expect("an older sibling");
    let writer = ProjectAuthority::admit_for_project(
        &launch.clone().with_data_dir(&older),
        "saved-project".into(),
        WriterRole::Host,
    )
    .unwrap()
    .open();
    writer
        .run_open("Keep this work".into(), vec![], vec![], "saved".into())
        .unwrap();
    drop(writer);
    request(&launch);

    let refused = ProjectAuthority::admit(&launch, WriterRole::Host).unwrap_err();
    let message = refused.to_string();
    assert!(message.contains(&older.display().to_string()), "{message}");
    assert!(message.contains("MAGISTR_WORKFLOW_DATA_DIR"), "{message}");
    assert_eq!(request_state(&launch)["complete"], false);
    assert!(
        records(launch.data_dir(), "saved-project", CURRENT_FAMILIES)
            .unwrap()
            .is_empty()
    );
    assert!(
        !records(&older, "saved-project", CURRENT_FAMILIES)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_project_that_never_saved_work_still_completes_its_upgrade() {
    let dir = tempfile::tempdir().unwrap();
    let launch = two_store_launch(dir.path());
    request(&launch);
    drop(ProjectAuthority::admit(&launch, WriterRole::Host).unwrap());
    assert_eq!(request_state(&launch)["complete"], true);
}
