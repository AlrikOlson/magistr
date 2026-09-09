use super::*;

#[test]
fn the_sidebar_order_groups_by_scope_app_then_project_then_team() {
    let scopes: Vec<SettingsScope> = SettingsGroup::ALL.iter().map(|g| g.scope()).collect();
    let mut sorted = scopes.clone();
    sorted.sort_by_key(|scope| SettingsScope::ALL.iter().position(|s| s == scope));
    assert_eq!(scopes, sorted, "scopes never interleave");
    assert_eq!(
        scopes.iter().filter(|s| **s == SettingsScope::App).count(),
        4
    );
    assert_eq!(
        scopes
            .iter()
            .filter(|s| **s == SettingsScope::Project)
            .count(),
        4
    );
    assert_eq!(
        scopes.iter().filter(|s| **s == SettingsScope::Team).count(),
        2
    );
}

#[test]
fn every_group_names_its_source_file() {
    assert_eq!(
        SettingsGroup::Appearance.source(),
        "~/.config/magistr/config.toml"
    );
    assert_eq!(SettingsGroup::Gates.source(), ".magistr/project.json");
    assert_eq!(SettingsGroup::Policy.source(), ".magistr/team.json");
    for group in SettingsGroup::ALL {
        assert_eq!(group.source(), group.scope().source());
    }
}

#[test]
fn only_the_policy_is_read_only_and_indices_round_trip() {
    let read_only: Vec<SettingsGroup> = SettingsGroup::ALL
        .into_iter()
        .filter(|g| g.is_read_only())
        .collect();
    assert_eq!(read_only, [SettingsGroup::Policy]);
    for (index, group) in SettingsGroup::ALL.into_iter().enumerate() {
        assert_eq!(group.index(), index);
        assert_eq!(SettingsGroup::at(index), Some(group));
    }
    assert_eq!(SettingsGroup::at(10), None);
}

#[test]
fn the_scopes_carry_the_pane_sub_line_words() {
    assert_eq!(SettingsScope::App.reach(), "this app, all windows");
    assert_eq!(SettingsScope::App.source_name(), "user config");
    assert_eq!(SettingsScope::Team.label(), "This team");
}
