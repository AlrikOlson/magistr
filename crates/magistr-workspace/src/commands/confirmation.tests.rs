use super::*;

#[tokio::test]
async fn unattended_binary_overwrite_is_previewable_journaled_and_undoable() {
    let mut h = Harness::with_config(TOKEN_MODE);
    let before = b"\x89PNG\r\n\x1a\n\0old";
    let after = b"\x89PNG\r\n\x1a\n\0new";
    h.write("current.png", before);
    h.write("capture.png", after);
    let mut input = files(FilesOp::Copy, Some("capture.png"), Some("current.png"));
    input.dry_run = true;
    let (rows, _) = ok(h.commands("s").files(&input).await);
    assert!(!rows[0].applied);
    assert_eq!(h.read("current.png").unwrap(), before);
    assert!(h.journal.list(0).unwrap().is_empty());

    input.dry_run = false;
    let (rows, _) = ok(h.commands("s").files(&input).await);
    assert!(rows[0].applied && rows[0].journal_seq.is_some());
    assert_eq!(h.read("current.png").unwrap(), after);
    assert_eq!(h.read("capture.png").unwrap(), after);
    assert_eq!(h.journal.list(0).unwrap().len(), 1);
    ok(h.commands("s").undo(&UndoInput::default()).await);
    assert_eq!(h.read("current.png").unwrap(), before);
}

#[tokio::test]
async fn unattended_writes_still_refuse_stale_anchors_and_invalid_confirmations() {
    let mut h = Harness::with_config(TOKEN_MODE);
    h.write("source.txt", "new");
    h.write("target.txt", "old");
    let mut input = files(FilesOp::Copy, Some("source.txt"), Some("target.txt"));
    input.expected_hash = Some("stale".into());
    assert!(matches!(
        refused(h.commands("s").files(&input).await).0,
        ErrorKind::StaleAnchor { .. }
    ));
    input.expected_hash = None;
    input.confirm = Some("mgw2.foreign".into());
    assert_eq!(
        refused(h.commands("s").files(&input).await).0,
        ErrorKind::ConfirmMismatch
    );
    assert_eq!(h.read("target.txt").unwrap(), b"old");
    assert!(h.journal.list(0).unwrap().is_empty());
}

#[tokio::test]
async fn human_and_forced_token_modes_keep_overwrite_and_move_gates() {
    for config in [
        "[edit]\nconfirm_mode = \"elicit\"\n",
        "[edit]\nconfirm_mode = \"token\"\nforce_confirm = true\n",
    ] {
        for operation in [FilesOp::Copy, FilesOp::Move] {
            let mut h = Harness::with_config(config);
            h.write("source.txt", "new");
            h.write("target.txt", "old");
            let mut input = files(operation, Some("source.txt"), Some("target.txt"));
            let (kind, _) = refused(h.commands("s").files(&input).await);
            let ErrorKind::ConfirmRequired { confirm_token } = kind else {
                panic!("expected confirmation");
            };
            assert_eq!(h.read("source.txt").unwrap(), b"new");
            assert_eq!(h.read("target.txt").unwrap(), b"old");
            assert!(h.journal.list(0).unwrap().is_empty());
            input.confirm = Some(confirm_token);
            let (rows, _) = ok(h.commands("s").files(&input).await);
            assert!(rows.iter().all(|row| row.applied));
            assert_eq!(h.read("target.txt").unwrap(), b"new");
        }
    }
}
