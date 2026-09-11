use super::*;
use crate::jail::WorkspaceKeyStore;

fn fixture() -> (tempfile::TempDir, WorkspaceKeyStore, ConfirmationSeal) {
    let dir = tempfile::tempdir().expect("temporary state");
    let store = WorkspaceKeyStore::new(dir.path().join("state"));
    let seal = ConfirmationSeal::new(&store.ensure().expect("private key"));
    (dir, store, seal)
}

#[test]
fn roundtrip_survives_key_reload_but_not_rotation_or_foreign_workspace() {
    let (_dir, store, seal) = fixture();
    let token = seal.seal("alice", "edit", b"{\"old\":1}", b"before\0after");
    assert_eq!(token.len(), 69);
    let loaded = ConfirmationSeal::new(&store.ensure().expect("same key"));
    assert_eq!(
        loaded.verify(&token, "alice", "edit", b"{\"old\":1}", b"before\0after"),
        Ok(())
    );
    let rotated = ConfirmationSeal::new(&store.rotate().expect("rotate"));
    assert_eq!(
        rotated.verify(&token, "alice", "edit", b"{\"old\":1}", b"before\0after"),
        Err(ConfirmationError::Mismatch)
    );
    let (_other, _store, foreign) = fixture();
    assert!(
        foreign
            .verify(&token, "alice", "edit", b"{\"old\":1}", b"before\0after")
            .is_err()
    );
}

#[test]
fn binds_identity_operation_exact_serialization_and_final_bytes() {
    let (_dir, _store, seal) = fixture();
    let token = seal.seal("alice", "edit", b"{\"a\":1,\"b\":2}", b"p\0before\0after");
    for (session, op, args, plan) in [
        (
            "bob",
            "edit",
            b"{\"a\":1,\"b\":2}".as_slice(),
            b"p\0before\0after".as_slice(),
        ),
        ("alice", "undo", b"{\"a\":1,\"b\":2}", b"p\0before\0after"),
        ("alice", "edit", b"{\"b\":2,\"a\":1}", b"p\0before\0after"),
        ("alice", "edit", b"{ \"a\":1,\"b\":2}", b"p\0before\0after"),
        (
            "alice",
            "edit",
            b"{\"a\":1,\"b\":2}",
            b"p\0before\0formatted",
        ),
        (
            "alice",
            "edit",
            b"{\"a\":1,\"b\":2}",
            b"p\0different\0after",
        ),
    ] {
        assert_eq!(
            seal.verify(&token, session, op, args, plan),
            Err(ConfirmationError::Mismatch)
        );
    }
}

#[test]
fn field_framing_prevents_separator_and_partition_aliases() {
    let (_dir, _store, seal) = fixture();
    let token = seal.seal("a\nb", "c", b"x\0y", b"z");
    assert!(seal.verify(&token, "a", "b\nc", b"x\0y", b"z").is_err());
    assert!(seal.verify(&token, "a\nb", "c", b"x", b"y\0z").is_err());
}

#[test]
fn rejects_forged_malformed_and_retained_tokens() {
    let (_dir, _store, seal) = fixture();
    let token = seal.seal("a", "edit", b"{}", b"plan");
    for legacy in ["wb1.a.b.c", "mgw2.any", ""] {
        assert_eq!(
            seal.verify(legacy, "a", "edit", b"{}", b"plan"),
            Err(ConfirmationError::UnsupportedVersion)
        );
    }
    for malformed in [
        "mgw1.",
        "mgw1.x",
        &format!("{token}.extra"),
        &format!("mgw1.{}", "g".repeat(64)),
    ] {
        assert_eq!(
            seal.verify(malformed, "a", "edit", b"{}", b"plan"),
            Err(ConfirmationError::Mismatch)
        );
    }
    let mut forged = token.into_bytes();
    let last = forged.last_mut().expect("tag");
    *last = if *last == b'0' { b'1' } else { b'0' };
    assert!(
        seal.verify(
            &String::from_utf8(forged).expect("hex"),
            "a",
            "edit",
            b"{}",
            b"plan"
        )
        .is_err()
    );
}
