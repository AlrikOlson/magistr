use super::*;
use crate::cockpit::{
    LauncherData, LauncherSession, Msg, Place, SessionOutcome, Size, StageTab, Surface, Widget,
    update,
};
use crate::engine::EngineEvent;
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::job::{JobId, JobOutput};

fn native() -> Model {
    Model::new(Size::new(120, 40)).with_surface(Surface::Native)
}

fn key(model: &mut Model, code: KeyCode, mods: Modifiers) {
    let _ = update(model, Msg::Key(KeyEvent::new(code).with(mods)));
}

fn chord(model: &mut Model) {
    key(
        model,
        KeyCode::Char('F'),
        Modifiers::SUPER | Modifiers::SHIFT,
    );
}

fn typed(model: &mut Model, text: &str) {
    for c in text.chars() {
        key(model, KeyCode::Char(c), Modifiers::NONE);
    }
}

fn click(model: &mut Model, target: Option<crate::widget_id::WidgetId>) {
    let _ = update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target,
        },
    );
}

/// The capture's own corpus (view 13): four shipped chunks that carry
/// the phrase or one of its words, the think step whose thought asks
/// after the floor, the ship action on `theme/ink.rs` — and one recent
/// session that matches nothing.
fn seeded() -> Model {
    let mut model = native();
    let done = |id: &str, title: &str, description: &str, updated: &str| {
        serde_json::json!({
            "id": id, "name": id, "title": title, "status": "done", "priority": 200,
            "description": description, "updated_at": updated,
        })
    };
    let _ = update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
            progress: crate::harness::fixtures::progress(4, 5),
            chunks: crate::harness::fixtures::chunks_from(vec![
                done(
                    "latte-floor",
                    "Every light palette clears its floor",
                    "The Latte pens are all contrast-checked, nudged until it clears the contrast floor the validator declares, role by role.",
                    "2026-08-29T15:00:00Z",
                ),
                done(
                    "pane-parity",
                    "The native pane at parity",
                    "Stills in both media are measured; every run clears the cell pens\u{2019} own contrast floor (crates/magistr-gui2/src/lexicon.rs).",
                    "2026-08-30T09:30:00Z",
                ),
                done(
                    "sweep",
                    "The contrast sweep",
                    "A sweep over every painted cell asks the floor of each pen.",
                    "2026-08-27T09:30:00Z",
                ),
                done(
                    "ink",
                    "Ink at the consumer",
                    "The consumer fixes its own floor; the palette is never retuned.",
                    "2026-08-28T09:30:00Z",
                ),
                serde_json::json!({
                    "id": "gui2-2a", "name": "gui2-2a", "status": "in_progress", "priority": 159,
                    "description": "contrast floor everywhere — but not shipped, so not a chapter",
                }),
            ]),
            steps: vec![crate::harness::fixtures::step_from(serde_json::json!({
                "step_number": 387,
                "purpose": "the pen on screen",
                "thought": "Anti-aliasing is a rasterizer-question; the pen on screen is style after REVERSE, and the floor is asked of what was painted, never of the palette.",
            }))],
            tasks: vec![crate::harness::fixtures::task_from(serde_json::json!({
                "id": "ink",
                "actions": [{
                    "id": 198, "type": "code",
                    "description": "ensure_contrast at the consumer",
                    "files_touched": ["theme/ink.rs"],
                    "timestamp": "2026-08-30T10:00:00Z",
                }],
                "checks": [{"name": "contrast", "passed": true, "timestamp": "2026-08-30T10:01:00Z"}],
            }))],
            ..magistr_engine::NativeSnapshot::default()
        })),
    );
    let _ = update(
        &mut model,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Launcher(Box::new(LauncherData {
                project: "magistr".to_owned(),
                has_config: true,
                stack: "Rust / Cargo".to_owned(),
                git: "master, clean".to_owned(),
                backend: "ACP (available)".to_owned(),
                sessions: vec![LauncherSession {
                    id: Some("sess-1".to_owned()),
                    when: "Aug 30 14:02".to_owned(),
                    duration: "12m".to_owned(),
                    outcome: SessionOutcome::Success,
                    preview: "shipped the palette".to_owned(),
                }],
            })),
        },
    );
    model
}

#[test]
fn the_matcher_folds_case_needs_one_word_and_ranks_the_phrase_first() {
    let needles = Needles::of("Contrast Floor");
    assert_eq!(needles.score("nothing here"), None);
    let one = needles.score("a FLOOR").expect("one word");
    let two = needles.score("floor … contrast").expect("two words apart");
    let phrase = needles.score("the contrast floor").expect("the phrase");
    assert!(one < two && two < phrase, "{one} < {two} < {phrase}");
    assert_eq!(
        needles.marks("Contrast floor, and a floor"),
        vec![(0, 14), (22, 27)]
    );
    // A blank query is no query.
    assert!(Needles::of("   ").words.is_empty());
}

#[test]
fn the_snippet_cuts_to_a_clause_or_a_word_and_marks_the_match() {
    let needles = Needles::of("contrast floor");
    let (line, marks) = snippet(
        "The Latte pens are all contrast-checked, nudged until it clears the contrast floor the validator declares, role by role.",
        &needles,
    );
    assert_eq!(
        line,
        "\u{2026}nudged until it clears the contrast floor the validator declares\u{2026}"
    );
    assert_eq!(&line[marks[0].0..marks[0].1], "contrast floor");
    let (line, marks) = snippet(
        "Stills in both media are measured; every run clears the cell pens\u{2019} own contrast floor (crates/magistr-gui2/src/lexicon.rs).",
        &needles,
    );
    assert_eq!(
        line,
        "\u{2026}every run clears the cell pens\u{2019} own contrast floor\u{2026}"
    );
    assert_eq!(&line[marks[0].0..marks[0].1], "contrast floor");
    let (line, marks) = snippet(
        "Anti-aliasing is a rasterizer-question; the pen on screen is style after REVERSE, and the floor is asked of what was painted, never of the palette.",
        &needles,
    );
    assert_eq!(
        line,
        "\u{2026}the pen on screen is style after REVERSE, and the floor is asked of what was painted\u{2026}"
    );
    assert_eq!(&line[marks[0].0..marks[0].1], "floor");
    // A short text is shown whole, no ellipsis.
    let (line, _) = snippet("a floor", &needles);
    assert_eq!(line, "a floor");
}

#[test]
fn results_group_by_kind_cut_each_kind_and_count_every_hit() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "contrast floor");
    let results = Search::results(&model);
    assert_eq!(results.total, 6, "4 chapters + 1 step + 1 receipt");
    assert_eq!(results.count_text(), "6 results");
    let kinds: Vec<SearchKind> = results.rows.iter().map(|hit| hit.kind).collect();
    assert_eq!(
        kinds,
        [
            SearchKind::Report,
            SearchKind::Report,
            SearchKind::Trace,
            SearchKind::Receipt
        ]
    );
    let first = &results.rows[0];
    assert_eq!(
        first.title.as_deref(),
        Some("Every light palette clears its floor")
    );
    assert_eq!(first.when.as_deref(), Some("Aug 29"));
    assert_eq!(results.rows[1].when.as_deref(), Some("Aug 30"));
    let step = &results.rows[2];
    assert!(
        step.line
            .starts_with("think:387 \u{b7} \u{201c}\u{2026}the pen"),
        "{}",
        step.line
    );
    assert!(
        step.line.ends_with("painted\u{2026}\u{201d}"),
        "{}",
        step.line
    );
    assert_eq!(&step.line[step.marks[0].0..step.marks[0].1], "floor");
    let receipt = &results.rows[3];
    assert_eq!(
        receipt.line,
        "#198 \u{b7} code \u{b7} theme/ink.rs \u{b7} ensure_contrast at the consumer"
    );
    assert_eq!(
        &receipt.line[receipt.marks[0].0..receipt.marks[0].1],
        "contrast"
    );
    // The in-progress chunk is not a chapter, and the session matched nothing.
    assert!(
        results
            .rows
            .iter()
            .all(|hit| hit.kind != SearchKind::Session)
    );
}

#[test]
fn a_scope_reads_one_kind_with_a_deeper_cut_and_an_empty_query_finds_nothing() {
    let mut model = seeded();
    chord(&mut model);
    assert_eq!(Search::results(&model), SearchResults::default());
    typed(&mut model, "floor");
    key(&mut model, KeyCode::Tab, Modifiers::NONE);
    assert_eq!(
        model.search().map(Search::scope),
        Some(SearchScope::Reports)
    );
    let results = Search::results(&model);
    assert_eq!(results.total, 4, "{results:?}");
    assert_eq!(
        results.rows.len(),
        4,
        "a scoped kind shows past the everything cut"
    );
    key(&mut model, KeyCode::Tab, Modifiers::SHIFT);
    key(&mut model, KeyCode::Tab, Modifiers::SHIFT);
    assert_eq!(
        model.search().map(Search::scope),
        Some(SearchScope::Sessions)
    );
    assert_eq!(Search::results(&model).total, 0);
    assert_eq!(Search::results(&model).count_text(), "0 results");
}

#[test]
fn the_chord_raises_search_on_a_window_never_on_a_terminal_and_esc_puts_it_away() {
    let mut terminal = Model::new(Size::new(120, 40));
    chord(&mut terminal);
    assert!(terminal.search().is_none(), "a terminal holds no search");

    let mut model = native();
    key(&mut model, KeyCode::Char('f'), Modifiers::SUPER);
    assert!(model.search().is_none(), "⌘F alone is not the chord");
    chord(&mut model);
    assert!(model.search().is_some());
    typed(&mut model, "ab");
    key(&mut model, KeyCode::Backspace, Modifiers::NONE);
    assert_eq!(model.search().map(Search::query), Some("a"));
    // ⌘K is swallowed while search is up: one float at a time.
    key(&mut model, KeyCode::Char('k'), Modifiers::SUPER);
    assert!(model.palette().is_none());
    key(&mut model, KeyCode::Esc, Modifiers::NONE);
    assert!(model.search().is_none());
    chord(&mut model);
    chord(&mut model);
    assert!(model.search().is_none(), "the chord toggles");
}

#[test]
fn the_choice_walks_within_the_rows_and_snaps_back_when_the_query_changes() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "floor");
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    key(&mut model, KeyCode::Down, Modifiers::NONE);
    let rows = Search::results(&model).rows.len();
    assert_eq!(
        model.search().map(Search::chosen),
        Some(rows - 1),
        "clamped"
    );
    key(&mut model, KeyCode::Up, Modifiers::NONE);
    assert_eq!(model.search().map(Search::chosen), Some(rows - 2));
    typed(&mut model, "s");
    assert_eq!(model.search().map(Search::chosen), Some(0));
}

#[test]
fn enter_opens_a_chapter_through_the_open_chunk_fold() {
    let mut model = seeded();
    model.place = Place::Session;
    chord(&mut model);
    typed(&mut model, "contrast floor");
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert!(model.search().is_none());
    assert_eq!(
        model.place(),
        Some(Place::Report),
        "a shipped chunk opens as its chapter"
    );
    assert_eq!(
        model.roadmap().selected().map(|c| c.id.as_str()),
        Some("latte-floor")
    );
}

#[test]
fn enter_opens_a_step_on_the_trace_expanded_and_a_receipt_on_the_trace() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "reverse");
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert_eq!(model.place(), Some(Place::Trace));
    assert!(model.trace().is_expanded(387));

    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "ink.rs");
    let results = Search::results(&model);
    assert_eq!(results.rows.len(), 1);
    assert_eq!(results.rows[0].kind, SearchKind::Receipt);
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    assert_eq!(model.place(), Some(Place::Trace));
}

#[test]
fn enter_opens_a_session_on_the_launchers_row() {
    let mut model = seeded();
    chord(&mut model);
    // `shipped the palette` would also find every chapter with a `the`
    // — words are words, there is no stop list — so the session is
    // asked for by the word only it carries.
    typed(&mut model, "shipped");
    let results = Search::results(&model);
    assert_eq!(results.rows.len(), 1);
    assert_eq!(results.rows[0].kind, SearchKind::Session);
    assert_eq!(
        results.rows[0].line,
        "Aug 30 14:02 \u{b7} 12m \u{b7} shipped the palette"
    );
    key(&mut model, KeyCode::Enter, Modifiers::NONE);
    // The launcher's slot on a window is the Sky (`Place::of_tab`), the
    // row selected — exactly where the palette's Resume lands.
    assert_eq!(model.place(), Some(Place::of_tab(StageTab::Home)));
    assert_eq!(model.session_selected(), Some(0));
}

#[test]
fn a_click_on_a_chip_rescopes_a_click_on_a_row_opens_and_a_click_elsewhere_closes() {
    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "floor");
    click(&mut model, Some(Widget::search_scope(3)));
    assert_eq!(model.search().map(Search::scope), Some(SearchScope::Trace));
    assert_eq!(Search::results(&model).rows.len(), 1);
    click(&mut model, Some(Widget::search_row(0)));
    assert!(model.search().is_none());
    assert_eq!(model.place(), Some(Place::Trace));
    assert!(model.trace().is_expanded(387));

    let mut model = seeded();
    chord(&mut model);
    typed(&mut model, "floor");
    click(&mut model, None);
    assert!(
        model.search().is_none(),
        "the press is spent putting the float away"
    );
    assert_eq!(model.place(), Some(Place::Session), "nothing moved");
}
