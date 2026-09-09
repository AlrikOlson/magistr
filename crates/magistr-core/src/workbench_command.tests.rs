use super::*;

fn env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn explicit_command_override_wins() {
    let cmd = WorkbenchCommand::resolve_with(
        env(&[("WORKBENCH_MCP", " /opt/wb/bin/workbench-mcp ")]),
        |_| true,
        |_| true,
    );
    assert_eq!(cmd.command, "/opt/wb/bin/workbench-mcp");
    assert!(cmd.leading_args.is_empty());
    assert!(cmd.available);
    assert_eq!(cmd.args("/proj"), vec!["/proj"]);
}

#[test]
fn installed_binary_is_preferred_over_a_checkout() {
    let cmd = WorkbenchCommand::resolve_with(
        env(&[("WORKBENCH_MCP_REPO", "/src/workbench-mcp")]),
        |bin| bin == "workbench-mcp" || bin == "bun",
        |_| true,
    );
    assert_eq!(cmd.command, "workbench-mcp");
    assert!(cmd.available);
    assert_eq!(cmd.args_json_fragment("/proj"), "\"/proj\"");
}

#[test]
fn checkout_runs_through_bun() {
    let cmd = WorkbenchCommand::resolve_with(
        env(&[("WORKBENCH_MCP_REPO", "/src/workbench-mcp")]),
        |bin| bin == "bun",
        |p| p == Path::new("/src/workbench-mcp/src/main.ts"),
    );
    assert_eq!(cmd.command, "bun");
    assert_eq!(
        cmd.leading_args,
        vec!["run", "/src/workbench-mcp/src/main.ts"]
    );
    assert!(cmd.available);
    assert_eq!(
        cmd.args_json_fragment("/proj"),
        "\"run\", \"/src/workbench-mcp/src/main.ts\", \"/proj\""
    );
    let entry = cmd.mcp_entry("/proj");
    assert_eq!(entry["command"], "bun");
    assert_eq!(entry["args"][2], "/proj");
    assert_eq!(entry["type"], "stdio");
}

#[test]
fn known_checkout_under_home_is_a_fallback() {
    let cmd = WorkbenchCommand::resolve_with(
        env(&[("HOME", "/Users/dev")]),
        |bin| bin == "bun",
        |p| p == Path::new("/Users/dev/Serious_Code/workbench-mcp/src/main.ts"),
    );
    assert_eq!(cmd.command, "bun");
    assert!(cmd.available);
}

#[test]
fn checkout_without_bun_falls_back_to_the_bare_name_with_a_hint() {
    let cmd = WorkbenchCommand::resolve_with(
        env(&[("WORKBENCH_MCP_REPO", "/src/workbench-mcp")]),
        |_| false,
        |_| true,
    );
    assert_eq!(cmd.command, "workbench-mcp");
    assert!(!cmd.available);
    let hint = cmd.install_hint().expect("hint when unavailable");
    assert!(hint.contains("bun install -g workbench-mcp"));
    assert!(hint.contains("WORKBENCH_MCP_REPO"));
}

#[test]
fn available_command_has_no_hint() {
    let cmd = WorkbenchCommand::resolve_with(env(&[]), |bin| bin == "workbench-mcp", |_| false);
    assert!(cmd.install_hint().is_none());
}

#[test]
fn template_vars_are_injected_but_never_overridden() {
    let cmd = WorkbenchCommand::resolve_with(env(&[]), |bin| bin == "workbench-mcp", |_| false);
    let mut vars = HashMap::new();
    cmd.inject_template_vars(&mut vars, "/proj");
    assert_eq!(vars["workbench_command"], "workbench-mcp");
    assert_eq!(vars["workbench_args"], "\"/proj\"");

    let mut preset = HashMap::from([("workbench_command".to_string(), "custom".to_string())]);
    cmd.inject_template_vars(&mut preset, "/proj");
    assert_eq!(preset["workbench_command"], "custom");
}

#[test]
fn args_are_json_escaped() {
    let cmd = WorkbenchCommand::resolve_with(env(&[]), |bin| bin == "workbench-mcp", |_| false);
    assert_eq!(
        cmd.args_json_fragment("C:/with \"quote\""),
        "\"C:/with \\\"quote\\\"\""
    );
}
