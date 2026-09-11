use super::*;
use crate::commands::rust_move::paths::crate_dir_of;

fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

fn strings(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| (*p).to_owned()).collect()
}

fn tree() -> ModuleTree {
    ModuleTree::build(
        set(&[
            "src/lib.rs",
            "src/main.rs",
            "src/a.rs",
            "src/a/b.rs",
            "src/a/b/c.rs",
            "src/m/mod.rs",
            "src/m/x.rs",
            "src/bin/tool.rs",
            "src/bin/big/main.rs",
            "src/bin/big/part.rs",
            "tests/it.rs",
            "tests/it/helper.rs",
            "tests/suite/main.rs",
            "tests/suite/common.rs",
            "build.rs",
            "src/orphan/deep.rs",
            "src/tools/cli.rs",
        ]),
        set(&["src/tools/cli.rs"]),
    )
}

#[test]
fn crate_dir_of_picks_the_nearest_cargo_toml_else_the_workspace_root() {
    let files = set(&[
        "Cargo.toml",
        "crates/x/Cargo.toml",
        "crates/x/src/lib.rs",
        "other/y.rs",
    ]);
    assert_eq!(crate_dir_of("crates/x/src/lib.rs", &files), "crates/x");
    assert_eq!(crate_dir_of("other/y.rs", &files), "");
    assert_eq!(crate_dir_of("lonely.rs", &set(&["lonely.rs"])), "");
}

#[test]
fn lib_and_main_share_one_src_tree_and_name_rs_and_mod_rs_are_modules() {
    let tree = tree();
    assert_eq!(
        tree.locate("src/a/b.rs"),
        Some(ModuleLocation {
            roots: strings(&["src/lib.rs", "src/main.rs"]),
            root_dir: "src".into(),
            segments: strings(&["a", "b"]),
            parents: strings(&["src/a.rs"]),
        })
    );
    let deep = tree.locate("src/a/b/c.rs").unwrap();
    assert_eq!(deep.segments, strings(&["a", "b", "c"]));
    assert_eq!(deep.parents, strings(&["src/a/b.rs"]));
    let m = tree.locate("src/m/mod.rs").unwrap();
    assert_eq!(m.segments, strings(&["m"]));
    assert_eq!(m.parents, strings(&["src/lib.rs", "src/main.rs"]));
    let x = tree.locate("src/m/x.rs").unwrap();
    assert_eq!(x.segments, strings(&["m", "x"]));
    assert_eq!(x.parents, strings(&["src/m/mod.rs"]));
    let lib = tree.locate("src/lib.rs").unwrap();
    assert!(lib.segments.is_empty() && lib.parents.is_empty());
}

#[test]
fn targets_are_their_own_roots() {
    let tree = tree();
    assert!(tree.locate("src/bin/tool.rs").unwrap().segments.is_empty());
    let part = tree.locate("src/bin/big/part.rs").unwrap();
    assert_eq!(part.roots, strings(&["src/bin/big/main.rs"]));
    assert_eq!(part.segments, strings(&["part"]));
    let helper = tree.locate("tests/it/helper.rs").unwrap();
    assert_eq!(helper.roots, strings(&["tests/it.rs"]));
    assert_eq!(helper.segments, strings(&["helper"]));
    let common = tree.locate("tests/suite/common.rs").unwrap();
    assert_eq!(common.roots, strings(&["tests/suite/main.rs"]));
    assert_eq!(common.segments, strings(&["common"]));
    assert!(tree.locate("build.rs").unwrap().segments.is_empty());
    assert!(tree.locate("src/tools/cli.rs").unwrap().segments.is_empty());
}

#[test]
fn an_orphan_with_no_chain_to_a_root_is_none() {
    let tree = tree();
    assert_eq!(tree.locate("src/orphan/deep.rs"), None);
    assert_eq!(tree.locate("README.md"), None);
}
