use super::*;

/// The seven words, each its own, each parsing back to itself.
#[test]
fn the_seven_words_round_trip() {
    let words: Vec<&str> = StateWord::ALL.iter().map(|s| s.word()).collect();
    assert_eq!(
        words,
        [
            "Ready",
            "Executing",
            "Review",
            "Stuck",
            "Blocked",
            "Done",
            "Idle"
        ]
    );
    for state in StateWord::ALL {
        assert_eq!(StateWord::parse(state.word()), Some(state));
    }
    assert_eq!(StateWord::parse("Pending"), None);
}

/// Every star state has a word, and the two the sky never labels fold
/// onto their text equivalents: a pending star cannot start, a ghost is
/// nothing happening.
#[test]
fn every_star_state_has_one_of_the_seven_words() {
    for state in StarState::ALL {
        let word = StateWord::of_star(state).word();
        assert!(
            StateWord::parse(word).is_some(),
            "{state:?} says {word}, which is not one of the seven"
        );
    }
    assert_eq!(StateWord::of_star(StarState::Pending), StateWord::Blocked);
    assert_eq!(StateWord::of_star(StarState::Ghost), StateWord::Idle);
    assert_eq!(StateWord::of_star(StarState::Done), StateWord::Done);
}

/// A wire status reads through the star's own mapping; readiness
/// decides between Ready and Blocked; an obsoleted chunk is Idle.
#[test]
fn a_wire_status_reads_as_one_word() {
    assert_eq!(
        StateWord::of_status("in_progress", false),
        StateWord::Executing
    );
    assert_eq!(StateWord::of_status("pending", true), StateWord::Ready);
    assert_eq!(StateWord::of_status("pending", false), StateWord::Blocked);
    assert_eq!(StateWord::of_status("backlog", false), StateWord::Blocked);
    assert_eq!(StateWord::of_status("blocked", true), StateWord::Blocked);
    assert_eq!(StateWord::of_status("stuck", true), StateWord::Stuck);
    assert_eq!(StateWord::of_status("done", true), StateWord::Done);
    assert_eq!(StateWord::of_status("obsoleted", true), StateWord::Idle);
}

/// No impostor is one of the seven — the negative space never overlaps
/// the vocabulary.
#[test]
fn no_impostor_is_a_state_word() {
    for impostor in StateWord::IMPOSTORS {
        assert_eq!(StateWord::parse(impostor), None, "{impostor}");
    }
}
