//! Open reasoning, select the previous step and drag the column boundary.
//! Close the window to verify retained draft and the completed interactions.
use magistr_cockpit::cockpit::{Selected, Size, Surface, flight_divider::FlightDivider as D};
use magistr_cockpit::harness::Scenes;
use magistr_gui2::Native;

fn main() {
    let model = Scenes::navigation(Size::new(120, 40)).with_surface(Surface::Native);
    let draft = model.prompt().clone();
    let model = Native::new("Magistr workspace")
        .run(model)
        .expect("native window");
    assert!(model.reasoning_expanded());
    assert_eq!(model.selected(), Some(&Selected::Step(917)));
    assert_ne!(model.panel_fraction(D::Columns, 0.65), 0.65);
    assert!(model.active_panel().is_none());
    assert!(model.palette().is_none());
    assert_eq!(model.prompt(), &draft);
    println!("Reasoning selection, native resize and unchanged draft verified.");
}
