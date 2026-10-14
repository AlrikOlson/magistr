use super::*;
use crate::native::{ChunkStatus, NativeEngines, ProjectAuthority, WriterRole};

fn fixture() -> (tempfile::TempDir, NativeConfig, NativeEngines) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let config = NativeConfig::from_lookup(root, |_| None).with_data_dir(temp.path().join("data"));
    let writer =
        ProjectAuthority::admit_for_project(&config, "workstream-read".into(), WriterRole::Host)
            .unwrap()
            .open();
    (temp, config, writer)
}

fn add(writer: &NativeEngines, id: &str, priority: u32, group: Option<&str>) {
    writer
        .roadmap_add_chunk(
            id.into(),
            format!("Work {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            vec![],
            None,
            group.map(str::to_owned),
        )
        .unwrap();
}

#[test]
fn current_workstreams_refresh_without_redirecting_closed_or_missing_names() {
    let (_temp, config, writer) = fixture();
    add(&writer, "old", 1, Some("Rust"));
    add(&writer, "desktop", 20, Some("Rust desktop"));
    add(&writer, "ungrouped", 10, None);
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Rust", "Rust desktop"]
    );
    writer
        .roadmap_set_status("old", ChunkStatus::Obsoleted)
        .unwrap();
    let catalog = observer.roadmap_workstreams().unwrap();
    assert_eq!(catalog.current_names(), ["Rust desktop"]);
    assert_eq!(catalog.history["Rust"].obsoleted, 1);
    assert_eq!(catalog.ungrouped.counts.total, 1);
    assert_eq!(observer.roadmap_chunks().unwrap().len(), 3);

    let refusal = observer.roadmap_resolve_track("Rust").unwrap_err();
    assert!(refusal.contains("no unfinished work"), "{refusal}");
    assert!(refusal.contains("Rust desktop"), "{refusal}");
    assert!(observer.roadmap_next_in_track("Rust").is_err());
    assert!(observer.roadmap_frontier(Some("Rust des")).is_err());
    assert_eq!(
        observer.roadmap_resolve_track("Rust des").unwrap(),
        "Rust desktop"
    );
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "ungrouped");
    let ungrouped = observer.roadmap_frontier(None).unwrap();
    assert_eq!(ungrouped.items.len(), 1);
    assert_eq!(ungrouped.items[0].id, "ungrouped");
    assert_eq!(
        observer
            .roadmap_next_in_track("Rust desktop")
            .unwrap()
            .unwrap()
            .id,
        "desktop"
    );

    writer
        .roadmap_set_group("desktop", Some("Interface".into()))
        .unwrap();
    assert!(observer.roadmap_next_in_track("Rust desktop").is_err());
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Interface"]
    );
    writer
        .roadmap_set_status("desktop", ChunkStatus::Obsoleted)
        .unwrap();
    let catalog = observer.roadmap_workstreams().unwrap();
    assert!(catalog.current.is_empty());
    assert_eq!(catalog.history["Interface"].obsoleted, 1);
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "ungrouped");
}

#[test]
fn selection_refuses_an_unreadable_store_and_recovers_without_cached_success() {
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, Some("Current"));
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "work");
    let path =
        Persistence::new(&config.persistence_config(), Domain::Roadmap).path_for("workstream-read");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ invalid roadmap").unwrap();

    assert!(observer.roadmap_chunks().is_err());
    assert!(observer.roadmap_next().is_err());
    assert!(observer.roadmap_next_in_track("Current").is_err());
    assert!(observer.roadmap_resolve_track("Current").is_err());
    assert!(observer.roadmap_workstreams().is_err());
    assert!(observer.roadmap_frontier(Some("Current")).is_err());
    let newly_opened = NativeReader::open_for_project(&config, "workstream-read".into());
    assert!(newly_opened.roadmap_chunks().is_err());
    assert!(newly_opened.roadmap_next().is_err());

    std::fs::write(&path, original).unwrap();
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "work");
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Current"]
    );
}
