use super::*;
use crate::cockpit::palette_row::PaletteGo;
use crate::cockpit::{GalaxyData, GalaxyProject, Palette, Place, Size, Surface, Widget, update};
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use crate::job::{JobId, JobOutput};

/// A hosted window on `here`, at the home page, knowing one other
/// project with a directory and one without.
fn window() -> Model {
    let mut model = Model::new(Size::new(120, 40))
        .with_hosted_work()
        .with_project("here", "/projects/here")
        .with_surface(Surface::Native)
        .with_place(Place::Session);
    let _ = update(&mut model, Msg::Paste("Keep my draft".into()));
    model = model.with_place(Place::Galaxy);
    let data = GalaxyData {
        projects: vec![
            GalaxyProject {
                name: "here".into(),
                dir: Some("/projects/here".into()),
                here: true,
                ..GalaxyProject::default()
            },
            GalaxyProject {
                name: "other".into(),
                dir: Some("/projects/other".into()),
                ..GalaxyProject::default()
            },
            GalaxyProject::named("nowhere"),
        ],
        spent_cents: 0,
        cap_cents: None,
        stop_cents: None,
    };
    let _ = update(
        &mut model,
        Msg::Job {
            id: JobId::new(1),
            output: JobOutput::Galaxy(Box::new(data)),
        },
    );
    model
}

fn click(index: usize) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
        target: Some(Widget::galaxy_cluster(index)),
    }
}

fn opens(cmds: &[Cmd]) -> Vec<&str> {
    cmds.iter()
        .filter_map(|cmd| match cmd {
            Cmd::OpenProject(dir) => Some(dir.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_press_or_return_on_another_project_asks_the_host_once_and_keeps_the_draft() {
    let mut pressed = window();
    let cmds = update(&mut pressed, click(1));
    assert_eq!(opens(&cmds), ["/projects/other"]);
    assert_eq!(pressed.galaxy().opening(), Some("/projects/other"));
    assert_eq!(pressed.place(), Some(Place::Galaxy), "the camera holds");
    assert_eq!(pressed.prompt().text(), "Keep my draft");
    let again = update(&mut pressed, click(1));
    assert!(
        opens(&again).is_empty(),
        "one ask while it opens: {again:?}"
    );

    let mut keyed = window();
    let _ = update(&mut keyed, Msg::Key(KeyEvent::new(KeyCode::Right)));
    let _ = update(&mut keyed, Msg::Key(KeyEvent::new(KeyCode::Right)));
    let cmds = update(&mut keyed, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert_eq!(opens(&cmds), ["/projects/other"]);
}

#[test]
fn a_project_without_a_directory_or_a_window_without_a_host_only_selects() {
    let mut model = window();
    let cmds = update(&mut model, click(2));
    assert!(opens(&cmds).is_empty());
    assert_eq!(model.galaxy().selected(), Some(2));
    assert!(!can_open_project(&model, 2));
    assert!(can_open_project(&model, 1));

    let mut unhosted = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_place(Place::Galaxy);
    let seeded = window();
    let _ = update(
        &mut unhosted,
        Msg::Job {
            id: JobId::new(1),
            output: JobOutput::Galaxy(Box::new(seeded.galaxy().data().cloned().expect("data"))),
        },
    );
    assert!(opens(&update(&mut unhosted, click(1))).is_empty());
    assert!(!can_open_project(&unhosted, 1));
}

#[test]
fn the_palette_lists_the_other_projects_and_opens_the_chosen_one() {
    let mut model = window();
    model.palette = Some(Palette::default());
    let rows = Palette::rows(&model);
    let projects: Vec<_> = rows
        .iter()
        .filter(|row| matches!(row.go, PaletteGo::Project(_)))
        .collect();
    assert_eq!(
        projects.len(),
        1,
        "only an openable project that is not here"
    );
    assert_eq!(projects[0].verb, "Open project");
    assert_eq!(projects[0].name.as_deref(), Some("other"));
    let PaletteGo::Project(index) = projects[0].go else {
        unreachable!()
    };
    assert_eq!(opens(&open(&mut model, index)), ["/projects/other"]);
}

#[test]
fn work_in_flight_does_not_prevent_selecting_another_project() {
    let mut model = window();
    model.agent_activity.active = true;
    let cmds = update(&mut model, click(1));
    assert_eq!(opens(&cmds), ["/projects/other"], "{cmds:?}");
    assert_eq!(model.galaxy().opening(), Some("/projects/other"));
    assert_eq!(model.prompt().text(), "Keep my draft");
}

#[test]
fn a_host_refusal_lands_on_the_project_and_the_current_one_stays_open() {
    let mut model = window();
    let _ = update(&mut model, click(1));
    let cmds = update(
        &mut model,
        Msg::CommandFailed {
            cmd: Cmd::OpenProject("/projects/other".into()),
            error: "Another Magistr window has this project open.".into(),
        },
    );
    assert_eq!(model.galaxy().opening(), None);
    assert_eq!(
        model.galaxy().refusal("/projects/other"),
        Some("Magistr could not open the project. Another Magistr window has this project open.")
    );
    assert!(cmds.iter().any(|cmd| matches!(cmd, Cmd::Announce(_))));
    assert_eq!(model.galaxy().home(), Some(0), "still rooted on here");
    assert_eq!(model.prompt().text(), "Keep my draft");
    // A retry clears the words and asks again.
    assert_eq!(opens(&update(&mut model, click(1))), ["/projects/other"]);
    assert_eq!(model.galaxy().refusal("/projects/other"), None);
}

#[test]
fn a_rehosted_window_shows_the_new_project_and_keeps_what_the_window_owns() {
    let mut model = window();
    let _ = update(&mut model, Msg::Resize(Size::new(150, 50)));
    let _ = update(&mut model, click(1));
    let next = ProjectSeed {
        name: "other".into(),
        dir: "/projects/other".into(),
        title: "other · acp".into(),
        ..ProjectSeed::default()
    };
    let address = super::super::SessionAddress {
        project: "other".into(),
        workspace: "/projects/other".into(),
        session: "other-session".into(),
    };
    let mut next = next;
    next.session_address = Some(address.clone());
    next.draft = Some("Other draft".into());
    let cmds = update(
        &mut model,
        Msg::SessionFrame {
            address,
            sequence: 1,
            seed: Some(Box::new(next)),
            messages: Vec::new(),
        },
    );
    assert_eq!(model.title(), Some("other · acp"));
    assert_eq!(model.galaxy().projects()[0].name, "other");
    assert!(model.galaxy().projects()[0].here);
    assert!(
        model.hosts_work(),
        "the next project can be switched from too"
    );
    assert_eq!(model.galaxy().opening(), None);
    assert_eq!(model.size(), Size::new(150, 50), "the window's size");
    assert_eq!(
        model.prompt().text(),
        "Other draft",
        "the selected session owns its draft"
    );
    assert_eq!(model.place(), Some(Place::Sky), "the new project's roadmap");
    assert!(
        cmds.contains(&Cmd::Announce("Opened other.".into())),
        "{cmds:?}"
    );
}
#[test]
fn commands_are_addressed_before_navigation_and_late_frames_are_ignored() {
    use magistr_application::sessions::AgentEvent;
    let first = super::super::SessionAddress {
        project: "one".into(),
        workspace: "/one".into(),
        session: "one".into(),
    };
    let second = super::super::SessionAddress {
        project: "two".into(),
        workspace: "/two".into(),
        session: "two".into(),
    };
    let mut model = ProjectSeed {
        session_address: Some(first.clone()),
        ..ProjectSeed::default()
    }
    .seed(window());
    let commands = update(&mut model, click(1));
    assert!(commands.iter().any(|command| matches!(command, Cmd::Session { address, command, draft } if address == &first && matches!(&**command, Cmd::OpenProject(_)) && draft == "Keep my draft")));
    let next = ProjectSeed {
        name: "two".into(),
        session_address: Some(second.clone()),
        draft: Some("second draft".into()),
        ..ProjectSeed::default()
    };
    update(
        &mut model,
        Msg::SessionFrame {
            address: second.clone(),
            sequence: 20,
            seed: Some(Box::new(next)),
            messages: Vec::new(),
        },
    );
    let before = model.agent_events;
    update(
        &mut model,
        Msg::SessionFrame {
            address: first.clone(),
            sequence: 40,
            seed: None,
            messages: vec![Msg::Agent(AgentEvent::Text("old session".into()))],
        },
    );
    update(
        &mut model,
        Msg::SessionFrame {
            address: second.clone(),
            sequence: 19,
            seed: None,
            messages: vec![Msg::Agent(AgentEvent::Text("old sequence".into()))],
        },
    );
    update(
        &mut model,
        Msg::CommandFailed {
            cmd: Cmd::Session {
                address: first,
                command: Box::new(Cmd::Prompt("old rejected".into())),
                draft: String::new(),
            },
            error: "closed".into(),
        },
    );
    assert_eq!(model.agent_events, before);
    assert_eq!(model.prompt().text(), "second draft");
    update(
        &mut model,
        Msg::SessionFrame {
            address: second,
            sequence: 21,
            seed: None,
            messages: vec![Msg::Agent(AgentEvent::Text("current".into()))],
        },
    );
    assert_eq!(model.agent_events, before + 1);
}
