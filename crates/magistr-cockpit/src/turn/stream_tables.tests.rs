use super::*;
use crate::turn::Voice;

#[test]
fn pipes_inside_fenced_code_are_exact_code() {
    let mut turn = Turn::new(60);
    turn.push_text(
        Voice::Agent,
        "```rust\n| Head | Value |\n| --- | --- |\n| exact | bytes |\n```\n",
    );
    assert!(
        turn.rows()
            .iter()
            .any(|row| row.text == "| exact | bytes |")
    );
}

#[test]
fn later_cards_and_prose_keep_their_source_positions_after_table_resize() {
    let mut turn = Turn::new(60);
    turn.push_text(
        Voice::Agent,
        "| Head | Value |\n| --- | --- |\n| A | Good |\n\nLater prose\n",
    );
    turn.push_note("Note");
    let anchor = turn
        .anchor_at(turn.lines().last().unwrap().row_start())
        .unwrap();
    turn.rewrap(12);
    assert_eq!(turn.rows()[turn.row_of(anchor)].text, "Note");
}

#[test]
fn a_table_at_the_start_of_a_reply_keeps_its_speaker() {
    let mut turn = Turn::new(60);
    turn.push_text(
        Voice::Agent,
        "| Head | Value |\n| --- | --- |\n| A | Good |\n",
    );
    turn.set_pixel_wrap(Some(600), crate::pixel::CellSize::new(10, 20));
    assert_eq!(
        turn.conversation().rows().first().unwrap().author,
        Some(Voice::Agent)
    );
}
