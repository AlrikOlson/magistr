use super::*;

/// The first raise rises from the edge over the glide's steps and lands
/// on the rows asked for; the height is whole rows on the way.
#[test]
fn a_raise_rises_from_the_edge_to_its_rows() {
    let mut dock = TermDock::new();
    assert!(!dock.is_up());
    assert_eq!(dock.height_at(0.0), 0);
    assert_eq!(dock.taken(0.0), 0);
    assert_eq!(dock.term(0.0), None);
    dock.raise("term-1", 12, 10.0, true);
    assert!(dock.is_up());
    assert_eq!(dock.id(), Some("term-1"));
    assert_eq!(dock.height_at(10.0), 0, "the first frame is the edge");
    assert!(dock.is_settling(10.0));
    let mid = dock.height_at(10.0 + Eased::STEPS as f32 / 2.0);
    assert!(mid > 0 && mid < 12, "{mid}");
    assert_eq!(dock.height_at(10.0 + Eased::STEPS as f32), 12);
    assert_eq!(
        dock.taken(10.0 + Eased::STEPS as f32),
        13,
        "the grip row too"
    );
    assert!(!dock.is_settling(10.0 + Eased::STEPS as f32));
    assert_eq!(dock.term(11.0), Some("term-1"));
}

/// Under reduced motion the raise lands at once.
#[test]
fn without_animation_the_dock_lands_at_once() {
    let mut dock = TermDock::new();
    dock.raise("t", 10, 3.0, false);
    assert_eq!(dock.height_at(3.0), 10);
    assert!(!dock.is_settling(3.0));
    assert!(dock.lower(4.0, false));
    assert_eq!(dock.height_at(4.0), 0);
    assert_eq!(dock.term(4.0), None);
}

/// Lowering glides back down with the terminal kept for the glide, and
/// a second lower is nothing.
#[test]
fn lowering_glides_down_and_keeps_the_screen_for_the_glide() {
    let mut dock = TermDock::new();
    dock.raise("t", 10, 0.0, false);
    assert!(dock.lower(5.0, true));
    assert!(!dock.is_up());
    assert!(dock.height_at(5.0) > 0);
    assert_eq!(dock.term(5.0), Some("t"), "still drawn while it lowers");
    assert_eq!(dock.height_at(5.0 + Eased::STEPS as f32), 0);
    assert_eq!(dock.term(5.0 + Eased::STEPS as f32), None);
    assert!(!dock.lower(20.0, true));
}

/// The rows are clamped to a card's pane and the screen.
#[test]
fn the_rows_are_held_between_a_card_and_the_screen() {
    let mut dock = TermDock::new();
    dock.raise("t", 2, 0.0, false);
    assert_eq!(dock.rows(), TermDock::MIN_ROWS);
    dock.raise("t", 200, 0.0, false);
    assert_eq!(dock.rows(), TermDock::MAX_ROWS);
    assert_eq!(TermDock::clamp(9), 9);
}

/// Maximize takes the stage's rows and keeps the dragged height for the
/// way back; restoring returns to it; nothing while down.
#[test]
fn maximize_keeps_the_dragged_height_for_the_way_back() {
    let mut dock = TermDock::new();
    dock.maximize(20, 0.0, false);
    assert!(!dock.is_up() && !dock.is_maximized(), "nothing while down");
    dock.raise("t", 10, 0.0, false);
    dock.maximize(20, 1.0, false);
    assert!(dock.is_maximized());
    assert_eq!(dock.target_rows(), 20);
    assert_eq!(dock.rows(), 10, "the dragged height is kept");
    assert_eq!(dock.height_at(1.0), 20);
    dock.restore();
    assert!(!dock.is_maximized());
    assert_eq!(dock.target_rows(), 10);
    // Maximized past the screen is the screen.
    dock.maximize(99, 2.0, false);
    assert_eq!(dock.target_rows(), TermDock::MAX_ROWS);
}

/// A grip drag: the press remembers where it began and the rows it had;
/// dragging up adds rows, down takes them, clamped; the release hands
/// back the height to remember. A drag un-maximizes.
#[test]
fn a_grip_drag_resizes_by_the_rows_the_pointer_moved() {
    let mut dock = TermDock::new();
    dock.begin_drag(30.0);
    assert!(!dock.is_dragging(), "nothing to drag while down");
    dock.raise("t", 10, 0.0, false);
    dock.maximize(20, 0.0, false);
    dock.begin_drag(30.5);
    assert!(dock.is_dragging());
    assert_eq!(
        dock.drag_to(30.5),
        Some(20),
        "not moved, but the maximized twenty rows are the reader's own now"
    );
    assert!(!dock.is_maximized(), "a drag makes the height the reader's");
    assert_eq!(dock.rows(), 20);
    assert_eq!(dock.drag_to(30.5), None, "still not moved");
    assert_eq!(dock.drag_to(27.5), Some(23));
    assert_eq!(dock.height_at(0.0), 23, "the dock follows the grip at once");
    assert_eq!(dock.drag_to(0.0), Some(TermDock::MAX_ROWS));
    assert_eq!(dock.drag_to(50.0), Some(TermDock::MIN_ROWS));
    assert_eq!(dock.drag_to(35.5), Some(15));
    assert_eq!(dock.end_drag(), Some(15));
    assert!(!dock.is_dragging());
    assert_eq!(dock.end_drag(), None, "no drag to end");
    assert_eq!(dock.rows(), 15);
}

/// A dock is down by default and equal to a fresh one.
#[test]
fn the_default_dock_is_down() {
    assert_eq!(TermDock::default(), TermDock::new());
    assert_eq!(TermDock::MIN_ROWS, 8);
    assert_eq!(TermDock::MAX_ROWS, 24);
}
