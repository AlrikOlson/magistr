use super::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

struct Fixture {
    resolve: Vec<String>,
    emit: Vec<String>,
    declared_before: BTreeSet<String>,
    declared_after: BTreeSet<String>,
    old: Vec<String>,
    new: Vec<String>,
}

impl Fixture {
    fn default() -> Self {
        Self {
            resolve: strings(&["a"]),
            emit: strings(&["a"]),
            declared_before: set(&["b"]),
            declared_after: set(&["bee"]),
            old: strings(&["a", "b"]),
            new: strings(&["a", "bee"]),
        }
    }

    fn ctx(&self) -> RewriteContext<'_> {
        RewriteContext {
            resolve_module: Some(&self.resolve),
            emit_module: Some(&self.emit),
            declared_before: &self.declared_before,
            declared_after: &self.declared_after,
            old_module: &self.old,
            new_module: &self.new,
        }
    }
}

#[test]
fn rename_in_place_keeps_every_path_style() {
    let fixture = Fixture::default();
    let ctx = fixture.ctx();
    assert_eq!(
        rewrite_paths("crate::a::b::helper()", &ctx).0,
        "crate::a::bee::helper()"
    );
    assert_eq!(
        rewrite_paths("self::b::helper()", &ctx).0,
        "self::bee::helper()"
    );
    assert_eq!(rewrite_paths("b::helper()", &ctx).0, "bee::helper()");
    let sibling = Fixture {
        resolve: strings(&["a", "other"]),
        emit: strings(&["a", "other"]),
        declared_before: set(&[]),
        declared_after: set(&[]),
        ..Fixture::default()
    };
    assert_eq!(
        rewrite_paths("super::b::helper()", &sibling.ctx()).0,
        "super::bee::helper()"
    );
    assert_eq!(
        rewrite_paths("use crate::a::b::*;\npub use crate::a::b;", &ctx).0,
        "use crate::a::bee::*;\npub use crate::a::bee;"
    );
}

#[test]
fn segment_boundaries_a_shared_prefix_is_not_the_module() {
    let fixture = Fixture::default();
    let (content, count, _) = rewrite_paths(
        "crate::a::bb::x; crate::ab::b::y; crate::a::b_x::z",
        &fixture.ctx(),
    );
    assert_eq!(count, 0);
    assert_eq!(
        content,
        "crate::a::bb::x; crate::ab::b::y; crate::a::b_x::z"
    );
}

#[test]
fn directory_move_relative_forms_that_no_longer_reach_become_absolute() {
    let fixture = Fixture {
        new: strings(&["c", "b"]),
        declared_after: set(&[]),
        ..Fixture::default()
    };
    assert_eq!(
        rewrite_paths("self::b::helper() + b::helper()", &fixture.ctx()).0,
        "crate::c::b::helper() + crate::c::b::helper()"
    );
    let deep = Fixture {
        resolve: strings(&["c", "d"]),
        emit: strings(&["c", "d"]),
        new: strings(&["c", "b"]),
        declared_before: set(&[]),
        declared_after: set(&[]),
        ..Fixture::default()
    };
    assert_eq!(
        rewrite_paths("super::super::a::b::helper()", &deep.ctx()).0,
        "super::super::c::b::helper()"
    );
}

#[test]
fn the_moved_file_re_roots_super_and_keeps_self_and_unrelated_crate_paths() {
    let moved = Fixture {
        resolve: strings(&["a", "b"]),
        emit: strings(&["c", "b"]),
        new: strings(&["c", "b"]),
        declared_before: set(&["inner"]),
        declared_after: set(&["inner"]),
        ..Fixture::default()
    };
    let (content, count, _) = rewrite_paths(
        "use super::sibling;\nsuper::sibling() + crate::util::one() + self::inner::f() + inner::g()",
        &moved.ctx(),
    );
    assert_eq!(
        content,
        "use crate::a::sibling;\ncrate::a::sibling() + crate::util::one() + self::inner::f() + inner::g()"
    );
    assert_eq!(count, 2);
}

#[test]
fn a_use_binding_of_the_module_follows_a_rename_and_survives_a_move() {
    let root = Fixture {
        resolve: Vec::new(),
        emit: Vec::new(),
        declared_before: set(&["a"]),
        declared_after: set(&["a"]),
        ..Fixture::default()
    };
    assert_eq!(
        rewrite_paths(
            "use crate::a::b;\nfn f() -> u32 { b::helper() }",
            &root.ctx()
        )
        .0,
        "use crate::a::bee;\nfn f() -> u32 { bee::helper() }"
    );
    assert_eq!(
        rewrite_paths(
            "use crate::a::b as bb;\nfn f() -> u32 { bb::helper() }",
            &root.ctx()
        )
        .0,
        "use crate::a::bee as bb;\nfn f() -> u32 { bb::helper() }"
    );
    let move_root = Fixture {
        new: strings(&["c", "b"]),
        ..root
    };
    assert_eq!(
        rewrite_paths(
            "use crate::a::b;\nfn f() -> u32 { b::helper() }",
            &move_root.ctx()
        )
        .0,
        "use crate::c::b;\nfn f() -> u32 { b::helper() }"
    );
}

#[test]
fn brace_group_members_naming_the_module_are_reported_not_rewritten() {
    let root = Fixture {
        resolve: Vec::new(),
        emit: Vec::new(),
        declared_before: set(&["a"]),
        declared_after: set(&["a"]),
        ..Fixture::default()
    };
    let (content, _, unrewritten) = rewrite_paths(
        "use crate::util::one;\nuse crate::a::{b, other};\nuse crate::a::b::{helper};\n",
        &root.ctx(),
    );
    assert_eq!(
        content,
        "use crate::util::one;\nuse crate::a::{b, other};\nuse crate::a::bee::{helper};\n"
    );
    assert_eq!(
        unrewritten,
        vec![(2, "use crate::a::{b, other}".to_owned())]
    );
}
