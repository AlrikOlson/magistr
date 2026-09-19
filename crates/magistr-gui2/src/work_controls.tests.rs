use magistr_cockpit::cockpit::{LoopEvent, Model, Msg, Size, update};

use super::*;

fn started() -> Model {
    let mut model = Model::new(Size::new(120, 40)).with_hosted_work();
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    model
}

fn labels(model: &Model) -> Vec<&'static str> {
    WorkControls::verbs(model)
        .into_iter()
        .map(|(_, label, _)| label)
        .collect()
}

/// The starts are offered only by a host that carries them out in place, and only
/// while no loop is live; an ended loop offers them again.
#[test]
fn the_starts_are_offered_while_idle_in_a_hosted_session_only() {
    let size = Size::new(120, 40);
    assert!(labels(&Model::new(size)).is_empty(), "no host, no start");
    assert_eq!(
        labels(&Model::new(size).with_hosted_work()),
        ["Start next item", "Start loop"]
    );
    let mut model = started();
    assert_eq!(labels(&model), ["Pause", "Step", "Checkpoint", "Abort"]);
    update(&mut model, Msg::Loop(LoopEvent::Paused("held".to_owned())));
    assert_eq!(labels(&model)[0], "Resume");
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("finished".to_owned())),
    );
    assert_eq!(labels(&model), ["Start next item", "Start loop"]);
}

/// Each verb is the cockpit's own widget wearing its registry keys.
#[test]
fn every_verb_is_a_cockpit_widget_with_the_registrys_keys() {
    let idle: Vec<_> = WorkControls::verbs(&Model::new(Size::new(120, 40)).with_hosted_work())
        .into_iter()
        .map(|(widget, _, binding)| (widget, binding.keys))
        .collect();
    assert_eq!(
        idle,
        [
            (Widget::LaunchNext, Binding::NEXT.keys),
            (Widget::LaunchLoop, Binding::LOOP.keys)
        ]
    );
    let live: Vec<_> = WorkControls::verbs(&started())
        .into_iter()
        .map(|(widget, ..)| widget)
        .collect();
    assert_eq!(live, Widget::LOOP_VERBS);
}
