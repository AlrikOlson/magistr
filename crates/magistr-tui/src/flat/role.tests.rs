use super::*;

const ALL: [Role; 12] = [
    Role::Session,
    Role::User,
    Role::Agent,
    Role::Thought,
    Role::Tool,
    Role::Gate,
    Role::Edit,
    Role::Image,
    Role::Event,
    Role::Note,
    Role::Error,
    Role::Attention,
];

#[test]
fn every_label_is_one_lowercase_ascii_word() {
    for role in ALL {
        let label = role.label();
        assert!(!label.is_empty(), "{role:?}");
        assert!(
            label.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "{role:?}: {label} is spoken before every line of its role"
        );
    }
}

#[test]
fn no_two_roles_share_a_label() {
    for (i, role) in ALL.iter().enumerate() {
        for other in &ALL[i + 1..] {
            assert_ne!(role.label(), other.label(), "{role:?} vs {other:?}");
        }
    }
}
