//! Interactive navigation check: open Retry delivery, Recorded work, then
//! Conversation through the palette; close the window to check retained state.
use magistr_cockpit::cockpit::{Selected, Size, StageTab, Surface};
use magistr_cockpit::harness::Scenes;
use magistr_gui2::Native;

fn main() {
    let model = Scenes::navigation(Size::new(120, 40)).with_surface(Surface::Native);
    let prompt = model.prompt().clone();
    let model = Native::new("Magistr navigation")
        .run(model)
        .expect("native window");
    assert_eq!(model.tab(), StageTab::Turn);
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("retry-work".into()))
    );
    assert_eq!(model.prompt(), &prompt);
    assert!(model.palette().is_none());
    println!("Navigation retained the selected work, draft and attachment.");
}
