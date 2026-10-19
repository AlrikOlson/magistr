use super::*;
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Sample {
    name: String,
    count: u32,
}

fn cfg(tmp: &TempDir) -> PersistenceConfig {
    PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_path_buf(),
    }
}

fn lookup(vars: Vec<(&'static str, &'static str)>) -> impl Fn(&str) -> Option<String> {
    move |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn data_dir_prefers_xdg_over_everything() {
    let dir = default_data_dir_from(lookup(vec![
        ("XDG_DATA_HOME", "/xdg"),
        ("HOME", "/home/dev"),
        ("APPDATA", "C:\\Users\\dev\\AppData\\Roaming"),
    ]));
    assert_eq!(dir, Path::new("/xdg").join("magistr/workflow"));
}

#[test]
fn data_dir_home_wins_over_appdata() {
    // An explicit home takes precedence over the platform application-data path.
    let dir = default_data_dir_from(lookup(vec![
        ("HOME", "/home/dev"),
        ("APPDATA", "C:\\Users\\dev\\AppData\\Roaming"),
    ]));
    assert_eq!(
        dir,
        Path::new("/home/dev")
            .join(".local")
            .join("share")
            .join("magistr/workflow")
    );
}

#[test]
fn data_dir_windows_without_home_lands_in_appdata() {
    // The PowerShell / GUI-launched shape: no HOME, no XDG.
    let dir = default_data_dir_from(lookup(vec![(
        "APPDATA",
        "C:\\Users\\dev\\AppData\\Roaming",
    )]));
    assert_eq!(
        dir,
        Path::new("C:\\Users\\dev\\AppData\\Roaming").join("magistr/workflow")
    );
}

#[test]
fn data_dir_userprofile_backstops_a_missing_appdata() {
    let dir = default_data_dir_from(lookup(vec![("USERPROFILE", "C:\\Users\\dev")]));
    assert_eq!(
        dir,
        Path::new("C:\\Users\\dev")
            .join("AppData")
            .join("Roaming")
            .join("magistr/workflow")
    );
}

#[test]
fn data_dir_empty_values_do_not_capture_the_chain() {
    let dir = default_data_dir_from(lookup(vec![
        ("XDG_DATA_HOME", "  "),
        ("HOME", ""),
        ("APPDATA", "C:\\Users\\dev\\AppData\\Roaming"),
    ]));
    assert_eq!(
        dir,
        Path::new("C:\\Users\\dev\\AppData\\Roaming").join("magistr/workflow")
    );
}

#[test]
fn data_dir_falls_back_to_the_os_temp_dir() {
    let dir = default_data_dir_from(lookup(vec![]));
    assert_eq!(dir, env::temp_dir().join("magistr/workflow"));
}

#[test]
fn save_then_load_round_trip() {
    let tmp = TempDir::new().unwrap();
    let p = Persistence::new(&cfg(&tmp), Domain::Think);
    let val = Sample {
        name: "x".into(),
        count: 1,
    };
    p.save("alpha", &val).unwrap();
    let back: Sample = p.load("alpha").unwrap().unwrap();
    assert_eq!(back, val);
}

#[test]
fn workflow_use_disjoint_subdirs() {
    let tmp = TempDir::new().unwrap();
    let t = Persistence::new(&cfg(&tmp), Domain::Think);
    let s = Persistence::new(&cfg(&tmp), Domain::Ship);
    assert!(t.sessions_dir().ends_with("think/sessions"));
    assert!(s.sessions_dir().ends_with("ship/sessions"));
    assert_ne!(t.sessions_dir(), s.sessions_dir());
}

#[test]
fn roadmap_uses_its_own_subdir() {
    let tmp = TempDir::new().unwrap();
    let r = Persistence::new(&cfg(&tmp), Domain::Roadmap);
    let t = Persistence::new(&cfg(&tmp), Domain::Think);
    let s = Persistence::new(&cfg(&tmp), Domain::Ship);
    assert!(r.sessions_dir().ends_with("roadmap/sessions"));
    assert_ne!(r.sessions_dir(), t.sessions_dir());
    assert_ne!(r.sessions_dir(), s.sessions_dir());
}

#[test]
fn atomic_write_leaves_no_tmp_behind() {
    let tmp = TempDir::new().unwrap();
    let p = Persistence::new(&cfg(&tmp), Domain::Ship);
    p.save(
        "beta",
        &Sample {
            name: "y".into(),
            count: 2,
        },
    )
    .unwrap();
    let entries: Vec<String> = fs::read_dir(p.sessions_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    assert!(entries.contains(&"beta.json".to_string()));
    assert!(entries.contains(&"beta.json.lock".to_string()));
    assert_eq!(entries.len(), 2);
}

#[test]
fn load_missing_returns_none() {
    let tmp = TempDir::new().unwrap();
    let p = Persistence::new(&cfg(&tmp), Domain::Think);
    let back: Option<Sample> = p.load("does-not-exist").unwrap();
    assert!(back.is_none());
}

#[test]
fn disabled_persistence_is_noop() {
    let tmp = TempDir::new().unwrap();
    let mut c = cfg(&tmp);
    c.enabled = false;
    let p = Persistence::new(&c, Domain::Think);
    let val = Sample {
        name: "z".into(),
        count: 3,
    };
    p.save("gamma", &val).unwrap();
    let back: Option<Sample> = p.load("gamma").unwrap();
    assert!(back.is_none());
}
