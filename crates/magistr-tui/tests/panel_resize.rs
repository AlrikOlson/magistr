//! Resizing through raw terminal reports and the production renderer/hit registry.
use magistr_cockpit::capability::Mode;
use magistr_cockpit::cockpit::{Cmd, FrameClock, Model, Msg, Size, StageTab, Widget, update};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::screenshot::{Profile, layout};
use std::time::Duration;
const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(132, 42);

fn prepared(widget: Widget, cell: (u16, u16)) -> Model {
    let tab = if widget == Widget::PanelGraph {
        StageTab::Roadmap
    } else {
        StageTab::Turn
    };
    let mut m = Model::new(SIZE)
        .with_tab(tab)
        .with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
    update(
        &mut m,
        Msg::Mode(Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    update(
        &mut m,
        Msg::CellSize(Some(magistr_cockpit::pixel::CellSize {
            width: cell.0,
            height: cell.1,
        })),
    );
    if widget == Widget::PanelReasoning {
        update(
            &mut m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::ReasoningToggle.id()),
            },
        );
    }
    update(&mut m, Msg::Paste("Keep my draft".into()));
    m
}

fn extent(m: &Model, widget: Widget) -> f32 {
    let l = m.flight_layout();
    match widget {
        Widget::PanelColumns => l.left_width,
        Widget::PanelGraph => l.graph_height,
        Widget::PanelRoadmap => l.roadmap_height,
        Widget::PanelReasoning => l.conversation_height,
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn panel_drag_uses_actual_cell_and_pixel_hit_targets_and_repaints_before_release() {
    for pixels in [false, true] {
        for cell in [(7, 17), (10, 20), (14, 28), (20, 40)] {
            for widget in [
                Widget::PanelColumns,
                Widget::PanelRoadmap,
                Widget::PanelReasoning,
                Widget::PanelGraph,
            ] {
                let m = prepared(widget, cell);
                let r = layout(&m, &FrameClock::new(0, Duration::ZERO), Profile::Kitty).unwrap();
                let rect = r.px(widget.id()).expect("divider drawn");
                let (x, y) = (
                    (rect.x + rect.width / 2.0) as u32,
                    (rect.y + rect.height / 2.0) as u32,
                );
                let (dx, dy) = if widget == Widget::PanelColumns {
                    (-60, 0)
                } else {
                    (0, -40)
                };
                let to = ((x as i32 + dx) as u32, (y as i32 + dy) as u32);
                let report = |button: u8, x: u32, y: u32, end: char| {
                    let (x, y) = if pixels {
                        (x, y)
                    } else {
                        (x / u32::from(cell.0), y / u32::from(cell.1))
                    };
                    format!("\x1b[<{button};{};{}{end}", x + 1, y + 1)
                };
                let mouse_mode = if pixels { "\x1b[?1016;2$y" } else { "" };
                let startup = format!(
                    "\x1b_Gi=31;OK\x1b\\\x1b[6;{};{}t{mouse_mode}\x1b[?62;4c",
                    cell.1, cell.0
                );
                let terminal = FakeTerminal::new(
                    SIZE,
                    Script::new()
                        .bytes(MS(0), startup)
                        .bytes(MS(200), report(35, x, y, 'M'))
                        .bytes(MS(250), report(0, x, y, 'M'))
                        .bytes(MS(350), report(32, to.0, to.1, 'M'))
                        .bytes(MS(600), report(0, to.0, to.1, 'm'))
                        .close(MS(750)),
                );
                let capture = terminal.capture();
                let deliveries = terminal.deliveries();
                let mut runtime = Runtime::new(
                    terminal,
                    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(100)),
                );
                let mut commands = runtime.commands().unwrap();
                let before = extent(&m, widget);
                let out = runtime.run(m).await.unwrap();
                assert!(
                    (extent(&out, widget) - before).abs() > 20.0,
                    "{widget:?} pixel={pixels} cell={cell:?}"
                );
                assert_eq!(out.prompt().text(), "Keep my draft");
                while let Some(command) = commands.try_next() {
                    assert!(!matches!(command, Cmd::Prompt(_) | Cmd::RoadmapPrompt(_)));
                }
                let inputs = deliveries.instants();
                let (drag, release) = (inputs[3], inputs[4]);
                assert!(
                    capture
                        .writes()
                        .iter()
                        .any(|(at, len)| *at >= drag && *at < release && *len > 1000),
                    "live paint before mouse release"
                );
                let pointer: &[u8] = if widget == Widget::PanelColumns {
                    b"\x1b]22;ew-resize\x1b\\"
                } else {
                    b"\x1b]22;ns-resize\x1b\\"
                };
                assert!(capture.count(pointer) > 0, "resize pointer was emitted");
                assert_eq!(capture.count(b"\x1b[?1016h"), usize::from(pixels));
            }
        }
    }
}

#[test]
fn shot_panel_resize_default_dragging_and_compact_in_both_themes() {
    use magistr_cockpit::golden::Golden;
    use magistr_cockpit::harness::Scenes;
    use magistr_tui::screenshot::frame;
    let golden =
        Golden::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/screens"));
    let mut failures = Vec::new();
    for (name, theme) in [("mocha", ThemeMode::Dark), ("latte", ThemeMode::Light)] {
        let mut m = Scenes::roadmapped(SIZE)
            .with_tab(StageTab::Roadmap)
            .with_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
        update(
            &mut m,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        let clock = FrameClock::new(0, Duration::ZERO);
        for stage in ["default", "dragging", "keyboard", "compact"] {
            if stage == "dragging" {
                let r = layout(&m, &clock, Profile::Kitty).unwrap();
                let rect = r.px(Widget::PanelGraph.id()).unwrap();
                let (x, y) = (
                    (rect.x + rect.width / 2.0) as u32,
                    (rect.y + rect.height / 2.0) as u32,
                );
                update(
                    &mut m,
                    Msg::Mouse {
                        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x, y),
                        target: Some(Widget::PanelGraph.id()),
                    },
                );
                update(
                    &mut m,
                    Msg::Mouse {
                        event: MouseEvent::new(MouseKind::Drag(Button::Left), 0, 0)
                            .at_px(x, y + 60),
                        target: None,
                    },
                );
            } else if stage == "keyboard" {
                use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};
                update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Esc)));
                update(
                    &mut m,
                    Msg::Key(KeyEvent::new(KeyCode::Char('p')).with(Modifiers::CTRL)),
                );
                for c in "Resize panels".chars() {
                    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
                }
                update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Enter)));
            } else if stage == "compact" {
                // Finish keyboard adjustment before showing the compact window.
                update(
                    &mut m,
                    Msg::Key(magistr_cockpit::input::KeyEvent::new(
                        magistr_cockpit::input::KeyCode::Esc,
                    )),
                );
                update(&mut m, Msg::Resize(Size::new(64, 24)));
            }
            let image = frame(&m, &clock, Profile::Kitty).unwrap();
            if let Err(error) = golden.check(&format!("panel-resize-{name}-{stage}"), &image) {
                failures.push(error.to_string());
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
