use super::*;
use crate::cockpit::{Model, Msg, Size, update};
use crate::engine::{EngineEvent, StoreSeed};

/// A roadmap with `shipped` done under `desktop-gui`, `next` waiting
/// on it alone (so it is Ready), `later` waiting on it and on `next`
/// (so it is Pending), and `elsewhere` standing on nothing of it.
fn seeded() -> Model {
    let mut m = Model::new(Size::new(120, 40));
    let chunk = |id: &str, status: &str, deps: Vec<&str>| {
        serde_json::json!({
            "id": id, "name": format!("{id} name"), "status": status, "priority": 200,
            "group": "desktop-gui", "deps": deps,
        })
    };
    let mut shipped = chunk("shipped", "done", vec![]);
    shipped["title"] = "The design language, decided live".into();
    shipped["description"] = "One view, two arrangements.".into();
    shipped["updated_at"] = "2026-08-30T14:02:00Z".into();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(StoreSeed {
            ship_status: serde_json::Value::Null,
            chunk: None,
            done: 1,
            total: 4,
            signals_pending: 0,
            chunks: vec![
                shipped,
                chunk("next", "pending", vec!["shipped"]),
                chunk("later", "pending", vec!["shipped", "next"]),
                chunk("elsewhere", "pending", vec![]),
            ],
            signals: Vec::new(),
            trace: Vec::new(),
        })),
    );
    m
}

#[test]
fn the_chapter_is_read_off_the_chunk_and_what_stands_on_it() {
    let m = seeded();
    let chunk = m.roadmap().chunk("shipped").expect("seeded");
    let chapter = Chapter::of(chunk, m.roadmap());
    assert_eq!(chapter.chunk, "shipped");
    assert_eq!(chapter.title, "The design language, decided live");
    assert_eq!(chapter.prose, "One view, two arrangements.");
    assert_eq!(chapter.group.as_deref(), Some("desktop-gui"));
    assert_eq!(chapter.shipped_day().as_deref(), Some("Aug 30"));
    assert_eq!(
        chapter.cap(),
        "Report \u{b7} Shipped Aug 30 \u{b7} desktop-gui"
    );
    assert_eq!(
        chapter.unlocked,
        vec![
            ("next".to_owned(), "Ready".to_owned()),
            ("later".to_owned(), "Pending".to_owned()),
        ],
        "the dependents in store order, each with its star's state"
    );
    assert_eq!(
        chapter.unlocked_text(),
        "next \u{b7} Ready, later \u{b7} Pending"
    );
}

#[test]
fn a_bare_record_falls_back_to_its_name_and_says_what_is_missing() {
    let m = seeded();
    let chunk = m.roadmap().chunk("elsewhere").expect("seeded");
    let chapter = Chapter::of(chunk, m.roadmap());
    assert_eq!(chapter.title, "elsewhere name", "no claim: the name");
    assert_eq!(chapter.prose_text(), Chapter::NO_PROSE);
    assert_eq!(chapter.shipped_day(), None);
    assert_eq!(chapter.cap(), "Report \u{b7} Shipped \u{b7} desktop-gui");
    assert_eq!(chapter.unlocked_text(), Chapter::NOTHING_UNLOCKED);
}

#[test]
fn the_markdown_and_the_page_carry_the_same_words_and_the_receipts() {
    let m = seeded();
    let chunk = m.roadmap().chunk("shipped").expect("seeded");
    let chapter = Chapter::of(chunk, m.roadmap());
    let mut shelf = Shelf::new("shipped", &["think:388".to_owned()]);
    shelf.cost_cents = Some(207);
    shelf.tokens = Some(41_000);
    let md = chapter.markdown(&shelf);
    assert!(
        md.starts_with("# The design language, decided live\n\n"),
        "{md}"
    );
    assert!(
        md.contains("Report \u{b7} Shipped Aug 30 \u{b7} desktop-gui\n"),
        "{md}"
    );
    assert!(md.contains("One view, two arrangements.\n"), "{md}");
    assert!(
        md.contains("## Unlocked\n\n- next \u{b7} Ready\n- later \u{b7} Pending\n"),
        "{md}"
    );
    assert!(md.contains("## Receipts\n\n- Shipped: no stamp\n"), "{md}");
    assert!(md.contains("- Cost: 2.07 USD \u{b7} 41k tokens\n"), "{md}");
    assert!(md.contains("- Trace: think 388\n"), "{md}");

    let html = chapter.html(&shelf);
    assert!(html.starts_with("<!doctype html>"), "{html}");
    assert!(html.contains("<title>The design language, decided live</title>"));
    assert!(html.contains("<h1>The design language, decided live</h1>"));
    assert!(html.contains("<p>One view, two arrangements.</p>"));
    assert!(html.contains("<li><code>next</code> \u{b7} Ready</li>"));
    assert!(
        html.contains("<li>Cost: 2.07 USD \u{b7} 41k tokens</li>"),
        "{html}"
    );
    assert!(!html.contains("<script"), "no script on a share page");
    assert!(!html.contains("http"), "no remote asset on a share page");
}

#[test]
fn the_page_escapes_the_record_it_prints() {
    let chapter = Chapter {
        chunk: "x".to_owned(),
        title: "a <b> & \"c\"".to_owned(),
        prose: "it's".to_owned(),
        ..Chapter::default()
    };
    let html = chapter.html(&Shelf::new("x", &[]));
    assert!(
        html.contains("<h1>a &lt;b&gt; &amp; &quot;c&quot;</h1>"),
        "{html}"
    );
    assert!(html.contains("<p>it&#39;s</p>"), "{html}");
    assert!(!html.contains("<b>"));
}

#[test]
fn the_model_hands_out_the_chapter_of_the_selected_done_chunk_only() {
    let mut m = seeded();
    assert_eq!(m.chapter(), None, "no selection, no chapter");
    let now = m.now();
    assert!(m.roadmap.select("next", now, false));
    assert_eq!(m.chapter(), None, "a chunk still to come has no chapter");
    assert!(m.roadmap.select("shipped", now, false));
    let chapter = m.chapter().expect("the shipped chunk's chapter");
    assert_eq!(chapter.title, "The design language, decided live");
}
