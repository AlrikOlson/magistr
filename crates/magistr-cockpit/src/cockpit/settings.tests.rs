use super::*;
use crate::cockpit::{
    Cmd, Density, Model, Msg, SettingsData, Size, Surface, ThemeChoice, Widget, WorkbenchCmd,
    update,
};
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::theme::{MotionMode, Theme, ThemeMode};

fn native() -> Model {
    Model::new(Size::new(120, 40)).with_surface(Surface::Native)
}

fn key(model: &mut Model, code: KeyCode, mods: Modifiers) -> Vec<Cmd> {
    update(model, Msg::Key(KeyEvent::new(code).with(mods)))
}

fn chord(model: &mut Model) {
    key(model, KeyCode::Char(','), Modifiers::SUPER);
}

fn typed(model: &mut Model, text: &str) {
    for c in text.chars() {
        key(model, KeyCode::Char(c), Modifiers::NONE);
    }
}

fn click(model: &mut Model, target: Option<crate::widget_id::WidgetId>) -> Vec<Cmd> {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target,
        },
    )
}

#[test]
fn the_chord_raises_settings_on_a_native_surface_only_and_toggles() {
    let mut terminal = Model::new(Size::new(120, 40));
    chord(&mut terminal);
    assert!(terminal.settings().is_none(), "a terminal never sees SUPER");

    let mut model = native();
    chord(&mut model);
    let settings = model.settings().expect("raised");
    assert_eq!(settings.group(), SettingsGroup::Appearance);
    assert_eq!(settings.query(), "");
    chord(&mut model);
    assert!(model.settings().is_none(), "the chord toggles");
    chord(&mut model);
    key(&mut model, KeyCode::Esc, Modifiers::NONE);
    assert!(model.settings().is_none(), "Esc closes");
}

#[test]
fn the_surface_owns_the_keyboard_while_up() {
    let mut model = native();
    chord(&mut model);
    // ⌘K and ⌘⇧F are swallowed: one float at a time.
    key(&mut model, KeyCode::Char('k'), Modifiers::SUPER);
    assert!(model.palette().is_none());
    key(
        &mut model,
        KeyCode::Char('F'),
        Modifiers::SUPER | Modifiers::SHIFT,
    );
    assert!(model.search().is_none());
    assert!(model.settings().is_some());
    // Typing never reaches the prompt.
    typed(&mut model, "ga");
    assert_eq!(model.prompt().text(), "");
}

#[test]
fn typing_filters_the_sidebar_and_backspace_widens_it() {
    let mut settings = Settings::default();
    assert_eq!(settings.visible(), SettingsGroup::ALL.to_vec());
    for c in "GA".chars() {
        settings.type_char(c);
    }
    assert_eq!(settings.visible(), [SettingsGroup::Gates]);
    settings.backspace();
    assert_eq!(
        settings.visible(),
        [
            SettingsGroup::Gates,
            SettingsGroup::Budget,
            SettingsGroup::Backend
        ]
    );
    // The open pane stays open even when the filter hides its item.
    assert_eq!(settings.group(), SettingsGroup::Appearance);
    for c in "xyz".chars() {
        settings.type_char(c);
    }
    assert!(settings.visible().is_empty());
    assert_eq!(settings.group(), SettingsGroup::Appearance);
}

#[test]
fn the_arrows_walk_the_visible_groups_and_hold_at_the_ends() {
    let mut model = native();
    chord(&mut model);
    key(&mut model, KeyCode::Up, Modifiers::NONE);
    assert_eq!(model.settings().unwrap().group(), SettingsGroup::Appearance);
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    assert_eq!(model.settings().unwrap().group(), SettingsGroup::Motion);
    for _ in 0..20 {
        key(&mut model, KeyCode::Down, Modifiers::NONE);
    }
    assert_eq!(model.settings().unwrap().group(), SettingsGroup::Policy);
    // Filtered: ↓ walks the filtered list; a hidden open group snaps
    // to the first visible one.
    typed(&mut model, "e");
    key(&mut model, KeyCode::Up, Modifiers::NONE);
    let visible = model.settings().unwrap().visible();
    assert!(visible.contains(&SettingsGroup::Members));
    assert!(!visible.contains(&SettingsGroup::Policy));
    assert_eq!(model.settings().unwrap().group(), visible[0]);
}

#[test]
fn a_nav_click_opens_its_pane_and_a_stray_click_holds() {
    let mut model = native();
    chord(&mut model);
    click(
        &mut model,
        Some(Widget::settings_nav(SettingsGroup::Charter.index())),
    );
    assert_eq!(model.settings().unwrap().group(), SettingsGroup::Charter);
    click(&mut model, None);
    assert!(model.settings().is_some(), "no stray close");
    click(&mut model, Some(Widget::Composer.id()));
    assert!(
        model.settings().is_some(),
        "the room behind is not clickable"
    );
    click(&mut model, Some(Widget::SettingsClose.id()));
    assert!(model.settings().is_none());
}

#[test]
fn a_theme_card_forces_the_models_theme_and_follow_os_releases_it() {
    let mut model = native().with_theme(Theme::detect(None, None).with_motion(MotionMode::Reduced));
    chord(&mut model);
    assert_eq!(model.settings_data().theme, ThemeChoice::FollowOs);
    click(&mut model, Some(Widget::settings_theme(1)));
    assert_eq!(model.settings_data().theme, ThemeChoice::Chart);
    assert!(model.theme().is_forced());
    assert_eq!(model.theme().mode(), ThemeMode::Light);
    assert_eq!(
        model.theme().motion(),
        MotionMode::Reduced,
        "the reader's motion choice survives"
    );
    click(&mut model, Some(Widget::settings_theme(0)));
    assert_eq!(model.theme().mode(), ThemeMode::Dark);
    assert!(model.theme().is_forced());
    click(&mut model, Some(Widget::settings_theme(2)));
    assert_eq!(model.settings_data().theme, ThemeChoice::FollowOs);
    assert!(!model.theme().is_forced());
    assert!(model.settings().is_some(), "a choice keeps the surface");
}

#[test]
fn switches_toggle_and_density_selects_in_the_model() {
    let mut model = native();
    chord(&mut model);
    assert!(model.settings_data().heatmap);
    assert!(model.settings_data().grain);
    click(&mut model, Some(Widget::settings_switch(0)));
    assert!(!model.settings_data().heatmap);
    click(&mut model, Some(Widget::settings_switch(1)));
    assert!(!model.settings_data().grain);
    click(&mut model, Some(Widget::settings_switch(1)));
    assert!(model.settings_data().grain);
    click(&mut model, Some(Widget::settings_density(1)));
    assert_eq!(model.settings_data().density, Density::Compact);
    click(&mut model, Some(Widget::settings_density(0)));
    assert_eq!(model.settings_data().density, Density::Comfortable);
}

#[test]
fn open_file_opens_the_panes_source_and_stands_the_surface_down() {
    let mut model = native();
    chord(&mut model);
    click(
        &mut model,
        Some(Widget::settings_nav(SettingsGroup::Policy.index())),
    );
    let cmds = click(&mut model, Some(Widget::SettingsOpenFile.id()));
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: ".magistr/team.json".to_owned()
        })]
    );
    assert!(model.settings().is_none());
}

#[test]
fn with_settings_seeds_the_panes_values() {
    let data = SettingsData {
        project_path: "~/Code/magistr".to_owned(),
        team_size: Some(4),
        ..SettingsData::default()
    };
    let model = native().with_settings(data.clone());
    assert_eq!(model.settings_data(), &data);
}
