use super::*;
use crate::commands::rust_move::{RustDeclarationChange, RustDeclarationKind};
use std::collections::BTreeMap;

fn fixture() -> BTreeMap<String, String> {
    [
        ("Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        ("src/lib.rs", "//! Fixture crate.\n\npub mod a;\npub mod c;\npub mod util;\n\npub use a::b::helper as root_helper;\n\npub fn run() -> u32 {\n    crate::a::b::helper() + a::b::helper() + c::from_c()\n}\n"),
        ("src/a.rs", "pub mod b;\npub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    self::b::helper()\n}\n\npub fn via_uniform() -> u32 {\n    b::helper()\n}\n"),
        ("src/a/b.rs", "use super::sibling;\n\n/// Helper.\npub fn helper() -> u32 {\n    crate::util::one() + sibling() + super::sibling()\n}\n"),
        ("src/a/other.rs", "pub fn twice() -> u32 {\n    super::b::helper() * 2\n}\n"),
        ("src/util.rs", "pub fn one() -> u32 {\n    1\n}\n"),
        ("src/c.rs", "pub mod d;\n\npub fn from_c() -> u32 {\n    crate::a::b::helper() + d::dd()\n}\n"),
        ("src/c/d.rs", "pub fn dd() -> u32 {\n    super::super::a::b::helper()\n}\n"),
    ]
    .into_iter()
    .map(|(path, content)| (path.to_owned(), content.to_owned()))
    .collect()
}

fn listing(files: &BTreeMap<String, String>) -> Vec<SourceEntry> {
    files
        .keys()
        .map(|path| SourceEntry {
            path: path.clone(),
            is_dir: false,
            junk: path.starts_with("target/"),
        })
        .collect()
}

fn plan_over(seed: &BTreeMap<String, String>, from: &str, to: &str) -> RustMovePlan {
    let read = |path: &str| seed.get(path).cloned();
    plan_rust_move(from, to, &seed[from], &listing(seed), &read)
}

fn content_of<'p>(plan: &'p RustMovePlan, path: &str) -> Option<&'p str> {
    plan.writes
        .iter()
        .find(|write| write.path == path)
        .map(|write| write.content.as_str())
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

#[test]
fn rename_in_place_rewrites_the_declaration_and_every_path_style_and_leaves_the_moved_file() {
    let plan = plan_over(&fixture(), "src/a/b.rs", "src/a/bee.rs");
    assert_eq!(plan.skipped, None);
    assert_eq!(plan.old_module, Some(strings(&["a", "b"])));
    assert_eq!(plan.new_module, Some(strings(&["a", "bee"])));
    assert_eq!(
        plan.declaration,
        Some(RustDeclarationChange {
            kind: RustDeclarationKind::Rename,
            from: strings(&["src/a.rs"]),
            to: None,
        })
    );
    assert_eq!(
        content_of(&plan, "src/a.rs"),
        Some(
            "pub mod bee;\npub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    self::bee::helper()\n}\n\npub fn via_uniform() -> u32 {\n    bee::helper()\n}\n"
        )
    );
    let lib = content_of(&plan, "src/lib.rs").unwrap();
    assert!(lib.contains("pub use a::bee::helper as root_helper;"));
    assert!(lib.contains("crate::a::bee::helper() + a::bee::helper() + c::from_c()"));
    assert!(
        content_of(&plan, "src/a/other.rs")
            .unwrap()
            .contains("super::bee::helper() * 2")
    );
    assert!(
        content_of(&plan, "src/c.rs")
            .unwrap()
            .contains("crate::a::bee::helper() + d::dd()")
    );
    assert!(
        content_of(&plan, "src/c/d.rs")
            .unwrap()
            .contains("super::super::a::bee::helper()")
    );
    assert_eq!(plan.moved_content, fixture()["src/a/b.rs"]);
    assert_eq!(plan.moved_rewrites, 0);
    assert_eq!(plan.manifest, None);
    assert!(plan.unrewritten.is_empty());
    assert_eq!(
        plan.notes[0],
        "rust: module crate::a::b → crate::a::bee; declaration renamed in \"src/a.rs\"; 8 paths rewritten across 5 files (contiguous crate::/super::/self::/uniform paths only; nothing synthesized)"
    );
    assert!(plan.notes.iter().any(|n| n.contains("from OTHER crates")));
}

#[test]
fn move_under_another_parent_moves_the_declaration_re_roots_super_and_goes_absolute_where_needed() {
    let plan = plan_over(&fixture(), "src/a/b.rs", "src/c/b.rs");
    assert_eq!(plan.skipped, None);
    assert_eq!(plan.new_module, Some(strings(&["c", "b"])));
    assert_eq!(
        plan.declaration,
        Some(RustDeclarationChange {
            kind: RustDeclarationKind::Move,
            from: strings(&["src/a.rs"]),
            to: Some("src/c.rs".into()),
        })
    );
    assert_eq!(
        content_of(&plan, "src/a.rs"),
        Some(
            "pub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    crate::c::b::helper()\n}\n\npub fn via_uniform() -> u32 {\n    crate::c::b::helper()\n}\n"
        )
    );
    assert_eq!(
        content_of(&plan, "src/c.rs"),
        Some(
            "pub mod d;\npub mod b;\n\npub fn from_c() -> u32 {\n    crate::c::b::helper() + d::dd()\n}\n"
        )
    );
    let lib = content_of(&plan, "src/lib.rs").unwrap();
    assert!(lib.contains("pub use c::b::helper as root_helper;"));
    assert!(lib.contains("crate::c::b::helper() + c::b::helper() + c::from_c()"));
    assert!(
        content_of(&plan, "src/a/other.rs")
            .unwrap()
            .contains("crate::c::b::helper() * 2")
    );
    assert!(
        content_of(&plan, "src/c/d.rs")
            .unwrap()
            .contains("super::super::c::b::helper()")
    );
    assert_eq!(
        plan.moved_content,
        "use crate::a::sibling;\n\n/// Helper.\npub fn helper() -> u32 {\n    crate::util::one() + sibling() + crate::a::sibling()\n}\n"
    );
    assert_eq!(plan.moved_rewrites, 2);
    assert!(plan.notes[0].contains("declaration moved from \"src/a.rs\" to \"src/c.rs\""));
    assert!(plan.notes[0].contains("plus 2 paths re-rooted in the moved file"));
}

#[test]
fn missing_destination_parent_removes_the_declaration_and_synthesizes_nothing() {
    let plan = plan_over(&fixture(), "src/a/b.rs", "src/zz/b.rs");
    assert_eq!(
        plan.declaration,
        Some(RustDeclarationChange {
            kind: RustDeclarationKind::RemoveOnly,
            from: strings(&["src/a.rs"]),
            to: None,
        })
    );
    assert!(!content_of(&plan, "src/a.rs").unwrap().contains("mod b;"));
    assert!(
        content_of(&plan, "src/lib.rs")
            .unwrap()
            .contains("crate::zz::b::helper()")
    );
    assert!(plan.writes.iter().all(|w| w.path != "src/zz.rs"));
    assert!(plan.notes.contains(&"no parent module file for \"src/zz\"/ (\"src/zz.rs\", \"src/zz/mod.rs\"): create one declaring `mod b;` and declare `mod zz;` in \"src/lib.rs\" yourself — nothing was synthesized".to_owned()));
}

#[test]
fn moving_up_to_the_root_directory_adds_the_declaration_to_the_owning_root() {
    let plan = plan_over(&fixture(), "src/a/b.rs", "src/b.rs");
    assert_eq!(
        plan.declaration,
        Some(RustDeclarationChange {
            kind: RustDeclarationKind::Move,
            from: strings(&["src/a.rs"]),
            to: Some("src/lib.rs".into()),
        })
    );
    let lib = content_of(&plan, "src/lib.rs").unwrap();
    assert!(lib.contains("mod a;\npub mod c;\npub mod util;\npub mod b;\n"));
    assert!(lib.contains("pub use b::helper as root_helper;"));
    assert!(lib.contains("crate::b::helper() + b::helper()"));
}

#[test]
fn both_lib_and_main_declaring_the_module_are_rewritten() {
    let mut seed = fixture();
    seed.insert(
        "src/main.rs".into(),
        "mod a;\nmod c;\nmod util;\n\nfn main() {\n    println!(\"{}\", a::b::helper());\n}\n"
            .into(),
    );
    let plan = plan_over(&seed, "src/a/b.rs", "src/a/bee.rs");
    assert_eq!(
        plan.declaration.as_ref().unwrap().from,
        strings(&["src/a.rs"])
    );
    assert!(
        content_of(&plan, "src/main.rs")
            .unwrap()
            .contains("a::bee::helper()")
    );
}

#[test]
fn manifest_facts_a_bin_path_follows_the_file_and_auto_targets_are_noted() {
    let seed: BTreeMap<String, String> = [
        ("Cargo.toml", "[package]\nname = \"fx\"\n\n[workspace]\nmembers = [\"crates/x\"]\n\n[[bin]]\nname = \"cli\"\npath = \"src/tools/cli.rs\"\n"),
        ("src/lib.rs", "pub fn lib() {}\n"),
        ("src/tools/cli.rs", "fn main() {}\n"),
        ("src/bin/other.rs", "fn main() {}\n"),
    ]
    .into_iter()
    .map(|(p, c)| (p.to_owned(), c.to_owned()))
    .collect();
    let pinned = plan_over(&seed, "src/tools/cli.rs", "src/bin/cli.rs");
    assert_eq!(pinned.manifest, Some(("Cargo.toml".into(), 1)));
    let manifest = content_of(&pinned, "Cargo.toml").unwrap();
    assert!(manifest.contains("path = \"src/bin/cli.rs\""));
    assert!(manifest.contains("members = [\"crates/x\"]"));
    assert!(
        pinned
            .skipped
            .as_deref()
            .unwrap()
            .contains("crate root named by a Cargo.toml target path")
    );
    let auto = plan_over(&seed, "src/bin/other.rs", "src/bin/renamed.rs");
    assert_eq!(auto.manifest, None);
    assert!(auto.writes.is_empty());
    assert!(auto.notes[0].contains("crate root discovered by location"));
    let unrelated = plan_over(&seed, "src/lib.rs", "src/lib2.rs");
    assert!(unrelated.writes.iter().all(|w| w.path != "Cargo.toml"));
}

#[test]
fn negatives_non_leaf_undiscoverable_path_attr_style_conversion_and_outside_the_tree() {
    let non_leaf = plan_over(&fixture(), "src/a.rs", "src/aa.rs");
    assert_eq!(
        non_leaf.skipped.as_deref(),
        Some(
            "\"src/a.rs\" has child modules (b.rs, other.rs) and directories are not movable in v1, so they cannot follow"
        )
    );
    assert!(non_leaf.writes.is_empty());

    let mut orphan_seed = fixture();
    orphan_seed.insert("src/orphan.rs".into(), "pub fn o() {}\n".into());
    let orphan = plan_over(&orphan_seed, "src/orphan.rs", "src/o2.rs");
    assert_eq!(
        orphan.skipped.as_deref(),
        Some("no `mod orphan;` declaration found in \"src/lib.rs\"")
    );

    let mut chain_seed = fixture();
    chain_seed.insert("src/q/deep.rs".into(), "pub fn o() {}\n".into());
    let no_chain = plan_over(&chain_seed, "src/q/deep.rs", "src/q/d2.rs");
    assert!(
        no_chain
            .skipped
            .as_deref()
            .unwrap()
            .contains("no module chain")
    );

    let mut attr_seed = fixture();
    attr_seed.insert("src/a.rs".into(), "#[path = \"b.rs\"]\npub mod b;\n".into());
    let path_attr = plan_over(&attr_seed, "src/a/b.rs", "src/a/bee.rs");
    assert!(
        path_attr
            .skipped
            .as_deref()
            .unwrap()
            .contains("#[path] attribute")
    );

    let style = plan_over(&fixture(), "src/c/d.rs", "src/c/d/mod.rs");
    assert!(
        style
            .skipped
            .as_deref()
            .unwrap()
            .contains("style conversion of the same module")
    );
    let mut back_seed = fixture();
    back_seed.insert("src/m/mod.rs".into(), "pub fn m() {}\n".into());
    let style_back = plan_over(&back_seed, "src/m/mod.rs", "src/m.rs");
    assert!(
        style_back
            .skipped
            .as_deref()
            .unwrap()
            .contains("style conversion of the same module")
    );

    let outside = plan_over(&fixture(), "src/a/b.rs", "tests/b.rs");
    assert!(
        outside
            .skipped
            .as_deref()
            .unwrap()
            .contains("outside the module tree")
    );
}

#[test]
fn junk_files_are_never_scanned_and_other_crates_are_not_touched() {
    let mut seed = fixture();
    seed.insert(
        "target/debug/build/gen.rs".into(),
        "use crate::a::b::helper;\n".into(),
    );
    seed.insert(
        "crates/other/Cargo.toml".into(),
        "[package]\nname = \"other\"\n".into(),
    );
    seed.insert(
        "crates/other/src/lib.rs".into(),
        "use fixture::a::b::helper;\nuse crate::a::b::x;\n".into(),
    );
    seed.insert("tests/it.rs".into(), "use crate::a::b::helper;\n".into());
    let plan = plan_over(&seed, "src/a/b.rs", "src/a/bee.rs");
    let touched: Vec<&str> = plan.writes.iter().map(|w| w.path.as_str()).collect();
    assert!(!touched.contains(&"target/debug/build/gen.rs"));
    assert!(!touched.contains(&"crates/other/src/lib.rs"));
    assert!(!touched.contains(&"tests/it.rs"));
}
