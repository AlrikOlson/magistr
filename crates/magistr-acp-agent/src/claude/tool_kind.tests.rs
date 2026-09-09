use super::*;

#[test]
fn the_built_in_tools_map_to_their_kinds_and_the_rest_to_other() {
    assert_eq!(tool_kind("Bash"), ToolKind::Execute);
    assert_eq!(tool_kind("Read"), ToolKind::Read);
    assert_eq!(tool_kind("Grep"), ToolKind::Search);
    assert_eq!(tool_kind("Write"), ToolKind::Edit);
    assert_eq!(tool_kind("Edit"), ToolKind::Edit);
    assert_eq!(tool_kind("WebFetch"), ToolKind::Fetch);
    assert_eq!(tool_kind("Agent"), ToolKind::Think);
    assert_eq!(tool_kind("ExitPlanMode"), ToolKind::SwitchMode);
    assert_eq!(tool_kind("mcp__magistr__magistr_gate"), ToolKind::Other);
    assert_eq!(tool_kind(""), ToolKind::Other);
}
