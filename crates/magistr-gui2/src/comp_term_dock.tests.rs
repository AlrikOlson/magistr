use super::*;

use magistr_cockpit::cockpit::{Eased, Msg, Size, TermDock, update};
use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};

use crate::element::Kind;
use crate::fixture::Fixture;

const SIZE: Size = Size::new(120, 40);

/// Every text run in `element`, in tree order.
fn texts(element: &Element) -> Vec<String> {
    element
        .runs()
        .into_iter()
        .map(|run| run.text.to_owned())
        .collect()
}

/// The grid's row count under `element`, when it holds one.
fn grid_rows(element: &Element) -> Option<usize> {
    let mut found = None;
    element.each(&mut |_, el| {
        if let Kind::Grid { rows, .. } = &el.kind {
            found = Some(rows.len());
        }
    });
    found
}

fn cmd_enter() -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER))
}

/// Down, the dock is nothing; raised, it is the grip — the command's
/// cap, the chords — over the terminal's window at the dock's rows,
/// with the ids the root resolves to the cockpit's widgets.
#[test]
fn the_dock_is_the_grip_over_the_windows_rows_and_nothing_while_down() {
    let down = Fixture::pane_following(SIZE);
    assert!(TermDockComposition::el(&down, Medium::Night).is_none());
    let docked = Fixture::pane_docked(SIZE);
    let rows = usize::from(docked.term_dock().rows());
    assert!(
        rows > TermDock::MIN_ROWS as usize,
        "the third of a 40-row stage"
    );
    for medium in Medium::ALL {
        let dock = TermDockComposition::el(&docked, medium).expect("raised");
        let texts = texts(&dock);
        assert_eq!(
            texts[0], "cargo nextest run -p magistr-tui",
            "the command's cap"
        );
        assert_eq!(texts[1], TermDockComposition::hint());
        assert!(texts[1].starts_with(Binding::DOCK_MAX_CHORD.keys));
        assert_eq!(grid_rows(&dock), Some(rows), "the dock's rows, all of them");
        let ids: Vec<ElementId> = {
            let mut ids = Vec::new();
            dock.each(&mut |_, el| {
                if let Some(id) = el.id {
                    ids.push(id);
                }
            });
            ids
        };
        assert_eq!(ids, [TermDockComposition::GRIP, TermDockComposition::PANE]);
        let owners: Vec<_> = dock.runs().into_iter().map(|r| r.owner).collect();
        assert_eq!(owners, [Some(Component::Cap), Some(Component::KeyHint)]);
    }
}

/// While the dock rises the composition is the same mapping at fewer
/// rows — the window's tail first — and once maximized it is the
/// stage's rows.
#[test]
fn a_rising_dock_shows_the_tail_at_the_rows_the_rise_has_reached() {
    let mut model = Fixture::pane_following(SIZE);
    update(&mut model, cmd_enter());
    assert!(
        TermDockComposition::el(&model, Medium::Night).is_none(),
        "the first frame is the edge"
    );
    for _ in 0..(Eased::STEPS / 2) {
        update(&mut model, Msg::Tick);
    }
    let rising = TermDockComposition::el(&model, Medium::Night).expect("rising");
    let shown = grid_rows(&rising).expect("a grid");
    let target = usize::from(model.term_dock().rows());
    assert!(shown > 0 && shown < target, "{shown} of {target}");
    // The last row shown is the window's last: the run's progress bar.
    let last = texts(&rising);
    assert_eq!(last.len(), 2, "no pill: the dock follows");
    let maximized = Fixture::pane_maximized(SIZE);
    let dock = TermDockComposition::el(&maximized, Medium::Day).expect("maximized");
    assert_eq!(
        grid_rows(&dock),
        Some(usize::from(maximized.dock_max_rows()))
    );
    assert!(maximized.term_dock().is_maximized());
}

/// On the whole frame the feed gives up the rows the dock takes: the
/// stage's box ends where the grip begins, the composer stands under
/// the dock, and the rail's box is the one it had before the rise.
#[test]
fn the_feed_ends_where_the_grip_begins_and_the_composer_stands_under_it() {
    use crate::comp_composer::ComposerComposition;
    use crate::comp_root::RootComposition;
    use crate::frame::Frame;
    use magistr_cockpit::cockpit::FrameClock;
    let (width, height) = Fixture::CAPTURE;
    let clock = FrameClock::new(0, std::time::Duration::ZERO);
    let mut frame = Frame::new().expect("a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut down = Fixture::with_pixels(Fixture::pane_following(size));
    let _ = frame
        .build(&mut down, &clock, width, height)
        .expect("builds");
    let divider_before = frame.rect_of(RootComposition::DIVIDER).expect("divider");
    let composer_before = frame.rect_of(ComposerComposition::PILL).expect("composer");
    let mut docked = Fixture::with_pixels(Fixture::pane_docked(size));
    let _ = frame
        .build(&mut docked, &clock, width, height)
        .expect("builds");
    let [_, stage_y, _, stage_h] = frame.rect_of(RootComposition::STAGE).expect("stage");
    let [_, grip_y, _, grip_h] = frame.rect_of(TermDockComposition::GRIP).expect("grip");
    let [_, pane_y, _, pane_h] = frame.rect_of(TermDockComposition::PANE).expect("pane");
    let composer = frame.rect_of(ComposerComposition::PILL).expect("composer");
    assert!(
        (stage_y + stage_h - grip_y).abs() < 0.5,
        "the feed ends at the grip: stage {stage_y}+{stage_h} vs grip {grip_y}"
    );
    assert!(
        (grip_y + grip_h - pane_y).abs() < 0.5,
        "the pane follows the grip"
    );
    assert!(
        pane_y + pane_h <= composer[1],
        "the composer stands under the dock"
    );
    assert_eq!(composer, composer_before, "the composer never moved");
    assert_eq!(
        frame.rect_of(RootComposition::DIVIDER).expect("divider"),
        divider_before,
        "the rail stays"
    );
}
