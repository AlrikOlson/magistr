use super::TraceFocus;
use crate::cockpit::TraceStep;

fn step(number: u32) -> TraceStep {
    TraceStep {
        number,
        ..TraceStep::default()
    }
}

/// 1 -> 2 -> 3 on the main line, 4 forked off 2, 5 revising 3, and 6
/// standing alone.
fn steps() -> Vec<TraceStep> {
    let mut two = step(2);
    two.dependencies = vec![1];
    let mut three = step(3);
    three.dependencies = vec![2];
    let mut four = step(4);
    four.branch_id = Some("b".into());
    four.branch_from = Some(2);
    let mut five = step(5);
    five.revises = Some(3);
    vec![step(1), two, three, four, five, step(6)]
}

#[test]
fn a_selection_lights_its_ancestors_and_descendants_and_nothing_else() {
    let focus = TraceFocus::of(&steps(), 1);
    assert_eq!(focus.selected, 1);
    assert_eq!(
        focus.lit,
        vec![true, true, true, true, true, false],
        "1 stands under 2; 3, the fork 4 and the revision 5 stand on it; 6 is unrelated"
    );
}

#[test]
fn a_leaf_lights_only_the_chain_beneath_it() {
    let focus = TraceFocus::of(&steps(), 3);
    assert_eq!(focus.lit, vec![true, true, false, true, false, false]);
    assert!(focus.is_lit(3));
    assert!(!focus.is_lit(2));
}

#[test]
fn an_isolated_step_lights_itself_alone_and_an_index_past_the_feed_lights_nothing() {
    let focus = TraceFocus::of(&steps(), 5);
    assert_eq!(focus.lit, vec![false, false, false, false, false, true]);
    let none = TraceFocus::of(&steps(), 9);
    assert!(none.lit.iter().all(|lit| !lit));
    assert!(!none.is_lit(9));
}
