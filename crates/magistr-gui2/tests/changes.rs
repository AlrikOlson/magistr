//! Real frame hit, accessibility, journal selection and refusal handling.
use magistr_cockpit::cockpit::{
    Cmd, FrameClock, Model, Msg, Place, Selected, Size, Surface, Widget, WorkbenchCmd,
    WorkbenchEvent, update,
};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_cockpit::widget_id::WidgetId;
use magistr_cockpit::workbench::JournalEntry;
use magistr_gui2::{Frame, comp_workspace::WorkspaceComposition as W};

fn press(frame: &mut Frame, model: &mut Model, target: WidgetId) -> Vec<Cmd> {
    let [x, y, w, h] = frame.rect_of(W::id(target)).expect("placed control");
    let hit = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    assert_eq!(hit, Some(target));
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: hit,
        },
    )
}

#[test]
fn keyboard_reveals_late_journal_entries_in_compact_and_wide_windows() {
    for (width, height) in [(1100, 760), (620, 640)] {
        let mut frame = Frame::new().unwrap();
        let mut model = Model::new(Size::new(120, 40))
            .with_surface(Surface::Native)
            .with_place(Place::Changes)
            .with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
        let entries: Vec<_> = (0..50)
            .map(|index| JournalEntry {
                id: index.to_string(),
                path: format!("file-{index}.txt"),
                ..JournalEntry::default()
            })
            .collect();
        let last = entries.last().unwrap().identity();
        update(
            &mut model,
            Msg::Workbench(WorkbenchEvent::Journal { entries }),
        );
        let clock = FrameClock::new(0, std::time::Duration::ZERO);
        frame.still(&mut model, &clock, width, height).unwrap();
        for _ in 0..50 {
            for message in frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Down))) {
                let commands = update(&mut model, message);
                assert!(
                    commands
                        .iter()
                        .all(|command| matches!(command, Cmd::Announce(_) | Cmd::Animate(_))),
                    "{commands:?}"
                );
            }
            frame.still(&mut model, &clock, width, height).unwrap();
        }
        assert_eq!(model.selected(), Some(&Selected::Change(last)));
        assert!(model.session_top() > 0);
        assert!(
            frame
                .access()
                .nodes()
                .iter()
                .any(|node| node.target == Some(Widget::change_row(49)))
        );
        press(&mut frame, &mut model, Widget::change_row(49));
    }
}

#[test]
fn source_changes_are_readable_actionable_and_keep_failed_undo_visible() {
    for (name, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for (width, height) in [(1100, 760), (620, 640)] {
            let mut frame = Frame::new().unwrap();
            let mut model = Model::new(Size::new(120, 40))
                .with_surface(Surface::Native)
                .with_place(Place::Changes)
                .with_theme(Theme::forced(mode).with_motion(MotionMode::Reduced));
            let entry = JournalEntry {
                id: "17".into(),
                path: "sample.txt".into(),
                added: 1,
                removed: 1,
                parse_ok: true,
                diff: "--- sample.txt\n+++ sample.txt\n-before\n+after".into(),
                ..JournalEntry::default()
            };
            update(
                &mut model,
                Msg::Workbench(WorkbenchEvent::Journal {
                    entries: vec![entry.clone()],
                }),
            );
            let clock = FrameClock::new(0, std::time::Duration::ZERO);
            frame.still(&mut model, &clock, width, height).unwrap();
            press(&mut frame, &mut model, Widget::change_row(0));
            let image = frame.still(&mut model, &clock, width, height).unwrap();
            // Measure the actual painted selected label against its row background.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let [x, y, w, h] = frame
                .rect_of(W::id(Widget::change_row(0)))
                .unwrap()
                .map(|value| value as u32);
            let color = |pixel: tiny_skia::PremultipliedColorU8| {
                magistr_theme::ThemeColor::new(pixel.red(), pixel.green(), pixel.blue())
            };
            let background = color(image.pixel(x + w - 1, y + h / 2).unwrap());
            let contrast = (y..y + h)
                .flat_map(|y| (x..x + w).map(move |x| (x, y)))
                .map(|(x, y)| {
                    magistr_theme::math::contrast_ratio(
                        color(image.pixel(x, y).unwrap()),
                        background,
                    )
                })
                .fold(1.0, f64::max);
            assert!(contrast >= 4.5, "selected label contrast is {contrast}");
            assert_eq!(model.selected(), Some(&Selected::Change(entry.identity())));
            assert!(
                frame
                    .access()
                    .find(Widget::Inspector)
                    .unwrap()
                    .value
                    .as_deref()
                    .unwrap()
                    .contains("+after")
            );
            assert_eq!(
                press(&mut frame, &mut model, Widget::change_undo(0)),
                vec![Cmd::Workbench(WorkbenchCmd::Undo { id: "17".into() })]
            );
            update(
                &mut model,
                Msg::Workbench(WorkbenchEvent::Failed {
                    cmd: WorkbenchCmd::Undo { id: "17".into() },
                    error: "The file has changed again.".into(),
                }),
            );
            frame.still(&mut model, &clock, width, height).unwrap();
            assert!(
                frame
                    .access()
                    .find(Widget::Inspector)
                    .unwrap()
                    .value
                    .as_deref()
                    .unwrap()
                    .contains("The file has changed again.")
            );
            assert_eq!(model.changes().entries().len(), 1);
            image
                .save_png(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(format!("target/changes-{width}-{name}.png")),
                )
                .unwrap();
        }
    }
}
