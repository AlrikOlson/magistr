use super::*;

#[test]
fn estimate_turns_from_prompt_short() {
    // "fix the typo" = 3 words → 0..=10 → 1 turn
    assert_eq!(estimate_turns_from_prompt("fix the typo"), 1);
}

#[test]
fn estimate_turns_from_prompt_medium() {
    // 18 words → 11..=30 → 2 turns
    let prompt = "Implement a new authentication system with JWT tokens, refresh token rotation, and rate limiting on the login endpoint";
    assert_eq!(estimate_turns_from_prompt(prompt), 2);
}

#[test]
fn estimate_turns_from_prompt_empty() {
    assert_eq!(estimate_turns_from_prompt(""), 1);
}
