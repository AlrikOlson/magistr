use super::*;

/// Every kind whose rows are laid out by counting columns stays in the
/// mono voice. Written as the *whole* set rather than a sample, so a new
/// `RowKind` has to be classified here rather than inheriting whichever
/// arm it happens to land in.
#[test]
fn everything_that_is_column_aligned_stays_mono() {
    for kind in [
        RowKind::Bullet,
        RowKind::Code,
        RowKind::Fence,
        RowKind::Tool(ToolStatus::InProgress),
        RowKind::Check(true),
        RowKind::DiffAdd,
        RowKind::DiffRemove,
        RowKind::Conflict,
        RowKind::EditButtons,
        RowKind::Image,
        RowKind::Pane,
        RowKind::Event,
    ] {
        assert!(kind.text_role().is_mono(), "{kind:?} must stay mono");
    }
}

#[test]
fn sentences_are_prose_and_a_markdown_heading_is_a_heading() {
    assert_eq!(RowKind::Prose(Voice::Agent).text_role(), TextRole::Prose);
    assert_eq!(RowKind::Prose(Voice::User).text_role(), TextRole::Prose);
    assert_eq!(RowKind::Note.text_role(), TextRole::Prose);
    assert_eq!(RowKind::Error.text_role(), TextRole::Prose);
    assert_eq!(RowKind::Heading.text_role(), TextRole::Heading);
}
