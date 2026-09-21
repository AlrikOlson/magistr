use super::*;

const PROJECT: &str = "/proj";

fn refused(command: &str) -> Option<String> {
    workspace_search_refusal(command, Path::new(PROJECT))
}

#[test]
fn searches_over_the_workspace_are_routed_to_workbench() {
    for cmd in [
        "grep -rn foo src",
        "grep -rn foo",
        "grep foo src/main.rs",
        "rg foo",
        "rg -n 'fn main' crates/",
        "find . -name '*.rs'",
        "find -name '*.rs'",
        "find src -type f",
        "sed -n 10,20p src/main.rs",
        "sed -i 's/a/b/' src/main.rs",
        "sed -e 's/a/b/' src/main.rs",
        "grep -rn foo /proj/src",
        "cargo fmt && grep -rn foo src",
        "cd crates && grep -rn foo .",
        "FOO=1 /usr/bin/grep -rn foo src",
        "grep -rn foo -- src",
    ] {
        let reason = refused(cmd).unwrap_or_else(|| panic!("{cmd} should be routed"));
        assert!(reason.contains("workbench-mcp"), "{reason}");
        assert!(reason.contains("`search`"), "{reason}");
    }
}

#[test]
fn the_refusal_names_the_tool_it_replaces() {
    let reason = refused("sed -n 1,5p src/main.rs").expect("routed");
    assert!(reason.starts_with("`sed`"), "{reason}");
    let reason = refused("find . -name x").expect("routed");
    assert!(reason.starts_with("`find`"), "{reason}");
}

#[test]
fn searches_outside_the_workspace_are_allowed() {
    for cmd in [
        "grep -rn foo /Users/someone/Code/other",
        "find /Users/someone/Code -maxdepth 2 -name speech",
        "sed -n 1,5p /etc/hosts",
        "rg foo ~/Code/other",
        "grep -rn foo ../sibling",
        "cd /Users/someone/Code/other && grep -rn foo src",
        "cd && grep -rn foo .",
        "grep -rn foo src /Users/someone/Code/other",
    ] {
        assert_eq!(refused(cmd), None, "{cmd}");
    }
}

#[test]
fn filtering_piped_output_is_allowed() {
    for cmd in [
        "mdfind -name wordpeel | grep -v node_modules",
        "cargo install --list | grep magistr",
        "ls | sed 's/x/y/'",
        "echo abc | sed -e 's/a/b/'",
        "git log --oneline | grep fix",
    ] {
        assert_eq!(refused(cmd), None, "{cmd}");
    }
}

#[test]
fn separators_inside_quotes_do_not_start_a_command() {
    for cmd in [
        "git commit -m 'fix: grep | sed over src'",
        "git commit -m \"route; find src -name x\"",
        "echo 'a | grep foo src'",
        "printf '%s' 'find . -name x' | cat",
    ] {
        assert_eq!(refused(cmd), None, "{cmd}");
    }
    assert!(
        refused("grep 'a|b' src").is_some(),
        "a quoted pattern still leaves the paths visible"
    );
    assert!(
        refused("grep -rn foo 'my dir'").is_some(),
        "a quoted relative path is inside the workspace"
    );
}

#[test]
fn other_commands_are_not_routed() {
    for cmd in [
        "git grep -n foo",
        "git status",
        "cargo nextest run",
        "echo grep",
        "ls src",
        "cat src/main.rs",
        "",
    ] {
        assert_eq!(refused(cmd), None, "{cmd}");
    }
}
