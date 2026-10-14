use super::*;

fn address(session: &str) -> ConversationTarget {
    ConversationTarget {
        conversation: "history".into(),
        address: magistr_domain::identity::SessionAddress {
            project: "project".into(),
            workspace: "working-copy".into(),
            session: session.into(),
        },
    }
}

fn inputs() -> NativeAttachments {
    let inputs = NativeAttachments::default();
    inputs.subscribe(Channel::new(|_| Ok(()))).unwrap();
    inputs
}

#[test]
fn a_native_drop_is_bound_to_the_full_address_and_stream_and_consumed_once() {
    let inputs = inputs();
    let target = address("one");
    inputs.dropped(
        Some((3, target.clone())),
        vec![PathBuf::from("/chosen/file")],
    );
    assert!(inputs.take(1, &address("two"), 3).is_err());
    assert!(inputs.take(1, &target, 4).is_err());
    let mut other_conversation = target.clone();
    other_conversation.conversation = "planning".into();
    assert!(inputs.take(1, &other_conversation, 3).is_err());
    let mut other_workspace = target.clone();
    other_workspace.address.workspace = "other-copy".into();
    assert!(inputs.take(1, &other_workspace, 3).is_err());
    assert_eq!(
        inputs.take(1, &target, 3).unwrap(),
        vec![PathBuf::from("/chosen/file")]
    );
    assert!(inputs.take(1, &target, 3).is_err());
}

#[test]
fn a_new_drop_replaces_the_previous_grant_and_an_invalid_drop_revokes_it() {
    let inputs = inputs();
    let target = address("one");
    inputs.dropped(Some((2, target.clone())), vec![PathBuf::from("first")]);
    inputs.dropped(Some((2, target.clone())), vec![PathBuf::from("second")]);
    assert!(inputs.take(1, &target, 2).is_err());
    assert_eq!(
        inputs.take(2, &target, 2).unwrap(),
        vec![PathBuf::from("second")]
    );
    inputs.dropped(Some((2, target.clone())), vec![PathBuf::from("third")]);
    inputs.dropped(
        Some((2, target.clone())),
        vec![PathBuf::from("large"); MAX_ATTACHMENTS + 1],
    );
    assert!(inputs.take(3, &target, 2).is_err());
}

#[test]
fn unsubscribed_or_unaddressed_drops_retain_no_file_authority() {
    let inputs = NativeAttachments::default();
    let target = address("one");
    inputs.dropped(Some((1, target.clone())), vec![PathBuf::from("file")]);
    assert!(inputs.take(1, &target, 1).is_err());
    inputs.subscribe(Channel::new(|_| Ok(()))).unwrap();
    inputs.dropped(None, vec![PathBuf::from("file")]);
    assert!(inputs.drops.lock().unwrap().pending.is_none());
}
