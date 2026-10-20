use super::*;
use crate::native::{ProjectAuthority, WriterRole};
use magistr_workflow::{roadmap::domain::ChunkStatus, ship::domain::task::TaskType};

fn fixture() -> (tempfile::TempDir, NativeConfig, NativeConfig) {
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
        .ship_set_objective("Keep this work".into(), vec![], vec![], "saved".into())
        .unwrap();
    writer
        .ship_add_task("one".into(), "Saved task".into(), TaskType::Implement, None)
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
    (dir, current, old)
}

fn request(config: &NativeConfig) {
    let path = config.workflow_root().join(WorkflowUpgrade::REQUEST);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, WorkflowUpgrade::request("saved-project")).unwrap();
}

#[test]
fn admission_restores_saved_execution_and_plan_and_retains_originals_once() {
    let (_dir, current, old) = fixture();
    let paths = records(old.data_dir(), "saved-project").unwrap();
    let originals: Vec<_> = paths
        .iter()
        .map(|p| fs::read(old.data_dir().join(p)).unwrap())
        .collect();
    let snapshot = NativeReader::open(&old).ship_snapshot().unwrap();
    request(&current);
    assert!(
        NativeReader::open(&current).ship_snapshot().is_err(),
        "pending upgrade never appears as an empty project"
    );
    let owner = ProjectAuthority::admit(&current, WriterRole::Host).unwrap();
    assert_eq!(
        NativeReader::open(&current).ship_snapshot().unwrap(),
        snapshot
    );
    assert_eq!(
        NativeReader::open(&current).roadmap_chunks().unwrap().len(),
        1
    );
    for (path, original) in paths.iter().zip(originals.iter()) {
        assert_eq!(fs::read(old.data_dir().join(path)).unwrap(), *original);
        assert_eq!(fs::read(current.data_dir().join(path)).unwrap(), *original);
    }
    drop(owner);
    let next = ProjectAuthority::admit(&current, WriterRole::Host)
        .unwrap()
        .open();
    next.ship_add_task("two".into(), "New work".into(), TaskType::Implement, None)
        .unwrap();
    drop(next);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(
        NativeReader::open(&current).ship_tasks().unwrap().len(),
        2,
        "later work is never overwritten by another import"
    );
}

#[test]
fn a_partial_copy_retries_but_a_different_destination_is_never_overwritten() {
    let (_dir, current, old) = fixture();
    request(&current);
    let paths = records(old.data_dir(), "saved-project").unwrap();
    let first = &paths[0];
    publish(
        &current.data_dir().join(first),
        &fs::read(old.data_dir().join(first)).unwrap(),
        false,
    )
    .unwrap();
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert_eq!(NativeReader::open(&current).ship_tasks().unwrap().len(), 1);
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
        records(current.data_dir(), "saved-project")
            .unwrap()
            .is_empty()
    );
    drop(guard);
    drop(ProjectAuthority::admit(&current, WriterRole::Host).unwrap());
    assert!(
        !current
            .data_dir()
            .join("ship/sessions/other-project.json")
            .exists()
    );
    assert_eq!(fs::read(other).unwrap(), b"unrelated");
}

#[test]
fn an_older_owner_is_fenced_even_before_its_first_record() {
    let (_dir, current, old) = fixture();
    for path in records(old.data_dir(), "saved-project").unwrap() {
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
    std::os::unix::fs::symlink(&external, current.data_dir().join("ship")).unwrap();
    assert!(ProjectAuthority::admit(&current, WriterRole::Host).is_err());
    assert_eq!(fs::read_dir(external).unwrap().count(), 0);
}
