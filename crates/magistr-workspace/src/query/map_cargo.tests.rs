use super::*;
use serde_json::json;

fn fixture() -> BTreeMap<String, String> {
    let mut files = BTreeMap::from([
        ("Cargo.toml".into(), "[workspace]\nmembers = [\"crates/*\", \"tools/cli\", \"vendor/**\"]\nexclude = [\"crates/skipme\"]\n[workspace.dependencies]\nalpha = { path = \"crates/alpha\" }\nserde = \"1\"\n[package]\nname = \"rootpkg\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nbeta-crate = { path = \"crates/beta\" }\n[[bin]]\nname = \"root-tool\"\npath = \"tools/root-tool.rs\"\n".into()),
        ("crates/alpha/Cargo.toml".into(), "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[features]\ndefault = [\"fast\"]\nfast = []\nslow = []\n[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\n".into()),
        ("crates/beta/Cargo.toml".into(), "[package]\nname = \"beta-crate\"\nversion = \"0.1.0\"\n[dependencies]\nalpha = { workspace = true }\nserde = { workspace = true }\n[dev-dependencies]\nmacros = { path = \"../macros\" }\n[build-dependencies]\ncli = { path = \"../../tools/cli\", package = \"cli\" }\n".into()),
        ("crates/macros/Cargo.toml".into(), "[package]\nname = \"macros\"\nversion = \"0.1.0\"\n[lib]\nproc-macro = true\n[dependencies]\nsyn = \"2\"\nquote = \"1\"\n".into()),
        ("crates/skipme/Cargo.toml".into(), "[package]\nname = \"skipme\"\n".into()),
        ("tools/cli/Cargo.toml".into(), "[package]\nname = \"cli\"\nversion = \"0.1.0\"\n[dependencies]\nalpha = { path = \"../../crates/alpha\" }\nrenamed = { package = \"beta-crate\", path = \"../../crates/beta\" }\n[target.'cfg(windows)'.dependencies]\nwindows-sys = \"0.61\"\n".into()),
    ]);
    for path in [
        "tools/root-tool.rs",
        "src/lib.rs",
        "crates/alpha/src/lib.rs",
        "crates/alpha/src/main.rs",
        "crates/alpha/src/bin/alpha-cli.rs",
        "crates/alpha/examples/demo.rs",
        "crates/alpha/benches/speed.rs",
        "crates/alpha/tests/integration.rs",
        "crates/alpha/build.rs",
        "crates/alpha/src/nested/not_a_root.rs",
        "crates/beta/src/lib.rs",
        "crates/macros/src/lib.rs",
        "crates/skipme/src/lib.rs",
        "crates/notacrate/README.md",
        "tools/cli/src/main.rs",
        "README.md",
    ] {
        files.insert(path.into(), String::new());
    }
    files
}

fn imports() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        (
            "crates/beta/src/lib.rs".into(),
            vec!["alpha::Thing".into(), "alpha::Other".into()],
        ),
        (
            "crates/beta/src/second.rs".into(),
            vec!["alpha::Thing".into()],
        ),
        (
            "tools/cli/src/main.rs".into(),
            vec!["alpha::Thing".into(), "beta_crate::Thing".into()],
        ),
        (
            "crates/alpha/tests/integration.rs".into(),
            vec!["alpha::Thing".into()],
        ),
    ])
}

fn model(files: &BTreeMap<String, String>) -> CargoMap {
    CargoMap::read(&files.keys().cloned().collect::<Vec<_>>(), |path| {
        files.get(path).cloned()
    })
    .expect("Cargo model")
}

#[test]
fn cargo_rows_match_the_retained_reference_golden() {
    let fixture = fixture();
    let (cargo, crates, notes) = model(&fixture).render("", Some(&imports()));
    let actual = json!({"cargo": cargo, "crates": crates});
    let expected: serde_json::Value = serde_json::from_str(include_str!("../../../../subsystems/workspace/reference/test/support/golden/map/cargo-workspace.expected.json")).expect("reference golden");
    assert_eq!(actual, expected);
    assert!(notes.iter().any(|note| note.contains("vendor/**")));
}

#[test]
fn scope_keeps_whole_workspace_counts_and_cold_omits_use_claims() {
    let fixture = fixture();
    let (cargo, rows, _) = model(&fixture).render("crates", None);
    assert_eq!(cargo.members, 5);
    assert_eq!(cargo.omitted, Some(2));
    assert_eq!(cargo.unused_deps, None);
    assert_eq!(
        rows.iter().map(MapRow::path).collect::<Vec<_>>(),
        ["crates/alpha", "crates/beta", "crates/macros"]
    );
    let MapRow::Crate { dependencies, .. } = &rows[1] else {
        panic!("crate row")
    };
    assert!(
        dependencies
            .iter()
            .all(|dependency| dependency.files.is_none())
    );
}

#[test]
fn row_manifest_feature_and_entry_caps_are_explicit() {
    let mut files = BTreeMap::from([(
        "Cargo.toml".into(),
        "[workspace]\nmembers=[\"crates/*\"]\n".into(),
    )]);
    for i in 0..258 {
        files.insert(
            format!("crates/m{i:03}/Cargo.toml"),
            format!("[package]\nname=\"m{i:03}\"\n"),
        );
        files.insert(format!("crates/m{i:03}/src/lib.rs"), String::new());
    }
    let (cargo, rows, notes) = model(&files).render("", None);
    assert_eq!(cargo.members, 256);
    assert_eq!(cargo.omitted, Some(232));
    assert_eq!(rows.len(), 24);
    assert!(
        notes
            .iter()
            .any(|note| note.contains("2 Cargo member manifests"))
    );
    assert!(
        notes
            .iter()
            .any(|note| note.contains("232 workspace members"))
    );
    let mut member = BTreeMap::from([(
        "Cargo.toml".into(),
        format!(
            "[package]\nname=\"many\"\n[features]\n{}",
            (0..20).map(|i| format!("f{i}=[]\n")).collect::<String>()
        ),
    )]);
    for i in 0..12 {
        member.insert(format!("examples/ex{i:02}.rs"), String::new());
    }
    let (_, rows, _) = model(&member).render("", None);
    let MapRow::Crate {
        more_entry_points,
        more_features,
        entry_points,
        features,
        ..
    } = &rows[0]
    else {
        panic!("crate")
    };
    assert_eq!((*more_entry_points, *more_features), (Some(4), Some(4)));
    assert_eq!((entry_points.len(), features.len()), (8, 16));
}

#[test]
fn longest_member_owns_imports_and_custom_lib_name_is_exact() {
    let mut files = BTreeMap::from([
        ("Cargo.toml".into(), "[workspace]\nmembers=[\"crates/a\",\"crates/a/nested\"]\n[package]\nname=\"root\"\n[dependencies]\na={path=\"crates/a\"}\n".into()),
        ("crates/a/Cargo.toml".into(), "[package]\nname=\"a\"\n[lib]\nname=\"custom_a\"\n".into()),
        ("crates/a/nested/Cargo.toml".into(), "[package]\nname=\"nested\"\n[dependencies]\na={path=\"..\"}\n".into()),
    ]);
    files.insert("crates/a/nested/src/lib.rs".into(), String::new());
    let imports = BTreeMap::from([(
        "crates/a/nested/src/lib.rs".into(),
        vec!["custom_a::Item".into(), "custom_a_other::Wrong".into()],
    )]);
    let (summary, rows, _) = model(&files).render("", Some(&imports));
    assert_eq!(summary.unused_deps, Some(1));
    let nested = rows
        .iter()
        .find(|row| matches!(row, MapRow::Crate { name, .. } if name == "nested"))
        .expect("nested crate");
    let MapRow::Crate { dependencies, .. } = nested else {
        unreachable!()
    };
    assert_eq!(dependencies[0].files, Some(1));
}

#[test]
fn unlisted_and_invalid_manifests_never_create_members() {
    assert!(CargoMap::read(&[], |_| panic!("unlisted read")).is_none());
    assert!(CargoMap::read(&["Cargo.toml".into()], |_| Some("not toml".into())).is_none());
    let files = BTreeMap::from([(
        "Cargo.toml".into(),
        "[workspace]\nmembers=[\"../outside\"]\n".into(),
    )]);
    assert!(
        CargoMap::read(&files.keys().cloned().collect::<Vec<_>>(), |path| files
            .get(path)
            .cloned())
        .is_none()
    );
}
