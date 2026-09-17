use super::*;
use magistr_cockpit::cockpit::{Size, Widget};
use magistr_cockpit::layout::{Length, TextRole};
use magistr_cockpit::render::{Color, Style};
use std::time::Duration;

#[test]
fn screenshot_activity_moves_only_the_header_and_reduced_motion_is_identical() {
    use magistr_cockpit::agent::AgentEvent;
    use magistr_cockpit::cockpit::view;
    use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
    for theme in [ThemeMode::Dark, ThemeMode::Light] {
        for cols in [64, 144] {
            let mut model = Model::new(Size::new(cols, 40)).with_theme(Theme::forced(theme));
            update(
                &mut model,
                Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
                    pointer_shapes: true,
                }),
            );
            update(
                &mut model,
                Msg::Agent(AgentEvent::UserText("Review recovery".into())),
            );
            for _ in 0..24 {
                update(&mut model, Msg::Tick);
            }
            let clock = FrameClock::new(0, Duration::ZERO);
            let before = Preview::frame(&model, &clock, view).unwrap();
            update(&mut model, Msg::Phase(0.5));
            let after = Preview::frame(&model, &clock, view).unwrap();
            let changed: Vec<_> = before
                .pixels()
                .iter()
                .zip(after.pixels())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, _)| i)
                .collect();
            assert!(!changed.is_empty(), "fractional samples move real pixels");
            assert!(
                changed.iter().all(|i| i / (before.width() as usize) < 52),
                "status text and conversation remain still"
            );

            model = model.with_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
            let before = Preview::frame(&model, &clock, view).unwrap();
            for _ in 0..16 {
                update(&mut model, Msg::Tick);
            }
            update(&mut model, Msg::Phase(0.75));
            let after = Preview::frame(&model, &clock, view).unwrap();
            assert_eq!(
                before.data(),
                after.data(),
                "reduced motion is pixel-identical"
            );
        }
    }
}

#[test]
fn captures_use_reported_terminal_pixels_instead_of_default_cells() {
    let mut model = Model::new(Size::new(20, 8));
    update(
        &mut model,
        Msg::Reply(magistr_cockpit::input::Reply::CellPixels {
            width: 13,
            height: 29,
        }),
    );
    let shot = Preview::frame(&model, &FrameClock::new(0, Duration::ZERO), |_, _| {
        Node::column()
            .style(Style::new().bg(Color::Rgb(31, 42, 53)))
            .child(Node::leaf().grow(1.0))
            .child(
                Node::text("Complete bottom edge")
                    .height(Length::Px(29.0))
                    .style(
                        Style::new()
                            .fg(Color::Rgb(250, 250, 250))
                            .bg(Color::Rgb(80, 90, 100)),
                    ),
            )
    })
    .expect("capture");
    assert_eq!((shot.width(), shot.height()), (260, 232));
    assert_eq!(shot.pixel(0, 0).expect("top").alpha(), 255);
    assert_eq!(shot.pixel(259, 231).expect("bottom").alpha(), 255);
    assert_eq!(shot.pixel(259, 231).expect("bottom").red(), 80);
}

#[test]
fn custom_compositions_keep_fractional_pixels_and_use_the_real_graphics_wire() {
    let model = Model::new(Size::new(80, 24));
    let clock = FrameClock::new(0, Duration::ZERO);
    let view = |_: &Model, _: &FrameClock| {
        Node::row()
            .child(Node::leaf().width(Length::Px(13.5)).shrink(0.0))
            .child(
                Node::text("WWW iii")
                    .id(Widget::Prompt.id())
                    .text_role(TextRole::Prose)
                    .width(Length::Px(137.25))
                    .style(Style::new().fg(Color::Rgb(255, 255, 255))),
            )
    };
    let resolved = Preview::layout(&model, &clock, view).expect("pixel layout");
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::Prompt.id()))
        .expect("text rectangle")
        .px;
    assert_eq!(rect.x, 13.5, "layout must not snap to a cell boundary");
    assert_eq!(rect.width, 137.25);
    let (cells, bytes) = Preview::wire(&model, &clock, Protocol::Kitty(Passthrough::Direct), view)
        .expect("native wire");
    assert!(
        cells.cells().iter().all(|c| c.symbol == " "),
        "graphics output must contain no terminal-painted glyphs"
    );
    let placements = kitty::decode(&bytes);
    assert_eq!(
        placements.len(),
        6,
        "six opaque four-row strips cover 24 rows"
    );
    assert!(
        placements
            .iter()
            .all(|p| p.rgba.chunks_exact(4).all(|px| px[3] == 255))
    );
    let capture = Preview::frame(&model, &clock, view).expect("capture");
    let unique: std::collections::HashSet<_> = capture.data().chunks_exact(4).collect();
    assert!(
        unique.len() > 2,
        "the proportional text is antialiased into the actual pixels"
    );
}
