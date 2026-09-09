use super::*;

fn mapping() -> Vec<GroupRole> {
    vec![
        GroupRole {
            group: "magistr-owners".into(),
            role: TeamRole::Owner,
        },
        GroupRole {
            group: "eng-*".into(),
            role: TeamRole::Developer,
        },
        GroupRole {
            group: "*-readonly".into(),
            role: TeamRole::Viewer,
        },
    ]
}

fn groups(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_string()).collect()
}

#[test]
fn an_exact_row_matches_its_group() {
    assert_eq!(
        GroupRole::highest(&mapping(), &groups(&["magistr-owners"])),
        Some(TeamRole::Owner)
    );
}

#[test]
fn a_glob_row_covers_a_family_of_groups() {
    assert_eq!(
        GroupRole::highest(&mapping(), &groups(&["eng-platform"])),
        Some(TeamRole::Developer)
    );
}

#[test]
fn a_group_no_row_names_earns_nothing() {
    assert_eq!(GroupRole::highest(&mapping(), &groups(&["finance"])), None);
}

#[test]
fn no_groups_at_all_earns_nothing() {
    assert_eq!(GroupRole::highest(&mapping(), &[]), None);
}

#[test]
fn the_most_privileged_match_wins() {
    let both = groups(&["eng-platform", "magistr-owners"]);
    assert_eq!(
        GroupRole::highest(&mapping(), &both),
        Some(TeamRole::Owner),
        "a person in two mapped groups holds the more privileged role"
    );
}

/// The reason the fold is an extremum rather than a first-match: rule
/// order is a thing a human gets wrong, and an extremum cannot see it.
#[test]
fn the_order_the_rows_are_written_in_cannot_change_the_answer() {
    let mut reversed = mapping();
    reversed.reverse();
    let both = groups(&["eng-platform", "magistr-owners"]);
    assert_eq!(
        GroupRole::highest(&mapping(), &both),
        GroupRole::highest(&reversed, &both)
    );
}

/// Ci is declared after Viewer but runs sessions and edits the roadmap,
/// so declaration order is not the ranking.
#[test]
fn ci_outranks_viewer_though_it_is_declared_after_it() {
    let mapping = vec![
        GroupRole {
            group: "bots".into(),
            role: TeamRole::Ci,
        },
        GroupRole {
            group: "everyone".into(),
            role: TeamRole::Viewer,
        },
    ];
    assert_eq!(
        GroupRole::highest(&mapping, &groups(&["everyone", "bots"])),
        Some(TeamRole::Ci)
    );
}

#[test]
fn matching_is_case_insensitive_the_way_directory_names_are() {
    assert_eq!(
        GroupRole::highest(&mapping(), &groups(&["ENG-Platform"])),
        Some(TeamRole::Developer)
    );
}

#[test]
fn a_row_round_trips_through_json() {
    let row = GroupRole {
        group: "eng-*".into(),
        role: TeamRole::Developer,
    };
    let text = serde_json::to_string(&row).expect("serialize");
    assert_eq!(text, r#"{"group":"eng-*","role":"developer"}"#);
    assert_eq!(
        serde_json::from_str::<GroupRole>(&text).expect("deserialize"),
        row
    );
}
