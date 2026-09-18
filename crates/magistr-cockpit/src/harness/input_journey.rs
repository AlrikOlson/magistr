//! One fixture journey exercised by real terminal and native input adapters.
use crate::cockpit::{Cmd, Model, Msg, Selected, StageTab, update};
use magistr_application::sessions::{
    AgentEvent, AskPlan, PermissionChoice, PermissionKind, TurnEnd,
};

/// A user gesture; each renderer translates it through its actual input controls.
#[derive(Clone, Copy, Debug)]
pub enum InputJourney {
    Navigate(&'static str),
    Text(&'static str),
    Send,
    Stop,
    AllowOnce,
    AllowAlways,
}

impl InputJourney {
    /// Verify the same intent and retained context on every input adapter.
    pub fn verify(model: &mut Model, mut drive: impl FnMut(&mut Model, Self) -> Vec<Cmd>) {
        let draft = model.prompt().clone();
        let anchor = model.turn().anchor_at(model.top_row()).map(|a| a.line());
        for (query, tab) in [
            ("Retry delivery", StageTab::Roadmap),
            ("Recorded work", StageTab::Session),
            ("Conversation", StageTab::Turn),
        ] {
            drive(model, Self::Navigate(query));
            assert_eq!(model.tab(), tab);
            assert_eq!(
                model.selected(),
                Some(&Selected::Chunk("retry-work".into()))
            );
            assert_eq!(model.prompt(), &draft);
        }
        assert_eq!(
            model.turn().anchor_at(model.top_row()).map(|a| a.line()),
            anchor
        );
        let sent = drive(model, Self::Send);
        assert!(sent.contains(&Cmd::Prompt(draft.text().into())), "{sent:?}");
        drive(model, Self::Text("Keep @src/retry.rs "));
        let newer = model.prompt().clone();
        assert!(drive(model, Self::Stop).contains(&Cmd::Cancel));
        assert_eq!(model.prompt(), &newer);
        update(
            model,
            Msg::Agent(AgentEvent::PromptRejected {
                text: draft.text().into(),
                error: "Files are unavailable in this session.".into(),
            }),
        );
        assert_eq!(model.prompt(), &newer);
        assert!(
            model
                .prompt()
                .history()
                .iter()
                .any(|text| text == draft.text())
        );
        update(model, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
        drive(model, Self::Navigate("Roadmap"));
        let planning = drive(model, Self::Send);
        assert!(
            planning.contains(&Cmd::RoadmapPrompt(newer.text().into())),
            "{planning:?}"
        );
        drive(model, Self::Text("Keep this request"));
        assert!(drive(model, Self::Stop).contains(&Cmd::CancelRoadmapPrompt));
        assert_eq!(model.prompt().text(), "Keep this request");
        update(model, Msg::RoadmapRequestCancelled);
        drive(model, Self::Navigate("Conversation"));
        update(
            model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 42,
                title: "Read source".into(),
                edit: false,
                risk: magistr_domain::policy::RiskTier::Low,
                choices: vec![PermissionChoice::new("once", PermissionKind::AllowOnce)],
                plan: Some(AskPlan::new(["Read source"])),
            }),
        );
        let before = model.prompt().clone();
        assert!(
            !drive(model, Self::AllowAlways)
                .iter()
                .any(|c| matches!(c, Cmd::Permission { .. }))
        );
        assert_eq!(model.permissions_pending(), 1);
        assert!(drive(model, Self::AllowOnce).contains(&Cmd::Permission {
            id: 42,
            option_id: "once".into()
        }));
        assert_eq!(model.permissions_pending(), 0);
        assert_eq!(model.prompt(), &before);
    }
}
