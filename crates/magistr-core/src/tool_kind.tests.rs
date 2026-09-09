use super::*;

#[test]
fn all_variants_have_icons() {
    let kinds = [
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Delete,
        ToolKind::Move,
        ToolKind::Search,
        ToolKind::Execute,
        ToolKind::Think,
        ToolKind::Fetch,
        ToolKind::SwitchMode,
        ToolKind::Other,
    ];
    for kind in &kinds {
        assert!(
            !kind.icon().is_empty(),
            "ToolKind {:?} has empty icon",
            kind
        );
    }
}

#[test]
fn equality() {
    assert_eq!(ToolKind::Read, ToolKind::Read);
    assert_ne!(ToolKind::Read, ToolKind::Edit);
}
