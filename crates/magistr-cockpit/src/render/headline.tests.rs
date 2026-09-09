use super::*;
use crate::render::Color;

fn rect() -> Rect {
    Rect::new(4, 2, 20, 2)
}

fn headline(text: &str) -> Headline {
    Headline::fill(text, rect(), Style::new()).expect("the rect fits a headline")
}

#[test]
fn a_slot_too_narrow_or_too_short_holds_no_headline() {
    assert!(Headline::fill("x", Rect::new(0, 0, 1, 2), Style::new()).is_none());
    assert!(Headline::fill("x", Rect::new(0, 0, 10, 1), Style::new()).is_none());
}

#[test]
fn the_rect_and_the_pen_come_back_out_for_the_screen_to_place_it() {
    let style = Style::new().fg(Color::Indexed(4));
    let headline = Headline::fill("x", rect(), style.clone()).expect("fits");
    assert_eq!(headline.rect(), rect());
    assert_eq!(headline.style(), &style);
}

#[test]
fn two_headlines_of_the_same_text_rect_and_pen_are_equal() {
    // What lets the screen re-emit only on a change.
    assert_eq!(headline("magistr"), headline("magistr"));
    assert_ne!(headline("magistr"), headline("other"));
}

#[test]
fn the_text_is_padded_to_the_cells_the_rect_holds() {
    // Ten columns at scale two hold five characters.
    let fitted = Headline::fill("ab", Rect::new(0, 0, 10, 2), Style::new()).expect("fits");
    assert_eq!(fitted.text(), "ab   ");
}

#[test]
fn a_padded_run_covers_the_same_cells_as_the_one_before_it() {
    // The padding is why replacing a headline needs no erase pass: both
    // runs cover exactly the same cells.
    assert_eq!(headline("magistr").text().len(), headline("m").text().len());
}

#[test]
fn text_too_long_for_the_rect_is_cut_to_it() {
    assert_eq!(headline("a-very-long-project-name").text(), "a-very-lon");
}

#[test]
fn a_wide_grapheme_costs_the_two_cells_it_takes() {
    // Four cells: two wide graphemes and nothing else.
    let fitted = Headline::fill("漢字ab", Rect::new(0, 0, 8, 2), Style::new()).expect("fits");
    assert_eq!(fitted.text(), "漢字");
}
