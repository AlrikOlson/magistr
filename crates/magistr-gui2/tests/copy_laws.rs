//! copy laws over the blessed views: every text run on the window belongs to a
//! component, no run says a metaphor word or a `$`, every state word is one of the
//! seven, money is spelled the one way, a scope is named by an identity row, and a
//! control names its chunk id.
//!
//! The vocabulary is the cockpit's (`Copy`, `StateWord`) and the stamp is the element
//! layer's (`Component`, `Element::claim`); what this suite pins is the *reading* of
//! every fixture through both, in both media, so a string that reaches the window
//! without a component — or with a word the design forbids — is a failing row with the
//! view's id and the string on it rather than a reviewer's impression.

use magistr_cockpit::cockpit::{
    Copy, FrameClock, Model, Motion, Msg, Size, StateWord, Surface, ThemeChoice, Widget, update,
};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_gui2::fixture::Fixture;
use magistr_gui2::{Component, Medium, RootComposition, TextRun};

/// The cockpit grid the fixtures are built on, the cell the frame
/// measures with, and the volume's window.
const SIZE: Size = Size::new(120, 40);
const CELL: (f32, f32) = (9.0, 20.0);
const VIEWPORT: (f32, f32) = (1100.0, 640.0);

/// `model` on a window, as the frame declares it before mapping the
/// root.
fn native(model: Model) -> Model {
    Fixture::with_pixels(model.with_surface(Surface::Native))
}

/// View 03 idled to rest — the murmur's status rows are the one place
/// the resting room prints a state word.
fn resting() -> Model {
    let mut model = native(Fixture::mock("03", SIZE).expect("view 03"));
    for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
        update(&mut model, Msg::Tick);
    }
    model
}

/// Every window this suite reads: the nineteen views, the working
/// stage, the ship moment and the resting sky, each in both media.
fn windows() -> Vec<(String, Model)> {
    let mut out = Vec::new();
    for medium in Medium::ALL {
        for view in Fixture::VIEWS {
            if let Some(model) = Fixture::mock(view, SIZE) {
                out.push((format!("{medium}-{view}"), native(model)));
            }
        }
        out.push((format!("{medium}-working"), native(Fixture::working(SIZE))));
        out.push((format!("{medium}-ship"), native(Fixture::ship_08(SIZE))));
        out.push((format!("{medium}-rest"), resting()));
    }
    out
}

/// The runs of `model`'s window in `medium`.
fn runs(model: &Model, medium: Medium) -> Vec<TextRun> {
    RootComposition::el(
        model,
        medium,
        CELL,
        VIEWPORT,
        &FrameClock::new(0, std::time::Duration::ZERO),
    )
    .runs()
}

/// The medium a window's name was built in.
fn medium_of(name: &str) -> Medium {
    if name.starts_with("day") {
        Medium::Day
    } else {
        Medium::Night
    }
}

/// no naked text: every run on every window is some component's.
#[test]
fn every_run_on_every_window_belongs_to_a_component() {
    let mut naked = Vec::new();
    let mut total = 0;
    for (name, model) in windows() {
        for run in runs(&model, medium_of(&name)) {
            total += 1;
            if run.owner.is_none() {
                naked.push(format!("{name}: {:?}", run.text));
            }
        }
    }
    assert!(
        total > 500,
        "read only {total} runs — the walk is wrong, not the views"
    );
    assert!(
        naked.is_empty(),
        "runs no component claimed:\n{}",
        naked.join("\n")
    );
}

/// These components contain application-authored copy. Other components contain user
/// data such as chunk IDs, file names, project paths, and prose, so copy restrictions
/// do not apply to their contents.
const COPY: [Component; 13] = [
    Component::Cap,
    Component::Chip,
    Component::Button,
    Component::KeyHint,
    Component::ModeBadge,
    Component::PhaseRail,
    Component::Stat,
    Component::Counter,
    Component::Meter,
    Component::Nav,
    Component::ThemeCard,
    Component::SettingRow,
    Component::Slot,
];

/// The part of a run that is the design's copy: all of it, except that a control names
/// its chunk id after the verb, so only a button's verb is read.
fn copy_of(run: &TextRun) -> &str {
    if run.owner == Some(Component::Button) {
        run.text.split_whitespace().next().unwrap_or("")
    } else {
        &run.text
    }
}

/// Application copy contains no metaphor terms or dollar symbols. Theme names and
/// externally supplied data have separate rules.
#[test]
fn no_copy_says_a_metaphor_word_and_no_run_wears_a_dollar_sign() {
    let media: Vec<&str> = ThemeChoice::ALL.iter().map(|c| c.label()).collect();
    let mut offences = Vec::new();
    let mut read = 0;
    for (name, model) in windows() {
        for run in runs(&model, medium_of(&name)) {
            if run.text.contains('$') {
                offences.push(format!("{name}: {:?} wears a $", run.text));
            }
            if !run.owner.is_some_and(|owner| COPY.contains(&owner))
                || media.contains(&run.text.as_str())
            {
                continue;
            }
            read += 1;
            if let Some(word) = Copy::metaphor_in(copy_of(&run)) {
                offences.push(format!("{name}: {:?} says {word}", run.text));
            }
        }
    }
    assert!(read > 200, "read only {read} copy runs");
    assert!(offences.is_empty(), "{}", offences.join("\n"));
}

/// one state vocabulary: a run that is a state word is one of the seven, and never one
/// of the synonyms a view reaches for by hand. The sky's card and the murmur both print
/// one, so the check is not vacuous.
#[test]
fn every_state_word_is_one_of_the_seven() {
    let mut impostors = Vec::new();
    let mut seen = 0;
    for (name, model) in windows() {
        for run in runs(&model, medium_of(&name)) {
            let text = run.text.trim();
            if StateWord::IMPOSTORS.contains(&text) {
                impostors.push(format!("{name}: {text:?}"));
            }
            if StateWord::parse(text).is_some() {
                seen += 1;
            }
        }
    }
    assert!(
        impostors.is_empty(),
        "state words outside the seven:\n{}",
        impostors.join("\n")
    );
    assert!(
        seen >= 4,
        "only {seen} state words on every window — the views print none?"
    );
}

/// money is `2.07 USD`: two places and the code, wherever the unit appears.
#[test]
fn money_is_two_places_and_the_code() {
    let mut off = Vec::new();
    let mut seen = 0;
    for (name, model) in windows() {
        for run in runs(&model, medium_of(&name)) {
            let words: Vec<&str> = run.text.split_whitespace().collect();
            for (i, word) in words.iter().enumerate() {
                if *word != "USD" {
                    continue;
                }
                seen += 1;
                let amount = words.get(i.wrapping_sub(1)).copied().unwrap_or("");
                let well_formed = amount.rsplit_once('.').is_some_and(|(whole, cents)| {
                    !whole.is_empty()
                        && whole.chars().all(|c| c.is_ascii_digit())
                        && cents.len() == 2
                        && cents.chars().all(|c| c.is_ascii_digit())
                });
                if !well_formed {
                    off.push(format!("{name}: {:?}", run.text));
                }
            }
        }
    }
    assert!(
        off.is_empty(),
        "money not spelled the one way:\n{}",
        off.join("\n")
    );
    assert!(seen >= 2, "only {seen} amounts on every window");
}

/// controls name chunk ids: the sky card's verb says which chunk it starts, the way
/// Birth's `Claim <id>` does.
#[test]
fn the_sky_cards_verb_names_the_chunk() {
    let mut model = native(Fixture::mock("04", SIZE).expect("view 04"));
    // Click the ready star through the shared update's own star arm,
    // exactly as a real click arrives, so the card opens.
    let ready = (0..model.roadmap().dag_nodes().len())
        .find(|&i| {
            model
                .roadmap()
                .graph_chunk(i)
                .is_some_and(|c| c.state == magistr_cockpit::cockpit::ChunkState::Pending)
        })
        .expect("view 04 holds a claimable star");
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
            target: Some(Widget::sky_star(ready)),
        },
    );
    let id = model
        .roadmap()
        .selected()
        .expect("the click selected")
        .id
        .clone();
    let verbs: Vec<String> = runs(&model, Medium::Night)
        .into_iter()
        .filter(|run| run.owner == Some(Component::Button))
        .map(|run| run.text)
        .filter(|text| text.starts_with("Start ") || text.starts_with("Complete "))
        .collect();
    assert!(!verbs.is_empty(), "view 04's card offers a verb");
    for verb in verbs {
        assert!(verb.ends_with(&id), "{verb} does not name {id}");
    }
}

/// a scope is named by an identity row: Settings' `This project` is followed by the row
/// that says which project.
#[test]
fn settings_names_the_scope_with_an_identity_row() {
    let model = native(Fixture::mock("10", SIZE).expect("view 10"));
    let identity: Vec<String> = runs(&model, Medium::Day)
        .into_iter()
        .filter(|run| run.owner == Some(Component::IdentityRow))
        .map(|run| run.text)
        .collect();
    assert!(
        !identity.is_empty(),
        "no identity row on the settings surface"
    );
    let name = identity[0].clone();
    assert!(!name.is_empty(), "the identity row names the project");
}
