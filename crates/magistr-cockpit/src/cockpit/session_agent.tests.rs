use super::*;

#[test]
fn the_two_lines_are_the_volumes_own() {
    let agent = SessionAgent::new("sonnet", "Draft mode").spent(61, 12, 2);
    assert_eq!(agent.identity(), "sonnet · Draft mode");
    assert_eq!(agent.ledger(), "0.61 USD · 12 min · 2 requests");
}

#[test]
fn a_dollar_is_two_places_however_round_it_is() {
    let agent = SessionAgent::new("opus", "Auto").spent(1200, 90, 41);
    assert_eq!(agent.ledger(), "12.00 USD · 90 min · 41 requests");
    let agent = SessionAgent::new("opus", "Auto").spent(5, 1, 1);
    assert_eq!(agent.ledger(), "0.05 USD · 1 min · 1 request");
}

#[test]
fn an_agent_that_has_spent_nothing_still_names_itself() {
    let agent = SessionAgent::new("haiku", "Draft mode");
    assert_eq!(agent.identity(), "haiku · Draft mode");
    assert_eq!(agent.ledger(), "0.00 USD · 0 min · 0 requests");
}

#[test]
fn an_agent_bound_under_no_mode_word_names_the_model_alone() {
    let agent = SessionAgent::new("sonnet", "");
    assert_eq!(agent.identity(), "sonnet");
}
