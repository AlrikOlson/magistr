use super::*;
use std::collections::BTreeMap;
use std::sync::{Barrier, mpsc};
use std::time::Duration;

fn entry(id: &str, turns: u32) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: "concurrent-settlement".into(),
        timestamp: chrono::Utc::now(),
        cost_usd: Some(f64::from(turns)),
        tokens: Default::default(),
        turns,
        duration_ms: u64::from(turns) * 100,
        actor: None,
        model: None,
    }
}

fn turns_by_conversation(path: &Path) -> BTreeMap<String, u32> {
    let entries = load_entries(path).unwrap();
    let turns: BTreeMap<_, _> = entries
        .iter()
        .map(|entry| (entry.session_id.clone(), entry.turns))
        .collect();
    assert_eq!(turns.len(), entries.len(), "one row per conversation");
    turns
}

#[test]
fn concurrent_threads_retain_every_conversations_latest_entry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/costs.json");
    append_entry(&path, &entry("existing", 7)).unwrap();
    let start = Barrier::new(12);
    std::thread::scope(|scope| {
        for index in 0..12 {
            let start = &start;
            let path = &path;
            scope.spawn(move || {
                let mut saved = Vec::new();
                for turns in 1..=3 {
                    start.wait();
                    saved.push(append_entry(
                        path,
                        &entry(&format!("thread-{index}"), turns),
                    ));
                }
                for result in saved {
                    result.unwrap();
                }
            });
        }
    });
    let mut expected = BTreeMap::from([("existing".into(), 7)]);
    expected.extend((0..12).map(|index| (format!("thread-{index}"), 3)));
    assert_eq!(turns_by_conversation(&path), expected);
}

#[test]
fn a_waiting_writer_reads_after_the_previous_replacement_commits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("costs.json");
    let lock = lock_ledger(&path).unwrap();
    save_entries(&path, &[entry("previous", 1)]).unwrap();

    // The stable sibling remains locked after replacing the ledger inode.
    let contender = File::options()
        .read(true)
        .write(true)
        .open(dir.path().join("costs.json.lock"))
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));

    let (ready, waiting) = mpsc::channel();
    let (done, completed) = mpsc::channel();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            ready.send(()).unwrap();
            done.send(append_entry(&path, &entry("waiting", 2)))
                .unwrap();
        });
        waiting.recv().unwrap();
        assert!(matches!(
            completed.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        save_entries(&path, &[entry("previous", 5)]).unwrap();
        drop(lock);
        completed
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
    });
    assert_eq!(
        turns_by_conversation(&path),
        BTreeMap::from([("previous".into(), 5), ("waiting".into(), 2)])
    );
}

#[test]
fn a_failed_update_preserves_the_ledger_and_releases_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("costs.json");
    std::fs::write(&path, b"invalid ledger").unwrap();
    assert!(append_entry(&path, &entry("refused", 1)).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"invalid ledger");
    let lock = lock_ledger(&path).unwrap();
    save_entries(&path, &[entry("recovered", 2)]).unwrap();
    drop(lock);
    append_entry(&path, &entry("next", 3)).unwrap();
    assert_eq!(
        turns_by_conversation(&path),
        BTreeMap::from([("next".into(), 3), ("recovered".into(), 2)])
    );
}

#[test]
fn concurrent_processes_retain_every_conversation() {
    use std::io::{BufRead, Read, Write};
    use std::process::{Command, Stdio};

    const CHILD_LEDGER: &str = "MAGISTR_TEST_CONCURRENT_COST_LEDGER";
    const CHILD_INDEX: &str = "MAGISTR_TEST_CONCURRENT_COST_INDEX";
    if let Some(path) = std::env::var_os(CHILD_LEDGER) {
        let index = std::env::var(CHILD_INDEX).unwrap();
        println!("cost writer ready");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_exact(&mut [0]).unwrap();
        for turn in 1..=3 {
            for item in 0..4 {
                append_entry(
                    Path::new(&path),
                    &entry(&format!("process-{index}-{item}"), turn),
                )
                .unwrap();
            }
        }
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("costs.json");
    append_entry(&path, &entry("existing", 7)).unwrap();
    let thread = std::thread::current();
    let test_name = thread.name().unwrap();
    let mut children = Vec::new();
    for index in 0..4 {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture"])
            .env(CHILD_LEDGER, &path)
            .env(CHILD_INDEX, index.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        loop {
            let mut line = String::new();
            assert_ne!(
                output.read_line(&mut line).unwrap(),
                0,
                "child exited before ready"
            );
            if line.trim() == "cost writer ready" {
                break;
            }
        }
        child.stdout = Some(output.into_inner());
        children.push(child);
    }
    for child in &mut children {
        child.stdin.take().unwrap().write_all(&[1]).unwrap();
    }
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut expected = BTreeMap::from([("existing".into(), 7)]);
    expected.extend(
        (0..4).flat_map(|index| (0..4).map(move |item| (format!("process-{index}-{item}"), 3))),
    );
    assert_eq!(turns_by_conversation(&path), expected);
}
