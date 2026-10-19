use super::*;

#[test]
fn state_is_excluded_but_project_and_workspace_settings_remain_source() {
    for path in [
        ".git/HEAD",
        ".magistr/workspace/journal.db",
        "nested/.magistr/workspace/index.db",
        ".MAGISTR/WORKSPACE/server.lock",
    ] {
        assert!(is_owner_path(Path::new(path)), "{path}");
    }
    for path in [
        ".magistr/project.json",
        ".magistr/workspace.toml",
        ".magistr/workspace-guide.md",
        "src/workspace.rs",
    ] {
        assert!(!is_owner_path(Path::new(path)), "{path}");
    }
}
