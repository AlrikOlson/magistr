use super::*;

#[test]
fn a_card_is_its_text_and_a_cell_of_padding_either_side() {
    let card = Tooltip {
        text: "rail toggle",
        col: 4,
        row: 2,
    };
    assert_eq!(card.cols(), 13);
}

#[test]
fn a_card_hangs_below_and_right_of_the_pointer() {
    let card = Tooltip {
        text: "Home",
        col: 10,
        row: 3,
    };
    assert_eq!(card.place(80, 24), (11, 4));
}

#[test]
fn a_card_at_the_edge_flips_to_the_other_side_rather_than_hiding_the_pointer() {
    // Clamping would put the card under the pointer, over the very thing
    // it is naming.
    let right = Tooltip {
        text: "Roadmap",
        col: 78,
        row: 3,
    };
    let (col, _) = right.place(80, 24);
    assert!(col + right.cols() <= 80, "on screen");
    assert!(col < 78, "left of the pointer: {col}");

    let bottom = Tooltip {
        text: "quit",
        col: 4,
        row: 23,
    };
    assert_eq!(bottom.place(80, 24).1, 22, "above the pointer");
}

#[test]
fn a_card_wider_than_the_screen_still_starts_on_it() {
    let card = Tooltip {
        text: "a label longer than this whole terminal is wide",
        col: 2,
        row: 1,
    };
    assert_eq!(card.place(20, 10).0, 0);
}
