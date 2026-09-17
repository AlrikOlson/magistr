use magistr_cockpit::cockpit::{Model, Msg, SettingsGroup, Size, Surface, Widget, update};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};

use super::*;
use crate::comp_root::RootComposition;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn ids(root: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    root.each(&mut |_, element| {
        if let Some(id) = element.id {
            ids.push(id);
        }
    });
    ids
}

fn view() -> Model {
    Fixture::mock("10", Size::new(120, 40)).expect("view 10 has a fixture")
}

/// The fixture is the capture: the Appearance pane over view 05's
/// session, the theme following the OS, a team of four.
#[test]
fn the_fixture_is_the_captures_settings() {
    let model = view();
    let settings = model.settings().expect("⌘, raised it");
    assert_eq!(settings.group(), SettingsGroup::Appearance);
    assert_eq!(settings.query(), "");
    assert_eq!(model.settings_data().theme, ThemeChoice::FollowOs);
    assert_eq!(model.settings_data().team_size, Some(4));
    assert_eq!(model.settings_data().project_path, "~/Code/magistr");
}

#[test]
fn the_surface_rises_with_the_model_and_shows_the_captures_words() {
    let closed = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    assert!(SettingsComposition::el(&closed, Medium::Night).is_none());

    let model = view();
    let surface = SettingsComposition::el(&model, Medium::Night).expect("raised");
    let texts = texts(&surface);
    for word in [
        "settings",
        "esc \u{b7} Close",
        "Search settings",
        "This app",
        "This project",
        "This team \u{b7} 4 people",
        "magistr",
        "~/Code/magistr",
        "read-only",
        "Appearance",
        "Scope: this app, all windows \u{b7} source: user config",
        "Theme",
        "Interface",
        "Show activity heatmap",
        "On the project view",
        "Texture",
        "Film grain, night only",
        "Density",
        "Row height and padding",
        "Comfortable",
        "Compact",
        "source",
        "~/.config/magistr/config.toml",
        "Open file",
    ] {
        assert!(texts.iter().any(|t| t == word), "{word}: {texts:?}");
    }
    for group in SettingsGroup::ALL {
        assert!(
            texts.iter().any(|t| t == group.label()),
            "{}",
            group.label()
        );
    }
    for choice in ThemeChoice::ALL {
        assert!(
            texts.iter().any(|t| t == choice.label()),
            "{}",
            choice.label()
        );
    }
}

/// Every control is a component with an id the root routes to the
/// cockpit's widgets; the pane's controls carry the capture's state.
#[test]
fn ids_route_to_the_cockpits_widgets() {
    let model = view();
    let surface = SettingsComposition::el(&model, Medium::Day).expect("raised");
    let ids = ids(&surface);
    let target = |id| RootComposition::target_of(&model, id);
    assert!(ids.contains(&SettingsComposition::CLOSE));
    assert!(ids.contains(&SettingsComposition::OPEN));
    assert_eq!(
        target(SettingsComposition::CLOSE),
        Some(Widget::SettingsClose.id())
    );
    assert_eq!(
        target(SettingsComposition::OPEN),
        Some(Widget::SettingsOpenFile.id())
    );
    for group in SettingsGroup::ALL {
        let id = ElementId(SettingsComposition::NAV + group.index() as u64);
        assert!(ids.contains(&id), "{}", group.label());
        assert_eq!(target(id), Some(Widget::settings_nav(group.index())));
    }
    for index in 0..ThemeChoice::ALL.len() {
        let id = ElementId(SettingsComposition::THEME + index as u64);
        assert!(ids.contains(&id));
        assert_eq!(target(id), Some(Widget::settings_theme(index)));
    }
    for index in 0..2 {
        let id = ElementId(SettingsComposition::SWITCH + index as u64);
        assert!(ids.contains(&id));
        assert_eq!(target(id), Some(Widget::settings_switch(index)));
    }
    for index in 0..Density::ALL.len() {
        let id = ElementId(SettingsComposition::DENSITY + index as u64);
        assert!(ids.contains(&id));
        assert_eq!(target(id), Some(Widget::settings_density(index)));
    }
}

/// The root gives the surface the whole window: no header path, no
/// rail, no foot, no room behind — and the window's edge.
#[test]
fn the_root_is_the_surface_alone() {
    let model = view();
    let root = RootComposition::el(
        &model,
        Medium::Night,
        (9.0, 20.0),
        (1100.0, 640.0),
        &magistr_cockpit::cockpit::FrameClock::new(0, std::time::Duration::ZERO),
    );
    let texts = texts(&root);
    assert!(texts.iter().any(|t| t == "Appearance"));
    assert!(!texts.iter().any(|t| t == "projects"), "no altitude path");
    assert!(!texts.iter().any(|t| t == "research"), "no foot");
    assert!(
        !texts.iter().any(|t| t == "Agent"),
        "no session cap, no rail"
    );
}

/// Every other group has a pane that repeats its scope and ends with
/// its own source file.
#[test]
fn every_group_has_a_pane_ending_with_its_source() {
    let mut model = view();
    for group in SettingsGroup::ALL {
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::settings_nav(group.index())),
            },
        );
        let surface = SettingsComposition::el(&model, Medium::Night).expect("raised");
        let texts = texts(&surface);
        assert!(
            texts.iter().filter(|t| *t == group.label()).count() >= 2,
            "{}: the nav item and the pane head",
            group.label()
        );
        let sub = format!(
            "Scope: {} \u{b7} source: {}",
            group.scope().reach(),
            group.scope().source_name()
        );
        assert!(texts.contains(&sub), "{sub}");
        assert!(
            texts.iter().any(|t| t == group.source()),
            "{}",
            group.source()
        );
        assert!(texts.iter().any(|t| t == "Open file"));
    }
}

/// Typing narrows the sidebar; a scope with nothing left loses its
/// cap; the open pane stays.
#[test]
fn the_search_narrows_the_sidebar() {
    let mut model = view();
    for c in "bud".chars() {
        update(
            &mut model,
            Msg::Key(magistr_cockpit::input::KeyEvent::new(
                magistr_cockpit::input::KeyCode::Char(c),
            )),
        );
    }
    let surface = SettingsComposition::el(&model, Medium::Night).expect("raised");
    let texts = texts(&surface);
    assert!(
        texts.iter().any(|t| t == "bud"),
        "the query replaces the placeholder"
    );
    assert!(!texts.iter().any(|t| t == "Search settings"));
    assert!(
        !texts.iter().any(|t| t == "This app"),
        "no app group matched"
    );
    assert!(texts.iter().any(|t| t == "This project"));
    assert!(!texts.iter().any(|t| t == "Motion"));
    assert!(texts.iter().any(|t| t == "Budget & limits"));
    assert!(
        texts.iter().any(|t| t == "Appearance"),
        "the pane stays open"
    );
}
