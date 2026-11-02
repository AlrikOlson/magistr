use super::*;
#[test]
fn guide_uses_public_catalog_and_explains_advanced_authorization() {
    let guide = build_tool_guide();
    for (_, name, description) in crate::unified_mcp::DIRECT {
        assert!(guide.contains(&format!("| `{name}` | {description} |")));
    }
    for required in [
        "discover",
        "execute",
        "magistr.call",
        "same authorization",
        "never replay",
        "verification: \"focused\"",
        "plan_exports",
        "check_step",
        "run_finish",
    ] {
        assert!(guide.contains(required), "missing {required}");
    }
    for obsolete in [
        "magistr_delete",
        "magistr_list",
        "workspace-mcp",
        "plan_work",
        "start_work",
        "ship_status",
    ] {
        assert!(
            !guide.contains(obsolete),
            "retired public interface {obsolete}"
        );
    }
}
#[test]
fn tool_guide_snapshot() {
    insta::assert_snapshot!("tools_md", build_tool_guide());
}
