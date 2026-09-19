use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
use magistr_cockpit::cockpit::{
    Copy, Model, Motion, Msg, Progress, Size, Spine, StageTab, StateWord, Surface,
    update as cockpit_update,
};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{Button, KeyCode, MouseKind};
use magistr_cockpit::theme::Severity;
use magistr_domain::policy::RiskTier;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::answer_field::AnswerField;
use crate::budget_meter::BudgetMeter;
use crate::card::Card;
use crate::chapter_title::ChapterTitle;
use crate::cmdk_row::CmdkRow;
use crate::comp_bad_day::BadDayComposition;
use crate::comp_band::BandComposition;
use crate::comp_birth::BirthComposition;
use crate::comp_brief::BriefComposition;
use crate::comp_composer::ComposerComposition;
use crate::comp_feed::FeedComposition;
use crate::comp_foot::FootComposition;
use crate::comp_galaxy::GalaxyComposition;
use crate::comp_header::HeaderComposition;
use crate::comp_moment::MomentComposition;
use crate::comp_palette::PaletteComposition;
use crate::comp_rail::RailComposition;
use crate::comp_report::ReportComposition;
use crate::comp_root::RootComposition;
use crate::comp_search::SearchComposition;
use crate::comp_settings::SettingsComposition;
use crate::comp_signals::SignalsComposition;
use crate::comp_sky::SkyComposition;
use crate::comp_term_dock::TermDockComposition;
use crate::comp_trace::TraceComposition;
use crate::component::Component;
use crate::composer::Composer;
use crate::crumb::Crumb;
use crate::crumb_stop::CrumbStop;
use crate::deliverable_row::DeliverableRow;
use crate::diff_block::DiffBlock;
use crate::diff_row::DiffRow;
use crate::edge::Edge;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::element_layout::ElementLayout;
use crate::element_state::ElementState;
use crate::finding_row::FindingRow;
use crate::fixture::Fixture;
use crate::icon::Icon;
use crate::identity_row::IdentityRow;
use crate::key_hint::KeyHint;
use crate::led::Led;
use crate::ledger::Ledger;
use crate::ledger_verbs::LedgerVerbs;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::match_line::MatchLine;
use crate::medium::Medium;
use crate::mode_badge::ModeBadge;
use crate::nav_chip::NavChip;
use crate::phase_rail::PhaseRail;
use crate::pick_chip::PickChip;
use crate::pill::DataPill;
use crate::progress_strip::ProgressStrip;
use crate::receipt_row::ReceiptRow;
use crate::scope_chip::ScopeChip;
use crate::search_field::SearchField;
use crate::search_row::SearchRow;
use crate::segments::Segments;
use crate::setting_row::SettingRow;
use crate::settings_nav::SettingsNav;
use crate::severity_cap::SeverityCap;
use crate::sink_section::SinkSection;
use crate::slot::Slot;
use crate::spine_ledger::SpineLedger;
use crate::star::Star;
use crate::stat::Stat;
use crate::status_row::StatusRow;
use crate::still_tile::StillTile;
use crate::switch::Switch;
use crate::term_pane::TermPane;
use crate::text::TextEngine;
use crate::theme_card::ThemeCard;
use crate::theme_preview::ThemePreview;
use crate::thin_bar::ThinBar;
use crate::voice::Voice;

/// The kitchen sink — the specimen sheet made executable (rule as a view): every a2/a3
/// component in every declared state and arrangement, on one scrollable page, live in
/// both media.
///
/// # One Msg loop, no second event system
///
/// The sink folds the cockpit's own [`Msg`] — the same messages the window's input
/// layer produces — through [`ElementState`] for hover/press/focus and through its own
/// activation step for what a completed click *does*: switches toggle, segments select,
/// the medium flips, the meter refills, the cards deal again. Every transient is a pure
/// function of the tick count, so reduced motion is structurally the end frame and a
/// golden needs no clock.
///
/// # The catalog is the roster
///
/// [`Sink::catalog`] answers the sectioned entries the view renders, each naming the
/// component modules it exhibits. The completeness test derives the component-module
/// set from `src/` and checks it against these names — a component absent from the sink
/// fails by name, which is what makes "not in the sink, not in a view" enforceable
/// rather than aspirational.
pub struct Sink {
    medium: Medium,
    reduced_motion: bool,
    ticks: u64,
    scroll: f32,
    state: ElementState,
    switch_a: bool,
    switch_b: bool,
    segment: usize,
    theme: usize,
    fill_from: Option<u64>,
    deal_from: Option<u64>,
    /// The last measured `(width, height)` of the content — a full
    /// layout pass, so it is kept until something that can change the
    /// page's extent happens. Hover, press, focus and scroll never do.
    extent: std::cell::Cell<Option<(f32, f32)>>,
    /// The built tree and its layout, reused for every hit test until
    /// the page itself changes — see [`Sink::fold`].
    scene: Option<SinkScene>,
    /// Bumped on every [`Sink::refresh`] — the renderer's staleness key.
    revision: u64,
    /// The models the composition exhibits map — real cockpit state,
    /// built once through the public `update` (a hand-assembled model
    /// would prove nothing about the mapping).
    views: Views,
}

/// The cockpit models the compositions section maps — one per
/// arrangement, all built through the public `update` and settled at
/// build time so the page is a still.
struct Views {
    /// View 05's session: chunk, spine, charter, budget, a turn.
    session: Model,
    /// The same session after its loop ended, in a window that starts work in place.
    idle: Model,
    /// A streaming turn — prose, list, fence.
    turn: Model,
    /// The same empty-prompt model with a steer typed into it.
    typed: Model,
    /// A window opened after hours away, its Brief's deal landed.
    briefed: Model,
    /// View 06's plan ask.
    asked: Model,
    /// A plain ask over view 05's room — the one a live session raises, no plan on it.
    asked_tool: Model,
    /// View 01's ⌘K palette: `thi` typed over the projects altitude.
    palette: Model,
    /// View 03's sky, native and idled to rest.
    resting: Model,
    /// View 08's ship moment in flight.
    shipped: Model,
    /// View 01's Galaxy: three projects, the window's own asking.
    galaxy: Model,
    /// View 11's Trace: 102 steps, eight pins, a kept branch, 388 opened.
    traced: Model,
    /// A source edit selected for inspection.
    changes: Model,
    /// View 12's Signals: one open, one researched, one promoted.
    signals: Model,
    /// View 13's search: `contrast floor` over the dimmed session.
    search: Model,
    /// View 10's settings: the Appearance pane over view 05's room.
    settings: Model,
    /// View 14's bad day: three faults over the faded Sky.
    bad_day: Model,
    /// A window mid-tool-call at the crest of its breath — the working card's halo as a
    /// still.
    working: Model,
    /// One card of every verb, every one open — the eight details.
    cards: Model,
    /// A run card open on its streaming terminal, the pane following.
    paned: Model,
    /// The same run's screen raised into the dock, the rise settled.
    docked: Model,
    /// The same run with the kill held past the dead zone: the head part-way red.
    killing: Model,
    /// A shipped chunk opened and its deliverables shelf landed: the Sky carries the
    /// star's card, and the same model's camera is at the Report (view 09).
    shelved: Model,
    /// F2's survey: `tidewatch` dropped, its report landed and every
    /// row risen under the sweep.
    surveyed: Model,
    /// F4's charting: the proposal landed, its stars born.
    charted: Model,
}

impl Views {
    fn new() -> Self {
        let size = Size::new(120, 40);
        let mut changes = Model::new(size)
            .with_surface(Surface::Native)
            .with_place(magistr_cockpit::cockpit::Place::Changes);
        cockpit_update(
            &mut changes,
            Msg::Workbench(magistr_cockpit::cockpit::WorkbenchEvent::Journal {
                entries: vec![magistr_cockpit::workbench::JournalEntry {
                    id: "1".into(),
                    path: "notes.txt".into(),
                    added: 1,
                    removed: 1,
                    parse_ok: true,
                    diff: "-before\n+after".into(),
                    ..magistr_cockpit::workbench::JournalEntry::default()
                }],
            }),
        );
        cockpit_update(
            &mut changes,
            Msg::Mouse {
                event: magistr_cockpit::input::MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(magistr_cockpit::cockpit::Widget::change_row(0)),
            },
        );
        let mut typed = Model::new(size);
        for key in "allow, but stop if the gates fail".chars() {
            cockpit_update(
                &mut typed,
                Msg::Key(magistr_cockpit::input::KeyEvent::new(KeyCode::Char(key))),
            );
        }
        let mut briefed = Model::new(size)
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap);
        cockpit_update(
            &mut briefed,
            Msg::Engine(EngineEvent::local(fixtures::frame(
                "roadmap",
                "chunk_completed",
                serde_json::json!({
                    "chunk": {"id": "e10-fleet", "name": "e10-fleet", "status": "done"}
                }),
            ))),
        );
        let mut briefed = briefed.with_away(std::time::Duration::from_secs(6 * 3600 + 12 * 60));
        // Land the deal, so the page is a still of the settled Brief.
        for _ in 0..(Motion::RISE_STAGGER * 4 + Motion::ARRIVAL_STEPS + 1) {
            cockpit_update(&mut briefed, Msg::Tick);
        }
        let mut resting =
            Fixture::with_pixels(Fixture::mock("03", size).expect("view 03 has a fixture"));
        resting.set_surface(Surface::Native);
        for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
            cockpit_update(&mut resting, Msg::Tick);
        }
        // The root composes a window: the session it exhibits declares
        // itself native, as the frame does before mapping the root.
        let mut session = Fixture::mock("05", size).expect("view 05 has a fixture");
        session.set_surface(Surface::Native);
        // Half a period from the foot the fixture settles on: the crest.
        let mut working = Fixture::working(size);
        for _ in 0..(Motion::GLOW_STEPS / 2) {
            cockpit_update(&mut working, Msg::Tick);
        }
        // A plain ask, folded through the one `update` a live session
        // reaches it by: a tool title, the three answers, no plan.
        let mut asked_tool = Fixture::mock("05", size).expect("view 05 has a fixture");
        asked_tool.set_surface(Surface::Native);
        cockpit_update(
            &mut asked_tool,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 9,
                title: "mcp__workbench-mcp__read".to_owned(),
                choices: vec![
                    PermissionChoice::new("allow", PermissionKind::AllowOnce),
                    PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
                    PermissionChoice::new("reject", PermissionKind::RejectOnce),
                ],
                edit: false,
                risk: RiskTier::Medium,
                plan: None,
            }),
        );
        Self {
            session,
            idle: Fixture::rail_idle(size),
            turn: Fixture::turn(size),
            typed,
            briefed,
            asked: Fixture::mock("06", size).expect("view 06 has a fixture"),
            asked_tool,
            palette: Fixture::mock("01", size).expect("view 01 has a fixture"),
            resting,
            shipped: Fixture::ship_08(size),
            galaxy: Fixture::galaxy(size),
            traced: Fixture::trace_11(size),
            changes,
            signals: Fixture::signals_12(size),
            search: Fixture::search_13(size),
            settings: Fixture::settings_10(size),
            bad_day: Fixture::bad_day_14(size),
            working,
            cards: Fixture::cards(Size::new(120, 400)),
            paned: Fixture::pane_following(size),
            docked: Fixture::pane_docked(size),
            killing: Fixture::pane_killing(size),
            shelved: Fixture::shelved(size),
            surveyed: Fixture::birth_f2(size),
            charted: {
                // The stars born: the page is a still of the drawn
                // proposal, not of its first frame.
                let mut charted = Fixture::birth_f4(size);
                for _ in 0..(Fixture::SETTLE_STEPS * 2) {
                    cockpit_update(&mut charted, Msg::Tick);
                }
                charted
            },
        }
    }
}

/// One laid-out page: what a pointer is hit-tested against, and what a
/// paint emits — built once per page change, not once per event.
struct SinkScene {
    viewport: (f32, f32),
    tree: Element,
    layout: ElementLayout,
}

impl Sink {
    // ── The stable identities interaction is keyed under ─────────────
    /// The medium segments — Night answers on this, Day on the next.
    pub const MEDIUM: ElementId = ElementId(1);
    /// The reduced-motion switch.
    pub const MOTION: ElementId = ElementId(5);
    /// The nav chip exhibit.
    pub const NAV_CHIP: ElementId = ElementId(10);
    /// The crumb's back stop.
    pub const CRUMB_BACK: ElementId = ElementId(11);
    /// The action row's five faces, primary first.
    pub const ACTIONS: ElementId = ElementId(12);
    /// The demo segments' first choice.
    pub const SEGMENTS: ElementId = ElementId(20);
    /// The first demo switch (starts on).
    pub const SWITCH_A: ElementId = ElementId(25);
    /// The second demo switch (starts off).
    pub const SWITCH_B: ElementId = ElementId(26);
    /// The sky theme card.
    pub const THEME_SKY: ElementId = ElementId(30);
    /// The follow-OS theme card.
    pub const THEME_OS: ElementId = ElementId(31);
    /// The composer exhibit.
    pub const COMPOSER: ElementId = ElementId(35);
    /// The search field exhibit.
    pub const SEARCH: ElementId = ElementId(36);
    /// The action that replays the meter's fill.
    pub const REFILL: ElementId = ElementId(37);
    /// The action that deals the rise cards again.
    pub const DEAL: ElementId = ElementId(38);
    /// The ⌘K result row exhibit.
    pub const CMDK: ElementId = ElementId(40);

    /// Ticks the meter's fill sweep takes.
    pub const FILL_STEPS: u64 = 12;
    /// Ticks between two dealt cards.
    pub const DEAL_STEP: u64 = 3;
    /// How many rise cards a deal reveals.
    const DEAL_COUNT: u64 = 3;
    /// Pixels one scroll notch moves the page.
    const NOTCH: f32 = 48.0;
    /// The page's own padding.
    const PAD: f32 = 28.0;
    /// The column card exhibits are laid on — wide enough that a rise
    /// card's severity cap keeps its phrase on one line, as the
    /// volume's own cards do.
    const CARD_W: f32 = 470.0;

    /// A sink at rest in `medium`: every transient settled on its end
    /// frame, nothing hovered, nothing pressed.
    #[must_use]
    pub fn new(medium: Medium) -> Self {
        Self {
            medium,
            reduced_motion: false,
            ticks: 0,
            scroll: 0.0,
            state: ElementState::default(),
            switch_a: true,
            switch_b: false,
            segment: 2,
            theme: 0,
            fill_from: None,
            deal_from: None,
            extent: std::cell::Cell::new(None),
            scene: None,
            revision: 0,
            views: Views::new(),
        }
    }

    /// The medium the sink currently paints in.
    #[must_use]
    pub fn medium(&self) -> Medium {
        self.medium
    }

    /// Whether a transient is mid-sweep — the window's cue to tick.
    /// Reduced motion answers `false` structurally: with every
    /// transient on its end frame there is nothing to sample.
    #[must_use]
    pub fn animating(&self) -> bool {
        if self.reduced_motion {
            return false;
        }
        let fill = self
            .fill_from
            .is_some_and(|from| self.ticks < from + Self::FILL_STEPS);
        let deal = self
            .deal_from
            .is_some_and(|from| self.ticks < from + Self::DEAL_STEP * (Self::DEAL_COUNT - 1));
        fill || deal
    }

    /// Fold one cockpit message. Answers whether anything changed —
    /// the caller's cue to paint.
    ///
    /// # A hover is an emit, never a layout
    ///
    /// The built tree does not depend on hover, press or focus — the
    /// emit path reads those off [`ElementState`] — so the tree and
    /// its layout are cached and a pointer move hit-tests against the
    /// cache. Only what actually reshapes the page rebuilds it: an
    /// activation, a scroll, a mid-transient tick, a new viewport.
    /// [`Sink::revision`] counts those rebuilds, which is how the
    /// window knows its scaled scene is stale.
    pub fn fold(&mut self, msg: &Msg, viewport: (f32, f32), text: &mut TextEngine) -> bool {
        // A tick needs no hit test, and the animation loop sends many:
        // it advances the clock without paying for a layout pass.
        if matches!(msg, Msg::Tick) {
            let moving = self.animating();
            self.ticks += 1;
            if moving {
                // A transient mid-sweep can change the page's extent
                // (an undealt card's ghost is shorter than the card).
                self.extent.set(None);
                self.refresh();
            }
            return moving;
        }
        // A scroll needs no hit test either — the page moves, nothing
        // is aimed at — and the clamp reads the cached extent.
        if let Msg::Mouse { event, .. } = msg {
            match event.kind {
                MouseKind::ScrollUp => return self.scroll_by(-Self::NOTCH, viewport, text),
                MouseKind::ScrollDown => return self.scroll_by(Self::NOTCH, viewport, text),
                _ => {}
            }
        }
        let scene = match self.scene.take() {
            Some(scene) if scene.viewport == viewport => scene,
            _ => {
                let tree = self.view(viewport, text);
                let Ok(layout) = ElementLayout::compute(&tree, viewport.0, viewport.1, text) else {
                    return false;
                };
                SinkScene {
                    viewport,
                    tree,
                    layout,
                }
            }
        };
        let was_pressed = self.state.pressed();
        let mut changed = self.state.fold(msg, &scene.tree, &scene.layout);
        let mut acted = false;
        match msg {
            // A click is a press released over the element it began
            // on — the button semantics every toolkit keeps, so a
            // drag off the control cancels it.
            Msg::Mouse { event, .. } if event.kind == MouseKind::Up(Button::Left) => {
                if let Some(id) = was_pressed
                    && self.state.hover() == Some(id)
                {
                    acted = self.activate(id);
                }
            }
            Msg::Key(key) if key.is_press() && key.code == KeyCode::Enter => {
                if let Some(id) = self.state.focus() {
                    acted = self.activate(id);
                }
            }
            _ => {}
        }
        if acted {
            // The activation changed what the page shows, so both
            // caches go: the extent may have moved and the tree did.
            self.extent.set(None);
            self.refresh();
            changed = true;
        } else {
            self.scene = Some(scene);
        }
        changed
    }

    /// How many times the page itself has been reshaped — the cache
    /// key a renderer holds beside its own scaled scene. Hover, press
    /// and focus never move it; activation, scroll and a mid-transient
    /// tick do.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Drop the cached scene: the next fold or paint lays out afresh.
    fn refresh(&mut self) {
        self.scene = None;
        self.revision += 1;
    }

    /// What a completed click (or Enter on the focused element) does.
    fn activate(&mut self, id: ElementId) -> bool {
        match id {
            ElementId(1) => self.medium = Medium::Night,
            ElementId(2) => self.medium = Medium::Day,
            Self::MOTION => self.reduced_motion = !self.reduced_motion,
            ElementId(n) if (20..23).contains(&n) => self.segment = (n - 20) as usize,
            Self::SWITCH_A => self.switch_a = !self.switch_a,
            Self::SWITCH_B => self.switch_b = !self.switch_b,
            Self::THEME_SKY => self.theme = 0,
            Self::THEME_OS => self.theme = 1,
            Self::REFILL => self.fill_from = Some(self.ticks),
            Self::DEAL => self.deal_from = Some(self.ticks),
            // The exhibits that answer hover, press and focus but
            // change no sink state — a chip is a chip.
            _ => return false,
        }
        true
    }

    /// Scroll by `px`, clamped to the page. Answers whether it moved.
    fn scroll_by(&mut self, px: f32, viewport: (f32, f32), text: &mut TextEngine) -> bool {
        let limit = (self.content_height(viewport.0, text) - viewport.1).max(0.0);
        let to = (self.scroll + px).clamp(0.0, limit);
        let moved = (to - self.scroll).abs() > f32::EPSILON;
        self.scroll = to;
        if moved {
            // The content is pinned at the scroll offset, so a moved
            // page is a different tree.
            self.refresh();
        }
        moved
    }

    /// The page under a `viewport`-sized window: the content pinned at
    /// the current scroll, the medium's ground behind it.
    pub fn view(&self, viewport: (f32, f32), text: &mut TextEngine) -> Element {
        let lex = Lexicon::of(self.medium);
        let height = self.content_height(viewport.0, text);
        let limit = (height - viewport.1).max(0.0);
        Element::surface().fill(lex.base).child(
            self.content()
                .pin_left(0.0)
                .pin_right(0.0)
                .pin_top(-self.scroll.min(limit))
                .height(height),
        )
    }

    /// The whole page at its natural extent — what the goldens are a
    /// picture of, and what [`Sink::view`] windows.
    #[must_use]
    pub fn content(&self) -> Element {
        let lex = Lexicon::of(self.medium);
        let mut page = Element::surface()
            .fill(lex.base)
            .pad(Self::PAD)
            .gap(22.0)
            .align_start()
            .child(self.header(&lex));
        for section in self.catalog() {
            let mut column = Element::surface()
                .gap(14.0)
                .align_start()
                .child(
                    Element::text(Voice::CAP, lex.ov0, section.title)
                        .claim(Component::Cap)
                        .nowrap(),
                )
                .child(
                    Element::surface()
                        .width(560.0)
                        .height(1.0)
                        .fill(lex.grounded(lex.hairc)),
                );
            for (modules, tree) in section.entries {
                column = column.child(
                    Element::surface()
                        .gap(6.0)
                        .align_start()
                        .child(
                            Element::text(Voice::CAP, lex.sb0, modules.join(" · "))
                                .claim(Component::Cap)
                                .nowrap(),
                        )
                        .child(tree),
                );
            }
            page = page.child(column);
        }
        page
    }

    /// How tall [`Sink::content`] lays out at `width` — a probe pass
    /// over the same tree, so the scroll clamp and the pinned height
    /// agree with what will actually paint.
    pub fn content_height(&self, width: f32, text: &mut TextEngine) -> f32 {
        const PROBE: f32 = 65536.0;
        if let Some((for_width, height)) = self.extent.get()
            && (for_width - width).abs() < f32::EPSILON
        {
            return height;
        }
        let content = self.content();
        let Ok(layout) = ElementLayout::compute(&content, width, PROBE, text) else {
            return 0.0;
        };
        let mut extent: f32 = 0.0;
        for index in 1..layout.len() {
            let [_, y, _, h] = layout.rect(index);
            extent = extent.max(y + h);
        }
        let height = (extent + Self::PAD).ceil();
        self.extent.set(Some((width, height)));
        height
    }

    /// The interaction state the emitted faces are read from.
    #[must_use]
    pub fn state(&self) -> &ElementState {
        &self.state
    }

    /// The sink's chrome: the title, the live flips (medium, reduced
    /// motion) and the focus hint — themselves catalog components, so
    /// the chrome exhibits nothing the roster does not know.
    fn header(&self, lex: &Lexicon) -> Element {
        let title = Voice {
            px: 16.0,
            weight: 600,
            ..Voice::SLOT
        };
        Element::surface()
            .gap(8.0)
            .align_start()
            .child(
                Element::text(title, lex.tx, "Kitchen sink")
                    .claim(Component::Title)
                    .nowrap(),
            )
            .child(
                Element::text(
                    Voice::CAP,
                    lex.ov0,
                    "every atom \u{b7} every state \u{b7} both media",
                )
                .claim(Component::Cap)
                .nowrap(),
            )
            .child(
                Element::row()
                    .gap(16.0)
                    .center()
                    .child(Segments::el(
                        &["Night", "Day"],
                        match self.medium {
                            Medium::Night => 0,
                            Medium::Day => 1,
                        },
                        Self::MEDIUM,
                        lex,
                    ))
                    .child(
                        Element::row()
                            .gap(7.0)
                            .center()
                            .child(Switch::el(self.reduced_motion, Self::MOTION, lex))
                            .child(
                                Element::text(Voice::CAP, lex.sb0, "reduced motion")
                                    .claim(Component::Cap)
                                    .nowrap(),
                            ),
                    )
                    .child(KeyHint::el("\u{21e5}", "Focus", self.medium, lex.base, lex)),
            )
    }

    /// The sectioned catalog — every a2/a3 component, every declared
    /// state, in the specimen sheet's own order.
    #[must_use]
    pub fn catalog(&self) -> Vec<SinkSection> {
        let lex = Lexicon::of(self.medium);
        vec![
            self.controls(&lex),
            copy_law(&lex, self.medium),
            self.surfaces(&lex),
            diffs(&lex),
            command(&lex),
            constellation(&lex),
            self.compositions(),
        ]
    }

    /// The view-scale compositions — each entry a real cockpit model mapped through its
    /// composition module, so the sink shows the arrangements before any view ships
    /// them.
    fn compositions(&self) -> SinkSection {
        let medium = self.medium;
        let views = &self.views;
        let cell = (9.0, 20.0);
        let entries: Vec<(&'static [&'static str], Element)> = vec![
            (
                &["comp_header"],
                HeaderComposition::el(&views.session, medium),
            ),
            // The identity band — the session's cap.
            (&["comp_band"], BandComposition::el(&views.session, medium)),
            (
                &["comp_rail"],
                RailComposition::el(&views.session, medium, cell.0),
            ),
            // The idle rail: no loop live, so the ledger head wears the two starts where
            // the running rail above wears the loop's verbs.
            (
                &["work_controls"],
                RailComposition::el(&views.idle, medium, cell.0),
            ),
            (
                &["comp_composer"],
                Element::surface()
                    .width(470.0)
                    .child(ComposerComposition::el(&views.typed, medium)),
            ),
            (
                &["comp_feed"],
                Element::surface()
                    .width(560.0)
                    .align_start()
                    .child(FeedComposition::el(&views.turn, medium, cell)),
            ),
            // The working breath at its crest: the running tool card under its halo
            // (night) or ring (day), the thought above it at rest since a card is
            // running.
            (
                &["breath"],
                Element::surface()
                    .width(560.0)
                    .align_start()
                    .child(FeedComposition::el(&views.working, medium, cell)),
            ),
            // Every card open: the eight details, each composed from its record — the
            // run's pane and cut, the read's range, the search's marked hits, the
            // edit's diff and strip, the fetch, the thought, the MCP tool's input and
            // output, the failed check's own rows.
            (
                &["comp_feed", "tool_detail"],
                Element::surface()
                    .width(560.0)
                    .align_start()
                    .child(FeedComposition::el(&views.cards, medium, cell)),
            ),
            // The terminal pane: the ANSI swatch row — the sixteen grounds, the sixteen
            // inks, every attribute — over a run card open on its streaming screen, the
            // cells in the program's own colours through the medium's sixteen, the
            // cursor standing on the tail.
            (
                &["term_pane", "pane_tile"],
                Element::surface()
                    .width(620.0)
                    .align_start()
                    .gap(12.0)
                    .child(TermPane::swatch(
                        Lexicon::of(medium).mantle,
                        &Lexicon::of(medium),
                    ))
                    .child(FeedComposition::el(&views.paned, medium, cell)),
            ),
            // The held kill: the same run's card with `k` held past the dead zone — the
            // head filling red from the left, the run still streaming under it.
            (
                &["comp_feed", "tool_detail"],
                Element::surface()
                    .width(620.0)
                    .align_start()
                    .child(FeedComposition::el(&views.killing, medium, cell)),
            ),
            // The terminal dock: the same run's screen risen from the composer's edge —
            // the grip's hairline and handle, the command's cap, the chords, the window
            // at a third of the stage — as it stands between the feed and the composer.
            (
                &["comp_term_dock"],
                Element::surface().width(620.0).align_start().child(
                    TermDockComposition::el(&views.docked, medium)
                        .expect("the sink's dock is raised"),
                ),
            ),
            (
                &["comp_sky"],
                Element::surface()
                    .width(560.0)
                    .height(200.0)
                    .child(SkyComposition::el(&views.resting, medium)),
            ),
            (
                &["comp_sky"],
                Element::surface()
                    .width(560.0)
                    .height(120.0)
                    .child(SkyComposition::el(&views.shipped, medium)),
            ),
            // A shipped star's card with its deliverables shelf: the gallery of the
            // gates' goldens, the receipts, [Export] — the whole Sky, so the card
            // floats where it does on the window.
            (
                &["comp_shelf"],
                Element::surface()
                    .width(700.0)
                    .height(460.0)
                    .child(SkyComposition::el(&views.shelved, medium)),
            ),
            // The Report (view 09): the header's two verbs, then the
            // chapter beside the deliverables rail, at the capture's
            // own width so the measure and the rail sit as they do.
            (
                &["comp_report"],
                Element::surface()
                    .width(1100.0)
                    .align_start()
                    .child(ReportComposition::chips(medium))
                    .child(
                        Element::surface()
                            .width(1100.0)
                            .height(420.0)
                            .child(ReportComposition::el(&views.shelved, medium)),
                    ),
            ),
            // Birth: the survey's columns and meter over the field the sweep is drawn
            // into, then the charting's list beside its field, each with its header
            // chips, at the capture's own width.
            (
                &["comp_birth"],
                Element::surface()
                    .width(1100.0)
                    .align_start()
                    .child(BirthComposition::chips(&views.surveyed, medium))
                    .child(Element::surface().width(1100.0).height(360.0).child(
                        BirthComposition::el(&views.surveyed, medium, (1100.0, 360.0), cell),
                    )),
            ),
            (
                &["comp_birth"],
                Element::surface()
                    .width(1100.0)
                    .align_start()
                    .child(BirthComposition::chips(&views.charted, medium))
                    .child(Element::surface().width(1100.0).height(360.0).child(
                        BirthComposition::el(&views.charted, medium, (1100.0, 360.0), cell),
                    )),
            ),
            (
                &["comp_brief"],
                BriefComposition::el(&views.briefed, medium).expect("the briefed model has news"),
            ),
            // The bad day's cards and murmur foot, exhibited inside a
            // field-sized box: the composition pins itself over the
            // field it is given.
            (
                &["comp_bad_day"],
                Element::surface().width(900.0).height(520.0).child(
                    BadDayComposition::el(&views.bad_day, medium, 520.0)
                        .expect("the bad-day model holds faults"),
                ),
            ),
            (
                &["comp_moment"],
                MomentComposition::el(&views.asked, medium).expect("the asked model has a moment"),
            ),
            (
                &["comp_moment"],
                MomentComposition::el(&views.asked_tool, medium)
                    .expect("a plain ask on a window at the Session is a card"),
            ),
            (
                // The float is exhibited inside a sized box: the
                // composition pins itself over its container, so the
                // box is the window it seats in.
                &["comp_palette"],
                Element::surface().width(560.0).height(250.0).child(
                    PaletteComposition::el(&views.palette, medium)
                        .expect("the palette model holds an open palette"),
                ),
            ),
            (
                &["comp_foot"],
                Element::surface()
                    .width(560.0)
                    .child(FootComposition::el(&views.session, medium)),
            ),
            // The Galaxy's three pieces: the header chips, the field
            // with its labels and verb (the stars are the drawing's,
            // put down by the frame), and the foot.
            (
                &["comp_galaxy"],
                Element::surface()
                    .width(900.0)
                    .align_start()
                    .child(GalaxyComposition::chips(&views.galaxy, medium))
                    .child(
                        Element::surface()
                            .width(900.0)
                            .height(520.0)
                            .child(GalaxyComposition::el(&views.galaxy, medium, (900.0, 520.0))),
                    )
                    .child(GalaxyComposition::foot(&views.galaxy, medium)),
            ),
            // The Trace view's two pieces: the header chips and the body
            // — the step feed beside the BRANCHES rail (the graph is the
            // drawing's, put down by the frame into its reservation).
            (
                &["comp_trace"],
                Element::surface()
                    .width(900.0)
                    .align_start()
                    .child(TraceComposition::chips(&views.traced, medium))
                    .child(
                        Element::surface()
                            .width(900.0)
                            .height(340.0)
                            .child(TraceComposition::el(&views.traced, medium)),
                    ),
            ),
            // The Signals view's two pieces: the header chip and the
            // inbox — the ages read against the fixtures' pinned now.
            (
                &["comp_signals"],
                Element::surface()
                    .width(700.0)
                    .align_start()
                    .child(SignalsComposition::chips(&views.signals, medium))
                    .child(
                        Element::surface().width(700.0).height(420.0).child(
                            SignalsComposition::el(
                                &views.signals,
                                medium,
                                &magistr_cockpit::cockpit::FrameClock::new(
                                    0,
                                    std::time::Duration::ZERO,
                                )
                                .with_epoch_secs(Fixture::NOW_SECS),
                            ),
                        ),
                    ),
            ),
            // The ⌘⇧F float, exhibited inside a sized box the way the
            // palette is: the composition pins itself over its
            // container.
            (
                &["comp_search"],
                Element::surface().width(640.0).height(420.0).child(
                    SearchComposition::el(&views.search, medium)
                        .expect("the search model holds an open search"),
                ),
            ),
            // The ⌘, surface at the capture's own measure: a real
            // surface, exhibited as the window it takes.
            (
                &["comp_settings"],
                Element::surface().width(1100.0).height(640.0).child(
                    SettingsComposition::el(&views.settings, medium)
                        .expect("the settings model holds an open surface"),
                ),
            ),
            (
                &["comp_roadmap", "comp_roadmap_requests"],
                Element::surface().width(620.0).height(520.0).child(
                    crate::comp_roadmap::RoadmapComposition::el(
                        &crate::fixture::Fixture::roadmap_request(
                            magistr_cockpit::cockpit::Size::new(110, 32),
                            magistr_cockpit::cockpit::RequestPhase::Complete,
                        ),
                        medium,
                        cell,
                        (620.0, 520.0),
                    ),
                ),
            ),
            // The whole window as one tree — the a6 composition root,
            // exhibited at a small viewport so the sheet stays a sheet.
            (
                &[
                    "comp_root",
                    "comp_workspace",
                    "comp_workspace_details",
                    "comp_workspace_reasoning",
                ],
                Element::surface().width(900.0).height(520.0).child(
                    RootComposition::el(
                        &views.session,
                        medium,
                        cell,
                        (900.0, 520.0),
                        &magistr_cockpit::cockpit::FrameClock::new(0, std::time::Duration::ZERO),
                    )
                    .grow(1.0),
                ),
            ),
            (
                &["comp_changes"],
                Element::surface().width(900.0).height(400.0).child(
                    crate::comp_changes::ChangesComposition::el(
                        &views.changes,
                        medium,
                        cell,
                        (900.0, 400.0),
                    ),
                ),
            ),
        ];
        SinkSection {
            title: "compositions \u{b7} view scale",
            entries,
        }
    }

    /// the control atoms.
    fn controls(&self, lex: &Lexicon) -> SinkSection {
        let entries: Vec<(&'static [&'static str], Element)> = vec![
            (
                &["pill"],
                Element::row()
                    .gap(10.0)
                    .center()
                    .child(DataPill::live("Executing \u{b7} 04:12", lex))
                    .child(DataPill::ok("gates \u{2713}\u{2713}\u{2713}", lex))
                    .child(DataPill::num("1.84 / 12.00", lex)),
            ),
            (
                &["nav_chip"],
                NavChip::el("gui2-2a \u{b7} execute", Self::NAV_CHIP, lex),
            ),
            (
                &["crumb", "crumb_stop"],
                Crumb::el(
                    &[
                        CrumbStop::back(
                            "projects",
                            Some(Mark::Icon(Icon::Projects)),
                            Self::CRUMB_BACK,
                        ),
                        CrumbStop::here("magistr", Some(Mark::Sigil("magistr"))),
                    ],
                    lex,
                ),
            ),
            (
                &["action", "action_row"],
                ActionRow::new(
                    Some(Action::new("Approve & merge", Self::ACTIONS).cap("A")),
                    vec![
                        Action::new("Request changes\u{2026}", ElementId(13)),
                        Action::new("Deny", ElementId(14)).cap("D").negative(),
                        Action::new("Promote", ElementId(15)).accent(),
                        Action::new("Later", ElementId(16)).disabled(),
                    ],
                )
                .gap(8.0)
                .el(self.medium),
            ),
            (
                &["scope_chip"],
                Element::row()
                    .gap(6.0)
                    .center()
                    .child(ScopeChip::el(
                        "everything",
                        true,
                        lex.base,
                        ElementId(17),
                        lex,
                    ))
                    .child(ScopeChip::el(
                        "reports",
                        false,
                        lex.base,
                        ElementId(18),
                        lex,
                    ))
                    .child(ScopeChip::el("trace", false, lex.base, ElementId(19), lex)),
            ),
            (
                &["segments"],
                Segments::el(
                    &["The sky", "The chart", "Follow OS"],
                    self.segment,
                    Self::SEGMENTS,
                    lex,
                ),
            ),
            (
                &["switch"],
                Element::row()
                    .gap(12.0)
                    .center()
                    .child(Switch::el(self.switch_a, Self::SWITCH_A, lex))
                    .child(Switch::el(self.switch_b, Self::SWITCH_B, lex)),
            ),
            (
                &["theme_card"],
                Element::row()
                    .gap(12.0)
                    .child(ThemeCard::el(
                        "The sky",
                        self.theme == 0,
                        ThemeCard::sky_preview(lex),
                        Self::THEME_SKY,
                        lex,
                    ))
                    .child(ThemeCard::el(
                        "Follow OS",
                        self.theme == 1,
                        ThemePreview::split(ThemeCard::PREVIEW_W, ThemeCard::PREVIEW_H),
                        Self::THEME_OS,
                        lex,
                    )),
            ),
            (
                &["slot"],
                Element::surface()
                    .gap(12.0)
                    .align_start()
                    .child(
                        Element::surface()
                            .width(Self::CARD_W)
                            .child(Slot::el("Drop a folder here", lex)),
                    )
                    .child(
                        Element::surface()
                            .width(Self::CARD_W)
                            .child(Slot::empty_state(
                                Icon::Folder,
                                "Drop a folder here",
                                "Or click",
                                lex,
                            )),
                    ),
            ),
            // Birth's picks (F3): the volume's `.chip7.ok` worn as a
            // multi-select — two picked, two offered.
            (
                &["pick_chip"],
                Element::row()
                    .gap(8.0)
                    .center()
                    .child(PickChip::el(
                        "prediction accuracy",
                        true,
                        lex.base,
                        ElementId(45),
                        lex,
                    ))
                    .child(PickChip::el(
                        "the public API",
                        true,
                        lex.base,
                        ElementId(46),
                        lex,
                    ))
                    .child(PickChip::el(
                        "offline operation",
                        false,
                        lex.base,
                        ElementId(47),
                        lex,
                    )),
            ),
            // The own-answer field (F3), empty and typed.
            (
                &["answer_field"],
                Element::surface()
                    .width(420.0)
                    .gap(10.0)
                    .child(AnswerField::el(
                        "",
                        "Type your own answer",
                        ElementId(48),
                        lex,
                    ))
                    .child(AnswerField::el(
                        "the harbor feed",
                        "Type your own answer",
                        ElementId(49),
                        lex,
                    )),
            ),
            (&["budget_meter"], self.meter_exhibit(lex)),
            (
                &["composer"],
                Element::surface().width(420.0).child(Composer::el(
                    "\u{203a}",
                    "allow, but stop if\u{2026}",
                    Self::COMPOSER,
                    lex,
                )),
            ),
            (
                &["search_field"],
                Element::surface().width(420.0).child(SearchField::el(
                    "\u{2318}\u{21e7}F",
                    "contrast floor",
                    "6 results",
                    Self::SEARCH,
                    lex,
                )),
            ),
        ];
        SinkSection {
            title: "\u{a7}5 \u{b7} controls",
            entries,
        }
    }

    /// The meter with its refill trigger — the fill a pure function of
    /// the ticks since the click. One builder feeds both the catalog
    /// and the transient's filmstrip, so the strip pictures the page.
    fn meter_exhibit(&self, lex: &Lexicon) -> Element {
        Element::row()
            .gap(14.0)
            .align_end()
            .child(Element::surface().width(220.0).child(BudgetMeter::el(
                "Budget",
                "7.44 / 12.00",
                self.fill_progress() * 0.62,
                Some(0.83),
                Some("stops at 83%"),
                Some(BudgetMeter::RAISE),
                220.0,
                lex,
            )))
            .child(ActionRow::new(None, vec![Action::new("Refill", Self::REFILL)]).el(self.medium))
    }

    /// The rise cards as the deal has revealed them — an undealt card
    /// holds its silhouette. Feeds the catalog and the deal filmstrip.
    fn dealt_cards(&self, lex: &Lexicon) -> Element {
        let cards = [
            (
                Severity::Alert,
                Icon::Alert,
                "Stuck \u{b7} 4 h",
                "e6b requires a decision: OTLP endpoint scope.",
                Action::new("Answer", ElementId(90)).accent(),
            ),
            (
                Severity::Attention,
                Icon::Report,
                "Review",
                "theme-sweep is ready for review. 3 files.",
                Action::new("Review", ElementId(91)),
            ),
            (
                Severity::Good,
                Icon::Done,
                "Done \u{b7} 2",
                "e10-fleet, tui2-sheet-overflow. Merged.",
                Action::new("View reports", ElementId(92)),
            ),
        ];
        let mut dealt = Element::surface().gap(10.0).width(Self::CARD_W);
        for (index, (severity, icon, cap, body, action)) in cards.into_iter().enumerate() {
            dealt = dealt.child(if self.dealt(index as u64) {
                rise_card(severity, icon, cap, body, action, self.medium, lex)
            } else {
                ghost_card(lex)
            });
        }
        dealt
    }

    /// The surfaces — the elevation-scale cards, with the rise cards
    /// dealt on demand.
    fn surfaces(&self, lex: &Lexicon) -> SinkSection {
        let entries: Vec<(&'static [&'static str], Element)> = vec![
            (
                &["card", "severity_cap"],
                Element::surface()
                    .gap(10.0)
                    .align_start()
                    .child(
                        ActionRow::new(None, vec![Action::new("Deal again", Self::DEAL)])
                            .el(self.medium),
                    )
                    .child(self.dealt_cards(lex)),
            ),
            (
                &["card"],
                Element::surface()
                    .width(Self::CARD_W)
                    .child(toast_card(self.medium, lex)),
            ),
            (
                &["card"],
                Element::surface().width(Self::CARD_W).child(pin_card(lex)),
            ),
            (
                &["card"],
                Element::surface()
                    .width(Self::CARD_W)
                    .child(overlay_card(self.medium, lex)),
            ),
            // The Report's shelf pieces (view 09): two of a shipped
            // chunk's stills as tiles, and the recording and filmstrips
            // as deliverable rows.
            (
                &["still_tile"],
                Element::row()
                    .gap(8.0)
                    .child(StillTile::el(
                        &self.views.shelved.shelf().expect("the shelf landed").stills[0],
                        139.0,
                        lex,
                    ))
                    .child(StillTile::el(
                        &self.views.shelved.shelf().expect("the shelf landed").stills[1],
                        139.0,
                        lex,
                    )),
            ),
            (
                &["deliverable_row"],
                Element::surface()
                    .width(286.0)
                    .gap(8.0)
                    .child(DeliverableRow::el(
                        Icon::Recording,
                        lex.mauve,
                        true,
                        "Session recording",
                        "12 min",
                        lex,
                    ))
                    .child(DeliverableRow::el(
                        Icon::Filmstrip,
                        lex.teal,
                        false,
                        "Motion filmstrips",
                        "3 strips",
                        lex,
                    )),
            ),
            // The display voice's one home beside the engraved data
            // voice it sits over in a chapter: the serif title, then a
            // mono meta line whose zeros wear the slash.
            (
                &["chapter_title"],
                Element::surface()
                    .width(Self::CARD_W)
                    .gap(6.0)
                    .child(ChapterTitle::el("The design language, decided live", lex))
                    .child(
                        Element::text(
                            Voice::LEDGER_META,
                            lex.ov0,
                            "cp-6 \u{b7} 10.00 USD \u{b7} 09:14 \u{b7} 0 blocking",
                        )
                        .claim(Component::Cap)
                        .nowrap(),
                    ),
            ),
        ];
        SinkSection {
            title: "surfaces \u{b7} the elevation scale",
            entries,
        }
    }

    /// Whether rise card `index` has been dealt yet.
    fn dealt(&self, index: u64) -> bool {
        let Some(from) = self.deal_from else {
            return true;
        };
        if self.reduced_motion {
            return true;
        }
        index <= self.ticks.saturating_sub(from) / Self::DEAL_STEP
    }

    /// Where the meter's fill sweep stands, 0..=1 — smoothstepped, and
    /// structurally 1 under reduced motion or at rest.
    fn fill_progress(&self) -> f32 {
        let Some(from) = self.fill_from else {
            return 1.0;
        };
        if self.reduced_motion {
            return 1.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let t = (self.ticks.saturating_sub(from) as f32 / Self::FILL_STEPS as f32).min(1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// the copy-law set: no naked text anywhere on a view, the one state vocabulary, the
/// units spelled once and a scope named by its identity row.
fn copy_law(lex: &Lexicon, medium: Medium) -> SinkSection {
    // The seven state words, each on the chip the sessions strip wears
    // it on — the whole vocabulary, so a word outside it has nowhere
    // on this sheet to stand.
    let mut states = Element::row().gap(8.0).center();
    for state in StateWord::ALL {
        states = states.child(DataPill::quiet(state.word(), lex));
    }
    let usd = Copy::usd(207);
    let pair = Copy::pair(184, 1200);
    let clock = Copy::clock(4 * 60 + 12);
    let minutes = Copy::duration(12 * 60);
    let hours = Copy::duration(4 * 3600);
    let days = Copy::duration(2 * 86_400);
    let entries: Vec<(&'static [&'static str], Element)> = vec![
        (
            &["identity_row"],
            IdentityRow::el(
                Some(Mark::Sigil("magistr")),
                "magistr",
                Some("~/Code/magistr"),
                lex,
            ),
        ),
        (&["pill"], states),
        (
            &["ledger"],
            Element::surface().width(Sink::CARD_W).child(Ledger::el(
                "units",
                &[
                    ("money", usd.as_str(), lex.sb1),
                    ("meter", pair.as_str(), lex.sb1),
                    ("clock", clock.as_str(), lex.sb1),
                    ("duration", minutes.as_str(), lex.sb1),
                    ("duration", hours.as_str(), lex.sb1),
                    ("duration", days.as_str(), lex.sb1),
                ],
                lex,
            )),
        ),
        (
            &["led"],
            Element::row()
                .gap(10.0)
                .center()
                .child(Led::el(true, lex))
                .child(Led::el(false, lex)),
        ),
        (
            &["mode_badge"],
            Element::row()
                .gap(10.0)
                .center()
                .child(ModeBadge::ask(lex.base, lex))
                .child(ModeBadge::auto(lex.base, lex))
                .child(ModeBadge::off(lex.base, lex)),
        ),
        (
            &["key_hint"],
            KeyHint::el("\u{23ce}", "Open", medium, lex.base, lex),
        ),
        // The ledger head's verbs: three pairs a size down, the first reading Resume
        // while the loop holds a boundary.
        (
            &["ledger_verbs"],
            Element::surface()
                .gap(8.0)
                .align_start()
                .child(LedgerVerbs::el(false, medium, lex.base, lex))
                .child(LedgerVerbs::el(true, medium, lex.base, lex)),
        ),
        // A survey finding (F2): the teal key and the fact beside it.
        (
            &["finding_row"],
            FindingRow::el(
                Icon::Stack,
                "stack",
                "Rust \u{b7} edition 2024 \u{b7} cargo workspace, 4 crates",
                330.0,
                lex,
            ),
        ),
        (&["stat"], Stat::el("211", "306", lex)),
        (
            &["phase_rail"],
            PhaseRail::el(&["research", "plan", "execute", "verify"], 2, lex),
        ),
        (&["thin_bar"], ThinBar::el(0.15, 150.0, lex)),
        (
            &["progress_strip"],
            ProgressStrip::el(Progress::Running(70), lex).expect("a report draws a strip"),
        ),
        (
            &["search_row", "match_line"],
            Element::surface()
                .width(Sink::CARD_W)
                .child(SearchRow::el(
                    Some("Every light palette clears its floor"),
                    MatchLine::el(
                        "\u{2026}nudged until it clears the contrast floor the validator \
                         declares\u{2026} \u{b7} Aug 29",
                        &[(30, 44)],
                        Voice::LEDGER_META,
                        lex.ov0,
                        lex,
                    ),
                    true,
                    7.0,
                    lex.base,
                    ElementId(20),
                    lex,
                ))
                .child(Element::surface().height(6.0))
                .child(SearchRow::el(
                    None,
                    MatchLine::el(
                        "#198 \u{b7} edit \u{b7} theme/ink.rs \u{b7} ensure_contrast at the consumer",
                        &[(38, 46)],
                        Voice::CHIP,
                        lex.sb0,
                        lex,
                    ),
                    false,
                    10.0,
                    lex.base,
                    ElementId(21),
                    lex,
                )),
        ),
        (
            &["status_row"],
            Element::surface()
                .width(Sink::CARD_W)
                .child(StatusRow::el(
                    lex.mauve,
                    "gui2-2a",
                    ("Executing", None),
                    "04:12",
                    true,
                    lex.base,
                    lex,
                ))
                .child(StatusRow::el(
                    lex.red,
                    "e6b",
                    ("Blocked", Some(lex.red)),
                    "Requires agent",
                    false,
                    lex.base,
                    lex,
                )),
        ),
        (
            &["receipt_row", "mode_badge"],
            Element::surface()
                .width(Sink::CARD_W)
                .child(ReceiptRow::value(
                    (Icon::Commit, "commit"),
                    "a3f9c21",
                    Some("View"),
                    true,
                    lex.base,
                    lex,
                ))
                .child(ReceiptRow::leds(
                    (Icon::Gate, "gates"),
                    &[true, true, false],
                    Some("Log"),
                    true,
                    lex.base,
                    lex,
                ))
                .child(ReceiptRow::rule(
                    "git push",
                    ModeBadge::off(lex.base, lex),
                    false,
                    lex.base,
                    lex,
                )),
        ),
        (
            &["severity_cap"],
            Element::row()
                .gap(12.0)
                .center()
                .child(SeverityCap::el(
                    Severity::Alert,
                    Some(Icon::Alert),
                    "Blocking",
                    lex,
                ))
                .child(SeverityCap::el(
                    Severity::Attention,
                    None,
                    "plan \u{b7} two-phase",
                    lex,
                ))
                .child(SeverityCap::el(Severity::Good, None, "shipped", lex))
                .child(SeverityCap::el(
                    Severity::Quiet,
                    None,
                    "Signal \u{b7} 2 d",
                    lex,
                ))
                .child(SeverityCap::tinted(
                    lex.mauve,
                    None,
                    "think \u{b7} 388 \u{b7} pinned",
                )),
        ),
    ];
    SinkSection {
        title: "\u{a7}6.3 \u{b7} the copy law",
        entries,
    }
}

/// The diff and ledger vocabulary.
fn diffs(lex: &Lexicon) -> SinkSection {
    let entries: Vec<(&'static [&'static str], Element)> = vec![
        (
            &["diff_row"],
            Element::surface()
                .width(Sink::CARD_W)
                .child(DiffRow::del(
                    "- pub fn ink(field: PaletteField) -> Color {",
                    lex.base,
                    lex,
                ))
                .child(DiffRow::add(
                    "+ pub fn ink(field: PaletteField, palette: &Palette)",
                    lex.base,
                    lex,
                ))
                .child(DiffRow::ctx(
                    "  receipt #241 \u{b7} rollback available",
                    lex,
                )),
        ),
        (
            &["diff_block"],
            Element::surface().width(Sink::CARD_W).child(DiffBlock::el(
                "diff \u{b7} theme/ink.rs",
                vec![
                    DiffRow::del("- pub fn ink(field) -> Color {", lex.base, lex),
                    DiffRow::add("+ pub fn ink(field, palette: &Palette)", lex.base, lex),
                ],
                lex.base,
                lex,
            )),
        ),
        (
            &["ledger"],
            Element::surface().width(300.0).child(Ledger::el(
                "receipts",
                &[
                    ("commit", "a3f9c21", lex.sb1),
                    ("gates", "\u{2713}\u{2713}\u{2713}", lex.green),
                    ("cost", "2.07 USD", lex.sb0),
                ],
                lex,
            )),
        ),
        (
            &["spine_ledger"],
            Element::surface().width(300.0).child(SpineLedger::el(
                &Spine::default(),
                3,
                None,
                None,
                lex,
            )),
        ),
    ];
    SinkSection {
        title: "diff \u{b7} ledger",
        entries,
    }
}

/// The command palette row and the settings rail.
fn command(lex: &Lexicon) -> SinkSection {
    let body = Voice {
        px: 12.0,
        ..Voice::SLOT
    };
    let entries: Vec<(&'static [&'static str], Element)> = vec![
        (
            &["cmdk_row"],
            Element::surface().width(Sink::CARD_W).child(CmdkRow::el(
                vec![
                    Element::row()
                        .center()
                        .child(
                            Element::text(body, lex.tx, "Open\u{a0}")
                                .claim(Component::CommandRow)
                                .nowrap(),
                        )
                        .child(Element::mark(
                            Mark::Sigil("think-and-ship"),
                            14,
                            Lexicon::ink_bytes(lex.tx),
                        ))
                        .child(
                            Element::text(
                                Voice {
                                    px: 12.0,
                                    weight: 700,
                                    ..Voice::SLOT
                                },
                                lex.tx,
                                "think-and-ship",
                            )
                            .claim(Component::CommandRow)
                            .nowrap(),
                        ),
                    Element::text(Voice::LEDGER_META, lex.peach, "1 action required")
                        .claim(Component::CommandRow)
                        .nowrap(),
                ],
                true,
                lex.base,
                Sink::CMDK,
                lex,
            )),
        ),
        (
            &["settings_nav"],
            Element::surface()
                .width(220.0)
                .child(SettingsNav::el(
                    Icon::Image,
                    "Appearance",
                    true,
                    lex.base,
                    ElementId(41),
                    lex,
                ))
                .child(SettingsNav::el(
                    Icon::Session,
                    "Motion",
                    false,
                    lex.base,
                    ElementId(42),
                    lex,
                ))
                .child(SettingsNav::tagged(
                    Icon::Checkpoint,
                    "Policy",
                    false,
                    Some("read-only"),
                    lex.base,
                    ElementId(43),
                    lex,
                )),
        ),
        (
            &["theme_preview"],
            Element::row()
                .gap(16.0)
                .child(
                    ThemePreview::sky(ThemePreview::DRAWING.0, ThemePreview::DRAWING.1)
                        .width(ThemePreview::DRAWING.0)
                        .height(ThemePreview::DRAWING.1)
                        .radius(9.0),
                )
                .child(
                    ThemePreview::chart(ThemePreview::DRAWING.0, ThemePreview::DRAWING.1)
                        .width(ThemePreview::DRAWING.0)
                        .height(ThemePreview::DRAWING.1)
                        .radius(9.0),
                )
                .child(
                    ThemePreview::split(ThemePreview::DRAWING.0, ThemePreview::DRAWING.1)
                        .width(ThemePreview::DRAWING.0)
                        .height(ThemePreview::DRAWING.1)
                        .radius(9.0),
                ),
        ),
        (
            &["setting_row"],
            Element::surface()
                .width(480.0)
                .child(SettingRow::el(
                    "Show activity heatmap",
                    "On the project view",
                    Switch::el(true, ElementId(44), lex),
                    true,
                    lex.base,
                    lex,
                ))
                .child(SettingRow::el(
                    "Density",
                    "Row height and padding",
                    Segments::el(&["Comfortable", "Compact"], 0, ElementId(45), lex),
                    false,
                    lex.base,
                    lex,
                )),
        ),
    ];
    SinkSection {
        title: "command \u{b7} settings",
        entries,
    }
}

/// The constellation vocabulary — the six star states, the hue dots,
/// both edges.
fn constellation(lex: &Lexicon) -> SinkSection {
    let entries: Vec<(&'static [&'static str], Element)> = vec![
        (
            &["star"],
            Element::row()
                .gap(14.0)
                .center()
                .child(Star::pending(lex))
                .child(Star::ready(lex))
                .child(Star::executing(lex))
                .child(Star::stuck(lex))
                .child(Star::blocked(lex))
                .child(Star::done(lex))
                .child(Star::hue(lex.mauve))
                .child(Star::hue(lex.blue))
                .child(Star::hue(lex.teal)),
        ),
        (
            &["edge"],
            Element::row()
                .gap(14.0)
                .center()
                .child(Edge::done(120.0, lex))
                .child(Edge::ahead(120.0, lex)),
        ),
    ];
    SinkSection {
        title: "the constellation",
        entries,
    }
}

/// One of the Brief's rise cards: severity ground, the coloured cap
/// with its kind mark, the body, one action.
fn rise_card(
    severity: Severity,
    icon: Icon,
    cap: &str,
    body: &str,
    action: Action,
    medium: Medium,
    lex: &Lexicon,
) -> Element {
    let text = Voice {
        px: 13.0,
        ..Voice::SLOT
    };
    Card::severity(severity, 16.0, 13.0, lex).child(
        Element::row()
            .gap(10.0)
            .center()
            .child(SeverityCap::el(severity, Some(icon), cap, lex))
            .child(
                Element::row()
                    .grow(1.0)
                    .child(Element::text(text, lex.tx, body).claim(Component::Prose)),
            )
            .child(ActionRow::new(None, vec![action]).el(medium)),
    )
}

/// The silhouette an undealt card holds — the same footprint, dashed,
/// so the deal reads as arrival rather than reflow.
fn ghost_card(lex: &Lexicon) -> Element {
    Element::surface()
        .height(46.0)
        .radius(13.0)
        .stroke(lex.grounded(lex.hairc), 1.0)
        .dashed()
        .center()
        .justify_center()
        .child(
            Element::text(Voice::CAP, lex.ov0, "dealing\u{2026}")
                .claim(Component::Cap)
                .nowrap(),
        )
}

/// The shipped toast card.
fn toast_card(medium: Medium, lex: &Lexicon) -> Element {
    Card::el(15.0, 12.0, lex)
        .child(SeverityCap::el(
            Severity::Good,
            None,
            "shipped \u{b7} toast",
            lex,
        ))
        .child(Element::surface().height(3.0))
        .child(
            Element::text(
                Voice {
                    px: 12.5,
                    weight: 600,
                    ..Voice::SLOT
                },
                lex.tx,
                "gui2-2a \u{b7} unlocked gui2-2b",
            )
            .claim(Component::Title)
            .nowrap(),
        )
        .child(Element::surface().height(8.0))
        .child(
            ActionRow::new(
                Some(Action::new("Claim gui2-2b", ElementId(94))),
                vec![Action::new("Later", ElementId(95))],
            )
            .gap(8.0)
            .el(medium),
        )
}

/// The pinned think-step card.
fn pin_card(lex: &Lexicon) -> Element {
    Card::el(14.0, 11.0, lex)
        .child(
            Element::row()
                .gap(8.0)
                .align_end()
                .child(SeverityCap::tinted(
                    lex.mauve,
                    None,
                    "think \u{b7} 388 \u{b7} pinned",
                ))
                .child(Element::row().grow(1.0))
                .child(
                    Element::text(Voice::KBD, lex.ov0, "task:design-canvas")
                        .claim(Component::Value),
                ),
        )
        .child(Element::surface().height(3.0))
        .child(
            Element::text(
                Voice {
                    px: 12.0,
                    ..Voice::SLOT
                },
                lex.sb1,
                "Open gui2-2a: one view, two arrangements; the ids never change.",
            )
            .claim(Component::Prose),
        )
}

/// The overlay-lift glass card, with the popover's chord row inside it
/// — the top two stops of the elevation scale on one exhibit.
fn overlay_card(medium: Medium, lex: &Lexicon) -> Element {
    let title = Voice {
        px: 16.0,
        weight: 600,
        ..Voice::SLOT
    };
    Card::overlay(lex.base, 26.0, 24.0, lex)
        .child(
            Element::text(title, lex.tx, "New project")
                .claim(Component::Title)
                .nowrap(),
        )
        .child(Element::surface().height(4.0))
        .child(
            Element::text(
                Voice::CHIP,
                lex.ov0,
                "Select a folder, a repository, or start empty",
            )
            .claim(Component::Prose)
            .nowrap(),
        )
        .child(Element::surface().height(14.0))
        .child(
            Element::row()
                .gap(10.0)
                .center()
                .child(
                    ActionRow::new(
                        None,
                        vec![
                            Action::new("Clone a repository\u{2026}", ElementId(120)),
                            Action::new("Start empty", ElementId(121)),
                        ],
                    )
                    .el(medium),
                )
                .child(Element::row().grow(1.0))
                .child(SeverityCap::tinted(lex.ov0, None, "esc \u{b7} Cancel")),
        )
}

#[cfg(test)]
#[path = "sink.tests.rs"]
mod tests;
