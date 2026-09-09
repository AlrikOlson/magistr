use magistr_cockpit::cockpit::{Msg, Place, Size, Widget, update};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

const FIELD: (f32, f32) = (1100.0, 553.0);

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn has_id(root: &Element, id: ElementId) -> bool {
    let mut found = false;
    root.each(&mut |_, element| found |= element.id == Some(id));
    found
}

/// Every project is a cluster wearing its sigil and its status line —
/// the capture's own three lines — the field is reserved, and the verb
/// wears its id.
#[test]
fn every_project_wears_its_sigil_and_its_status_line() {
    let model = Fixture::galaxy(Size::new(120, 40));
    assert_eq!(model.place(), Some(Place::Galaxy));
    let field = GalaxyComposition::el(&model, Medium::Night, FIELD);
    let texts = texts(&field);
    for name in ["magistr", "think-and-ship", "quicksilver"] {
        assert!(texts.iter().any(|t| t == name), "{name} is named");
    }
    assert!(texts.iter().any(|t| t == "211 done \u{b7} 1 running"));
    assert!(texts.iter().any(|t| t == "1 Action required \u{b7} 11 min"));
    assert!(texts.iter().any(|t| t == "Idle \u{b7} 3 d"));
    let mut sigils = 0;
    field.each(&mut |_, element| {
        if let Kind::Mark {
            mark: Mark::Sigil(_),
            ..
        } = &element.kind
        {
            sigils += 1;
        }
    });
    assert_eq!(sigils, 3, "one sigil per project");
    assert!(
        has_id(&field, GalaxyComposition::FIELD),
        "the field is reserved"
    );
    assert!(
        has_id(&field, GalaxyComposition::NEW),
        "the verb wears its id"
    );
    assert!(texts.iter().any(|t| t == "+ New project"));
}

/// The header chips count across every project and the foot reads the
/// projects, the spend against the cap and the two hints.
#[test]
fn the_chips_and_the_foot_read_the_galaxys_totals() {
    let model = Fixture::galaxy(Size::new(120, 40));
    let chips = texts(&GalaxyComposition::chips(&model, Medium::Day));
    assert_eq!(chips, vec!["2 running", "1 action required", "\u{2318}K"]);
    let foot = texts(&GalaxyComposition::foot(&model, Medium::Day));
    assert!(foot.iter().any(|t| t == "3 projects"), "{foot:?}");
    assert!(foot.iter().any(|t| t == "2.45 / 12.00 USD"), "{foot:?}");
    assert!(foot.iter().any(|t| t == "Open") && foot.iter().any(|t| t == "Zoom"));
}

/// Before the job delivers, the window's own project alone is the
/// Galaxy, and what the window knows about itself is laid over it: a
/// pending question counts on its cluster and on the chips.
#[test]
fn an_unseeded_galaxy_is_the_windows_own_project_with_its_live_state() {
    use magistr_cockpit::agent::{AgentEvent, PermissionChoice, PermissionKind};
    use magistr_cockpit::cockpit::Surface;
    use magistr_engine::policy::RiskTier;
    let mut model = magistr_cockpit::cockpit::Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_place(Place::Galaxy);
    let alone = GalaxyComposition::projects(&model);
    assert_eq!(alone.len(), 1);
    assert!(alone[0].here);
    assert_eq!(alone[0].asking, 0);
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 1,
            title: "Write a file".to_owned(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            risk: RiskTier::Low,
            plan: None,
        }),
    );
    let asking = GalaxyComposition::projects(&model);
    assert_eq!(asking[0].asking, 1, "the dock's question is the cluster's");
    let chips = texts(&GalaxyComposition::chips(&model, Medium::Night));
    assert!(chips.iter().any(|t| t == "1 action required"), "{chips:?}");
    assert!(
        !chips.iter().any(|t| t.ends_with("running")),
        "nothing runs"
    );
    let foot = texts(&GalaxyComposition::foot(&model, Medium::Night));
    assert!(foot.iter().any(|t| t == "1 project"), "{foot:?}");
    assert!(
        !foot.iter().any(|t| t.contains("USD")),
        "no spend before the job"
    );
}

/// The copy is derived once, from the numbers: the durations the
/// volume prints and the three readings of a project.
#[test]
fn the_status_line_is_derived_from_the_numbers() {
    let lex = Lexicon::of(Medium::Night);
    let line = |project: GalaxyProject| status_line(&project, &lex).1;
    assert_eq!(ago(30), "30 s");
    assert_eq!(ago(11 * 60), "11 min");
    assert_eq!(ago(5 * 3600), "5 h");
    assert_eq!(ago(86_400), "1 d");
    assert_eq!(ago(3 * 86_400), "3 d");
    assert_eq!(
        line(GalaxyProject {
            asking: 2,
            ..GalaxyProject::default()
        }),
        "2 Action required"
    );
    assert_eq!(
        line(GalaxyProject {
            running: 1,
            ..GalaxyProject::default()
        }),
        "1 running"
    );
    assert_eq!(
        line(GalaxyProject {
            done: Some(40),
            ..GalaxyProject::default()
        }),
        "40 done \u{b7} idle"
    );
    assert_eq!(line(GalaxyProject::default()), "Idle");
    let (name_ink, _, status_ink) = status_line(
        &GalaxyProject {
            asking: 1,
            ..GalaxyProject::default()
        },
        &lex,
    );
    assert_eq!(status_ink, lex.peach, "a question glows amber");
    assert_eq!(name_ink, lex.tx);
    let (idle_ink, _, _) = status_line(&GalaxyProject::default(), &lex);
    assert_eq!(idle_ink, lex.sb1, "an idle project dims its name");
}

/// The frame's wiring: the field's id is the Galaxy's field widget and
/// the verb's is the new-project widget, and nothing else here is
/// clickable — the clusters are registered by the drawing.
#[test]
fn the_field_and_the_verb_route_to_their_widgets() {
    use crate::comp_root::RootComposition;
    let model = Fixture::galaxy(Size::new(120, 40));
    assert_eq!(
        RootComposition::target_of(&model, GalaxyComposition::FIELD),
        Some(Widget::GalaxyField.id())
    );
    assert_eq!(
        RootComposition::target_of(&model, GalaxyComposition::NEW),
        Some(Widget::GalaxyNew.id())
    );
}
