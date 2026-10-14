use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::{Duration, TimeZone};

fn record(project: &str, started: DateTime<Utc>, cost_usd: f64) -> SessionRecord {
    SessionRecord {
        session_id: format!("{project}-{started}"),
        name: None,
        project: project.to_owned(),
        workspace: project.to_owned(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: Some(HistoryOutcome::Completed),
        started_at: started,
        updated_at: started,
        ended_at: Some(started),
        cost_usd: Some(cost_usd),
        turns: Some(1),
        duration_ms: 1,
        files_changed: Vec::new(),
        tags: Vec::new(),
        summary: None,
        model: None,
        owner: None,
    }
}

fn prepared(root: &Path, name: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(dir.join(".magistr")).expect("project");
    std::fs::write(dir.join(".magistr/project.json"), "{}").expect("config");
    dir.canonicalize().expect("canonical")
}

#[test]
fn linked_working_copies_share_one_recent_project() {
    let tmp = tempfile::tempdir().unwrap();
    let main = prepared(tmp.path(), "main");
    let linked = tmp.path().join("linked");
    for args in [
        vec!["init", "--initial-branch=main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "fixture",
        ],
        vec!["worktree", "add", "--detach", linked.to_str().unwrap()],
    ] {
        assert!(
            std::process::Command::new("git")
                .current_dir(&main)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let now = Utc::now();
    let records = [
        record(main.to_str().unwrap(), now, 1.0),
        record(linked.to_str().unwrap(), now, 2.0),
    ];
    for here in [None, Some(linked.as_path())] {
        let data = from_records(here, &records, now);
        assert_eq!(data.projects.len(), 1);
        assert_eq!(data.projects[0].dir.as_deref(), main.to_str());
        assert_eq!(data.spent_cents, 300);
    }
}

#[test]
fn recent_projects_are_the_recorded_directories_newest_first_with_this_one_leading() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let here = prepared(tmp.path(), "here");
    let older = prepared(tmp.path(), "older");
    let newer = prepared(tmp.path(), "newer");
    let now = Utc.with_ymd_and_hms(2026, 9, 19, 12, 0, 0).unwrap();
    let records = vec![
        record(&older.display().to_string(), now - Duration::days(3), 0.50),
        record(&newer.display().to_string(), now - Duration::hours(2), 1.25),
        // The same project spelled with a trailing slash is one project.
        record(
            &format!("{}/", newer.display()),
            now - Duration::days(9),
            0.25,
        ),
        // A name is not a directory, a vanished directory cannot be opened,
        // and a directory without Magistr setup is not a project yet.
        record("just-a-name", now, 9.0),
        record("/nowhere/at/all", now, 9.0),
        record(&tmp.path().display().to_string(), now, 9.0),
    ];
    let data = from_records(Some(&here), &records, now);
    let listed: Vec<(&str, Option<&str>, bool)> = data
        .projects
        .iter()
        .map(|p| (p.name.as_str(), p.dir.as_deref(), p.here))
        .collect();
    assert_eq!(
        listed,
        [
            ("here", Some(here.to_str().unwrap()), true),
            ("newer", Some(newer.to_str().unwrap()), false),
            ("older", Some(older.to_str().unwrap()), false),
        ]
    );
    assert_eq!(
        data.projects[1].idle_secs,
        Some(2 * 3600),
        "its latest session"
    );
    assert_eq!(
        data.projects[0].idle_secs, None,
        "this project has no record yet"
    );
    assert_eq!(
        data.spent_cents, 200,
        "what the listed projects' sessions cost"
    );
    assert_eq!(data.cap_cents, None);
}

#[test]
fn the_list_is_bounded_and_an_unlocated_database_still_lists_this_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let here = prepared(tmp.path(), "here");
    let now = Utc.with_ymd_and_hms(2026, 9, 19, 12, 0, 0).unwrap();
    let records: Vec<_> = (0..20)
        .map(|n| {
            let dir = prepared(tmp.path(), &format!("p{n:02}"));
            record(&dir.display().to_string(), now - Duration::minutes(n), 0.0)
        })
        .collect();
    let data = from_records(Some(&here), &records, now);
    assert_eq!(data.projects.len(), RECENT_PROJECTS + 1);
    assert_eq!(data.projects[1].name, "p00");

    let inputs = ProjectInputs::from_lookup(&here, |_| None);
    let alone = collect(Some(&here), &inputs);
    assert_eq!(alone.projects.len(), 1);
    assert!(alone.projects[0].here);
}
