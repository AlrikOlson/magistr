use super::*;

const PARENT: &str = "//! docs\n#![allow(dead_code)]\n\n/// The b module.\n#[cfg(test)]\npub(crate) mod b; // trailing\nmod other;\nmod inline { }\n";

#[test]
fn declared_modules_sees_file_and_inline_modules() {
    let declared: Vec<_> = declared_modules(PARENT).into_iter().collect();
    assert_eq!(declared, vec!["b", "inline", "other"]);
}

#[test]
fn find_declaration_spans_attributes_and_docs_and_reports_pub_and_path() {
    assert_eq!(
        find_declaration(PARENT, "b"),
        Some(DeclarationItem {
            start_line: 3,
            decl_line: 5,
            has_path_attr: false,
            is_pub: true,
        })
    );
    assert_eq!(
        find_declaration(PARENT, "other"),
        Some(DeclarationItem {
            start_line: 6,
            decl_line: 6,
            has_path_attr: false,
            is_pub: false,
        })
    );
    assert_eq!(find_declaration(PARENT, "inline"), None);
    assert_eq!(find_declaration(PARENT, "missing"), None);
    assert!(
        find_declaration("#[path = \"x.rs\"]\nmod b;\n", "b")
            .unwrap()
            .has_path_attr
    );
}

#[test]
fn rename_declaration_keeps_visibility_attributes_and_the_trailing_comment() {
    assert!(
        rename_declaration(PARENT, "b", "bee")
            .contains("#[cfg(test)]\npub(crate) mod bee; // trailing\n")
    );
    // Segment boundaries: `mod bb;` is not `mod b;`.
    assert_eq!(
        rename_declaration("mod bb;\nmod b;\n", "b", "c"),
        "mod bb;\nmod c;\n"
    );
}

#[test]
fn remove_and_insert_declaration_follow_the_retained_placement_rules() {
    let item = find_declaration(PARENT, "b").unwrap();
    assert_eq!(
        remove_declaration(PARENT, &item),
        "//! docs\n#![allow(dead_code)]\n\nmod other;\nmod inline { }\n"
    );
    assert_eq!(
        insert_declaration("mod x;\nmod y;\n\nfn f() {}\n", "pub mod b;"),
        "mod x;\nmod y;\npub mod b;\n\nfn f() {}\n"
    );
    assert_eq!(
        insert_declaration("//! inner\n#![no_std]\nfn f() {}\n", "pub mod b;"),
        "//! inner\n#![no_std]\n\npub mod b;\n\nfn f() {}\n"
    );
    assert_eq!(
        insert_declaration("fn f() {}\n", "mod b;"),
        "mod b;\n\nfn f() {}\n"
    );
}
