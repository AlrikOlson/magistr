use super::*;

#[test]
fn altered_evidence_or_context_cannot_reuse_a_plan_reference() {
    let original = briefing();
    let bytes = original.to_json().unwrap();
    for field in ["roadmap", "code", "routing_history"] {
        let mut wire: Value = serde_json::from_slice(&bytes).unwrap();
        wire["evidence"][field] = "different evidence".into();
        assert!(PeerBriefing::from_json(&serde_json::to_vec(&wire).unwrap(), &contract()).is_err());
    }
    for field in 0..4 {
        let mut changed = evidence();
        let mut plan = original.revision().body().clone();
        match field {
            0 => changed.roadmap.push('x'),
            1 => changed.code.push('x'),
            2 => changed.routing_history.push('x'),
            _ => plan.context.constraints.push("another constraint".into()),
        }
        if field < 3 {
            assert!(PeerBriefing::new(contract(), plan.clone(), changed.clone()).is_err());
        }
        plan.context = changed.context(plan.context.constraints);
        let next = PeerBriefing::new(contract(), plan, changed).unwrap();
        assert_ne!(
            next.revision().reference().digest,
            original.revision().reference().digest
        );
    }
}

#[test]
fn briefing_rejects_unknown_duplicate_and_oversized_input() {
    let bytes = briefing().to_json().unwrap();
    for field in 0..6 {
        let mut wire: Value = serde_json::from_slice(&bytes).unwrap();
        match field {
            0 => wire["schema_version"] = 99.into(),
            1 => wire["unexpected"] = true.into(),
            2 => wire["evidence"]["unexpected"] = true.into(),
            3 => wire["plan"]["run_id"] = "other".into(),
            4 => wire["body"]["context"]["constraints"] = json!(["replaced"]),
            _ => wire["contract"]["limits"]["max_attempts"] = 99.into(),
        }
        assert!(
            PeerBriefing::from_json(&serde_json::to_vec(&wire).unwrap(), &contract()).is_err(),
            "field {field}"
        );
    }
    let text = String::from_utf8(bytes).unwrap();
    for (old, new) in [
        (
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        ("\"evidence\":{", "\"evidence\":{\"code\":\"duplicate\","),
    ] {
        assert!(
            PeerBriefing::from_json(text.replacen(old, new, 1).as_bytes(), &contract()).is_err()
        );
    }
    let padded = format!(
        "{text}{}",
        " ".repeat(PeerBriefing::MAX_JSON_BYTES - text.len())
    );
    assert!(PeerBriefing::from_json(padded.as_bytes(), &contract()).is_ok());
    assert!(PeerBriefing::from_json(format!("{padded} ").as_bytes(), &contract()).is_err());
}

#[test]
fn evidence_is_nonblank_bounded_and_separates_content_roles() {
    for field in 0..4 {
        let mut evidence = evidence();
        match field {
            0 => evidence.roadmap.clear(),
            1 => evidence.code = " ".into(),
            2 => evidence.routing_history.clear(),
            _ => evidence.code = "x".repeat(PeerBriefing::MAX_JSON_BYTES),
        }
        let mut plan = body();
        plan.context = evidence.context(plan.context.constraints);
        assert!(PeerBriefing::new(contract(), plan, evidence).is_err());
    }
    let evidence = PeerEvidence {
        roadmap: "same".into(),
        code: "same".into(),
        routing_history: "same".into(),
    };
    let context = evidence.context(vec![]);
    assert_ne!(context.roadmap_revision, context.code_revision);
    assert_ne!(context.code_revision, context.routing_history_revision);
}

#[test]
fn every_fixed_identity_and_reference_field_is_required_for_both_receipts() {
    for field in 0..5 {
        let mut value = authority();
        let mut stranger = contract().peers[0].clone();
        match field {
            0 => stranger.peer = Peer::Fable,
            1 => stranger.session_id.push('x'),
            2 => stranger.model.provider.push('x'),
            3 => stranger.model.model.push('x'),
            _ => stranger.model.effort.push('x'),
        }
        assert!(value.briefing_for(&stranger).is_err());
        assert!(
            value
                .apply(
                    observation(),
                    AuthorityAction::DeliverBriefing {
                        peer: stranger.clone(),
                        briefing: briefing().revision().reference(),
                    }
                )
                .is_err()
        );
        deliver(&mut value, 0);
        let bytes = PeerRequest::AcknowledgeBriefing {
            briefing: briefing().revision().reference(),
        }
        .to_json()
        .unwrap();
        assert!(matches!(
            value.receive_peer(observation(), &stranger, &bytes),
            Err(AgreementError::UnknownPeer)
        ));
        assert!(propose(&mut value, 0).is_err());
    }
    for field in 0..4 {
        let mut value = authority();
        let mut reference = briefing().revision().reference();
        match field {
            0 => reference.project_id.push('x'),
            1 => reference.run_id.push('x'),
            2 => reference.revision += 1,
            _ => reference.digest.push('x'),
        }
        assert!(
            value
                .apply(
                    observation(),
                    AuthorityAction::DeliverBriefing {
                        peer: contract().peers[0].clone(),
                        briefing: reference.clone(),
                    }
                )
                .is_err()
        );
        deliver(&mut value, 0);
        assert!(
            send(
                &mut value,
                0,
                json!({"kind":"acknowledge_briefing","briefing":reference})
            )
            .is_err()
        );
        assert!(propose(&mut value, 0).is_err());
    }
}
