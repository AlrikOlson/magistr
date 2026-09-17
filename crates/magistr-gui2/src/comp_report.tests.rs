use magistr_cockpit::cockpit::{Model, Place, Size, Surface};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push(content.clone());
        }
    });
    out
}

fn ids_of(root: &Element) -> Vec<ElementId> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Some(id) = element.id {
            out.push(id);
        }
    });
    out
}

fn pictures(root: &Element) -> usize {
    let mut n = 0;
    root.each(&mut |_, element| {
        if matches!(element.kind, Kind::Picture { .. }) {
            n += 1;
        }
    });
    n
}

fn view() -> Model {
    Fixture::mock("09", Size::new(120, 40)).expect("view 09 has a fixture")
}

/// A completed chunk with a deliverables shelf opens at its Report.
#[test]
fn the_fixture_is_the_captures_report() {
    let model = view();
    assert_eq!(model.place(), Some(Place::Report));
    let chapter = model.chapter().expect("the shipped chunk's chapter");
    assert_eq!(chapter.title, "The design language, decided live");
    assert_eq!(
        chapter.cap(),
        "Report \u{b7} Shipped Aug 30 \u{b7} desktop-gui"
    );
    assert_eq!(chapter.unlocked_text(), "gui2-2b \u{b7} Ready");
    let shelf = model.shelf().expect("the shelf landed");
    assert_eq!(shelf.count(StillKind::Still), 2);
    assert_eq!(shelf.count(StillKind::Filmstrip), 3);
    assert_eq!(shelf.recording_text(), "12 min");
    assert_eq!(shelf.spend_text(), "2.07 USD \u{b7} 41k tokens");
    assert_eq!(shelf.trace_refs_text(), "think:388");
    let path: Vec<String> = model.path().into_iter().map(|stop| stop.label).collect();
    assert_eq!(path, ["projects", "magistr", "gui2-2a \u{b7} Done"]);
}

/// The chapter: the cap with its glyph, the title in the display face,
/// the prose in the chapter's serif, the Unlocked line — and the rail:
/// two tiles, the two rows, the four receipts with their verbs.
#[test]
fn the_report_is_the_chapter_beside_the_shelf() {
    let model = view();
    let body = ReportComposition::el(&model, Medium::Night);
    let texts = texts(&body);
    for expected in [
        "Report \u{b7} Shipped Aug 30 \u{b7} desktop-gui",
        "The design language, decided live",
        "Unlocked",
        "gui2-2b \u{b7} Ready",
        "Deliverables",
        "still \u{b7} ",
        "turn",
        "still \u{b7} ",
        "sky",
        "Session recording",
        "\u{b7} 12 min",
        "Motion filmstrips",
        "\u{b7} 3 strips",
        "Receipts",
        "commit",
        "a3f9c21",
        "View",
        "gates",
        "3 passed",
        "Log",
        "cost",
        "2.07 USD \u{b7} 41k tokens",
        "Detail",
        "trace",
        "think:388",
        "Open",
    ] {
        assert!(
            texts.contains(&expected.to_owned()),
            "{expected}: {texts:?}"
        );
    }
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("One view, two arrangements.")),
        "the prose: {texts:?}"
    );
    assert_eq!(pictures(&body), 2, "two tiles, the filmstrips counted");
    let ids = ids_of(&body);
    for id in [
        ReportComposition::COMMIT,
        ReportComposition::GATES,
        ReportComposition::COST,
        ReportComposition::TRACE,
    ] {
        assert!(ids.contains(&id), "{id:?} is placed");
    }
    assert!(
        !texts.contains(&"bundle".to_owned()) && !texts.contains(&"page".to_owned()),
        "nothing written yet"
    );
}

/// The title wears the display voice and the prose the chapter's
/// serif — the serif's one home, two seats — and nothing else on the
/// page is serif.
#[test]
fn the_serif_sits_on_the_title_and_the_prose_and_nowhere_else() {
    let model = view();
    let body = ReportComposition::el(&model, Medium::Day);
    let mut serif = Vec::new();
    body.each(&mut |_, element| {
        if let Kind::Text { voice, content } = &element.kind
            && voice.is_serif()
        {
            serif.push((*voice, content.clone()));
        }
    });
    assert_eq!(serif.len(), 2, "{serif:?}");
    assert_eq!(serif[0].0, Voice::DISPLAY);
    assert_eq!(serif[0].1, "The design language, decided live");
    assert_eq!(serif[1].0, ReportComposition::PROSE);
    assert!(serif[1].1.starts_with("One view"));
}

/// The header's slot is the two verbs: `Share page` in the accent and
/// `Export bundle` on the shelf's own id — no primary, the page has no
/// lit exit.
#[test]
fn the_chips_are_share_page_and_export_bundle() {
    let chips = ReportComposition::chips(Medium::Night);
    assert_eq!(texts(&chips), vec!["Share page", "Export bundle"]);
    assert_eq!(
        ids_of(&chips),
        vec![ReportComposition::SHARE, ShelfComposition::EXPORT]
    );
}

/// With no shelf landed and no chunk selected the page still stands,
/// in its own components, saying what is missing.
#[test]
fn a_report_with_nothing_under_it_says_so() {
    let model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_place(Place::Report);
    assert_eq!(model.chapter(), None);
    assert_eq!(model.shelf(), None);
    let body = ReportComposition::el(&model, Medium::Night);
    let texts = texts(&body);
    assert!(texts.contains(&"No report is open".to_owned()), "{texts:?}");
    assert!(texts.contains(&"no stills kept".to_owned()), "{texts:?}");
    assert!(
        texts.contains(&"\u{b7} not recorded".to_owned()),
        "{texts:?}"
    );
    assert!(texts.contains(&"\u{b7} 0 strips".to_owned()), "{texts:?}");
    assert!(texts.contains(&"none found".to_owned()), "{texts:?}");
    assert!(texts.contains(&"none kept".to_owned()), "{texts:?}");
    assert!(texts.contains(&"none linked".to_owned()), "{texts:?}");
    assert_eq!(pictures(&body), 0);
}

/// Once written, the bundle and the page are named on the rail by
/// their tails.
#[test]
fn a_written_bundle_and_page_are_named_on_the_rail() {
    let mut model = view();
    let mut shelf = Fixture::shelf();
    shelf.bundle = Some(std::path::PathBuf::from("/p/.magistr/deliverables/gui2-2a"));
    shelf.page = Some(std::path::PathBuf::from(
        "/p/.magistr/deliverables/gui2-2a/chapter.html",
    ));
    magistr_cockpit::cockpit::update(
        &mut model,
        magistr_cockpit::cockpit::Msg::Job {
            id: magistr_cockpit::job::JobId::new(9),
            output: magistr_cockpit::job::JobOutput::Shelf(Box::new(shelf)),
        },
    );
    let texts = texts(&ReportComposition::el(&model, Medium::Night));
    assert!(texts.contains(&"bundle".to_owned()), "{texts:?}");
    assert!(
        texts.contains(&".magistr/deliverables/gui2-2a".to_owned()),
        "{texts:?}"
    );
    assert!(texts.contains(&"page".to_owned()), "{texts:?}");
    assert!(
        texts.contains(&"deliverables/gui2-2a/chapter.html".to_owned()),
        "{texts:?}"
    );
}
