use super::*;
use alloc::vec;
#[test]
fn files_share_quoted_names_order_and_duplicate_handling() {
    let quoted = |tail: &str| {
        let token = "\"résumé draft.md\"";
        assert!(tail.starts_with(token));
        Ok(("résumé draft.md".to_owned(), token.len()))
    };
    assert_eq!(
        prompt_file_references(
            "Review @\"résumé draft.md\"\n@src/main.rs @src/main.rs user@example.com",
            quoted
        )
        .unwrap(),
        vec!["résumé draft.md", "src/main.rs"]
    );
    assert!(
        prompt_file_references("No files @", |_| unreachable!())
            .unwrap()
            .is_empty()
    );
    assert!(
        prompt_file_references("@\"unfinished", |_| Err("Incomplete name".to_owned())).is_err()
    );
    assert!(prompt_file_references("@\"bad\"", |_| Ok(("bad".to_owned(), 999))).is_err());
}
