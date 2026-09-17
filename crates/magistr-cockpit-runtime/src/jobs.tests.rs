use std::time::Duration;

use super::*;

#[tokio::test]
async fn a_job_result_arrives_on_the_inbox_with_its_id() {
    let (inbox, mut rx) = Inbox::channel();
    let jobs = Jobs::new(inbox);
    let first = jobs.spawn_job(|| JobOutput::Text("one".to_owned()));
    let second = jobs.spawn_job(|| JobOutput::Text("two".to_owned()));
    assert_ne!(first, second);
    assert_eq!(jobs.spawned(), 2);

    let mut got = Vec::new();
    for _ in 0..2 {
        let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("a result within 2 s")
            .expect("the inbox is open");
        match msg {
            Msg::Job { id, output } => got.push((id, output)),
            other => panic!("not a job result: {other:?}"),
        }
    }
    got.sort_by_key(|(id, _)| *id);
    assert_eq!(
        got,
        vec![
            (first, JobOutput::Text("one".to_owned())),
            (second, JobOutput::Text("two".to_owned())),
        ]
    );
}

#[tokio::test]
async fn a_panicking_job_comes_back_as_failed() {
    let (inbox, mut rx) = Inbox::channel();
    let jobs = Jobs::new(inbox);
    let id = jobs.spawn_job(|| panic!("boom"));
    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("a result within 2 s")
        .expect("the inbox is open");
    assert_eq!(
        msg,
        Msg::Job {
            id,
            output: JobOutput::Failed("boom".to_owned())
        }
    );
}

#[tokio::test]
async fn a_slow_job_does_not_block_the_task_that_spawned_it() {
    let (inbox, mut rx) = Inbox::channel();
    let jobs = Jobs::new(inbox);
    let began = std::time::Instant::now();
    jobs.spawn_job(|| {
        std::thread::sleep(Duration::from_millis(200));
        JobOutput::Text("late".to_owned())
    });
    assert!(
        began.elapsed() < Duration::from_millis(100),
        "spawn returned at once"
    );
    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("a result within 2 s")
        .expect("the inbox is open");
    assert!(began.elapsed() >= Duration::from_millis(200));
    assert!(matches!(msg, Msg::Job { output: JobOutput::Text(t), .. } if t == "late"));
}

#[tokio::test]
async fn a_result_for_a_gone_loop_is_dropped_quietly() {
    let (inbox, rx) = Inbox::channel();
    drop(rx);
    let jobs = Jobs::new(inbox);
    jobs.spawn_job(|| JobOutput::Text("nobody home".to_owned()));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(jobs.spawned(), 1);
}
