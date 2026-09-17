use super::*;
use crate::render::width;
use magistr_application::sessions::{ToolStatus, ToolVerb};

#[test]
fn every_line_kind_maps_to_its_role() {
    assert_eq!(Accent::of(&LineKind::Prose(Voice::User)), Accent::User);
    assert_eq!(Accent::of(&LineKind::Prose(Voice::Agent)), Accent::Agent);
    assert_eq!(
        Accent::of(&LineKind::Prose(Voice::Thought)),
        Accent::Thought
    );
    assert_eq!(
        Accent::of(&LineKind::Tool {
            id: "t1".to_owned(),
            status: ToolStatus::InProgress,
            verb: ToolVerb::Read,
            duration_ms: None,
            expanded: false,
        }),
        Accent::Tool
    );
    assert_eq!(
        Accent::of(&LineKind::Check {
            name: "test".to_owned(),
            passed: true,
            exit_code: Some(0),
            duration_ms: None,
            expanded: false,
        }),
        Accent::Tool
    );
    assert_eq!(
        Accent::of(&LineKind::Edit {
            id: "t2".to_owned(),
            path: "src/main.rs".to_owned(),
            status: ToolStatus::Completed,
            expanded: false,
            undone: false,
        }),
        Accent::Tool
    );
    assert_eq!(Accent::of(&LineKind::Event), Accent::Meta);
    assert_eq!(Accent::of(&LineKind::Note), Accent::Meta);
    assert_eq!(Accent::of(&LineKind::Error), Accent::Meta);
}

#[test]
fn every_gutter_is_the_gutter_width() {
    for accent in [
        Accent::User,
        Accent::Agent,
        Accent::Thought,
        Accent::Tool,
        Accent::Meta,
    ] {
        assert_eq!(
            width(accent.gutter()),
            Accent::GUTTER_COLS,
            "{accent:?}'s gutter"
        );
    }
}

#[test]
fn only_meta_rows_go_bare() {
    assert_eq!(Accent::Meta.gutter().trim(), "");
    for accent in [Accent::User, Accent::Agent, Accent::Thought, Accent::Tool] {
        assert!(
            !accent.gutter().trim().is_empty(),
            "{accent:?} carries a bar"
        );
    }
}
