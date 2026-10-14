use super::*;
use alloc::vec;

#[test]
fn attachments_keep_exact_text_bytes_and_identity_when_cloned() {
    let bytes = "first\r\n第二行\n".as_bytes().to_vec();
    let attachment =
        PromptAttachment::from_bytes("guide ü.md", "version-one", bytes.clone()).unwrap();
    assert_eq!(attachment.bytes(), bytes);
    assert_eq!(attachment.text(), Some("first\r\n第二行\n"));
    assert_eq!(attachment.kind(), PromptAttachmentKind::Text);
    assert_eq!(attachment.name(), "guide ü.md");
    assert_eq!(attachment.content_hash(), "version-one");
    let cloned = attachment.clone();
    assert_eq!(attachment.id(), cloned.id());
    assert!(Arc::ptr_eq(&attachment.bytes, &cloned.bytes));
    assert_ne!(
        attachment,
        PromptAttachment::from_bytes("guide ü.md", "version-two", bytes).unwrap()
    );
}

#[test]
fn selecting_identical_bytes_again_creates_a_distinct_receipt() {
    let first =
        PromptAttachment::from_bytes("notes.txt", "same-hash", b"same bytes".to_vec()).unwrap();
    let second =
        PromptAttachment::from_bytes("notes.txt", "same-hash", b"same bytes".to_vec()).unwrap();
    assert_eq!(first.bytes(), second.bytes());
    assert_ne!(first.id(), second.id());
    let input = PromptInput {
        text: "Review".into(),
        attachments: vec![first],
    };
    assert_eq!(input.receipt(), input.clone().receipt());
    let replaced = PromptInput {
        text: input.text.clone(),
        attachments: vec![second],
    };
    assert_ne!(input.receipt(), replaced.receipt());
}

#[test]
fn file_type_comes_from_bytes_and_images_require_negotiated_support() {
    let bytes = b"\x89PNG\r\n\x1a\nexact image bytes".to_vec();
    let attachment = PromptAttachment::from_bytes("picture.txt", "hash", bytes.clone()).unwrap();
    assert_eq!(attachment.kind(), PromptAttachmentKind::Image);
    assert_eq!(attachment.mime_type(), Some("image/png"));
    assert_eq!(attachment.text(), None);
    assert_eq!(attachment.bytes(), bytes);
    let input = PromptInput {
        text: String::new(),
        attachments: vec![attachment],
    };
    assert!(!input.is_empty());
    assert!(
        input
            .validate(false)
            .unwrap_err()
            .contains("does not accept images")
    );
    assert_eq!(input.validate(true), Ok(()));
}

#[test]
fn binary_unsupported_and_oversize_files_are_refused_without_truncation() {
    for bytes in [vec![0xff, 1], b"text\0binary".to_vec()] {
        assert!(PromptAttachment::from_bytes("file", "hash", bytes).is_err());
    }
    assert!(
        PromptAttachment::from_bytes("large.txt", "hash", vec![b'a'; MAX_ATTACHMENT_BYTES + 1])
            .unwrap_err()
            .contains("1 MB")
    );
    assert!(
        PromptAttachment::from_bytes("limit.txt", "hash", vec![b'a'; MAX_ATTACHMENT_BYTES]).is_ok()
    );
}

#[test]
fn prompt_and_file_count_have_shared_bounds() {
    let attachment = PromptAttachment::from_bytes("a.txt", "hash", vec![b'a']).unwrap();
    let input = PromptInput {
        text: "Review".into(),
        attachments: vec![attachment; MAX_ATTACHMENTS + 1],
    };
    assert!(input.validate(true).unwrap_err().contains("8 files"));
    let input: PromptInput = "a".repeat(MAX_PROMPT_BYTES + 1).into();
    assert!(input.validate(true).unwrap_err().contains("too long"));
    let input: PromptInput = "person@example.com and @literal".into();
    assert_eq!(input.validate(false), Ok(()));
    assert!(input.attachments.is_empty());
    assert_eq!(input.byte_len(), input.text.len());
}

#[test]
fn acceptance_receipts_keep_ordered_identity_without_file_payloads() {
    let input = PromptInput {
        text: "Review the files".into(),
        attachments: vec![
            PromptAttachment::from_bytes("first.txt", "hash-one", vec![b'a'; MAX_ATTACHMENT_BYTES])
                .unwrap(),
            PromptAttachment::from_bytes("second.txt", "hash-two", b"second file".to_vec())
                .unwrap(),
        ],
    };
    let receipt = input.receipt();
    assert_eq!(receipt.text, input.text);
    assert_eq!(
        receipt.attachments,
        vec![
            AttachmentReceipt {
                id: input.attachments[0].id().into(),
                name: "first.txt".into(),
                content_hash: "hash-one".into(),
                kind: PromptAttachmentKind::Text,
                byte_len: MAX_ATTACHMENT_BYTES
            },
            AttachmentReceipt {
                id: input.attachments[1].id().into(),
                name: "second.txt".into(),
                content_hash: "hash-two".into(),
                kind: PromptAttachmentKind::Text,
                byte_len: 11
            },
        ]
    );
    assert!(alloc::format!("{receipt:?}").len() < 512);
}

#[test]
fn attachment_identity_is_bounded_before_admission() {
    for (name, hash) in [
        (String::new(), "hash".into()),
        ("a".repeat(MAX_ATTACHMENT_NAME_BYTES + 1), "hash".into()),
        ("a.txt".into(), String::new()),
        ("a.txt".into(), "h".repeat(MAX_ATTACHMENT_HASH_BYTES + 1)),
    ] {
        assert!(PromptAttachment::from_bytes(name, hash, b"text".to_vec()).is_err());
    }
}
