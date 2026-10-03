//! The cockpit's state.

use std::collections::VecDeque;
use std::time::Duration;

use magistr_workflow_contracts::{FeedStatus, NativeSnapshot};

use super::agent_missing::AgentMissing;
use super::animation::Animation;
use super::animations::Animations;
use super::bad_day::BadDay;
use super::binding::Binding;
use super::birth::Birth;
use super::birth_moment::BirthMoment;
use super::brief::Brief;
use super::changes::Changes;
use super::chapter::Chapter;
use super::checkpoint::Checkpoint;
use super::dag_node::DagNode;
use super::decision::Decision;
use super::dock::Dock;
use super::galaxy_project::GalaxyProject;
use super::hover_role::HoverRole;
use super::kill_cmd::KillCmd;
use super::launch_action::LaunchAction;
use super::launcher_data::LauncherData;
use super::loop_status::LoopStatus;
use super::motion::Motion;
use super::motion_key::MotionKey;
use super::motion_name::MotionName;
use super::msg::Msg;
use super::palette::Palette;
use super::pane::Pane;
use super::path_stop::PathStop;
use super::phase::DisplayPhase;
use super::place::Place;
use super::projection::Projection;
use super::rail::Rail;
use super::roadmap::Roadmap;
use super::search::Search;
use super::selected::Selected;
use super::session::Session;
use super::settings::Settings;
use super::settings_data::SettingsData;
use super::shelf::Shelf;
use super::size::Size;
use super::space::Space;
use super::stage_tab::StageTab;
use super::star_state::StarState;
use super::surface::Surface;
use super::task_state::TaskState;
use super::term::Term;
use super::term_dock::TermDock;
use super::term_hold::TermHold;
use super::terminals::Terminals;
use super::theme_choice::ThemeChoice;
use super::tooltip::Tooltip;
use super::trace::Trace;
use super::turn_state::TurnState;
use super::view;
use super::widget::Widget;
use crate::input::MouseEvent;
use crate::job::{JobId, JobOutput};
use crate::pixel::CellSize;
use crate::render::Color;
use crate::theme::{Theme, ThemeMode};
use crate::turn::{Accent, MAX_IMAGE_ROWS, Row, Viewport};
use crate::widget_id::WidgetId;
use crate::workbench::JournalEntry;

/// How many recent inputs the model remembers.
const RIBBON: usize = 8;

/// How many painted frames the activity ribbon keeps — the width of the
/// header's sparkline in samples.
pub(super) const ACTIVITY: usize = 24;

/// Rows the chrome takes around the stage on a terminal: the header,
/// the phase rail, the stage panel's top and bottom border, its tab
/// bar, the prompt line and the footer. The dock adds its own rows
/// ([`Dock::rows`] — its divider when empty, the cards plus the panel
/// ring otherwise). A window answers its own count through
/// [`Model::chrome_rows`].
const CHROME_ROWS: u16 = 7;
/// Rows the chrome takes around the stage on a window: the breadcrumb
/// header (3 — the design's ~55 px band on 20 px rows), the margin row
/// under it, the `AGENT · ITERATION` cap row, the composer pill (2),
/// the margin row under the pill, and the bottom bar. The window has no
/// tab bar, no panel ring, no prompt line, no phase strip and no
/// footer — the altitude is the navigation and the window's foot is the
/// bottom bar. The two margins are where the capture's own
/// rhythm puts the cap and the pill, measured against view 05.
const NATIVE_CHROME_ROWS: u16 = 9;
/// Columns the stage panel's left and right border take.
const STAGE_FRAME_COLS: u16 = 2;
/// Narrowest screen that undocks a stage tab into a third column.
const UNDOCK_MIN_COLS: u16 = 160;
/// Columns the undocked column takes (its divider adds one).
pub(super) const UNDOCK_COLS: u16 = 48;

/// Rows the prompt box grows to before it scrolls.
pub(super) const PROMPT_MAX_ROWS: usize = 5;

/// The cockpit: its size, the theme it draws with, the agent's turn and
/// the window onto it, the prompt line and the permission requests
/// waiting for an answer, how many frames it has drawn, how many inputs
/// it received and the last few of them, what the terminal answered at
/// startup, whether it is focused, whether it was asked to quit, and what
/// its producers delivered — agent events, engine frames and the feed's
/// attachment, job results — plus the workflow phase the feed implies
/// and, while a loop runs, where the loop stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub(super) size: Size,
    pub(super) theme: Theme,
    pub(super) title: Option<String>,
    /// When this session started, as the entry point spelled it
    /// (`09:02`) — the header's started stop. The
    /// core reads no clock, so it is told once.
    pub(super) session_started: Option<String>,
    pub(super) debug: bool,
    pub(super) session: Session,
    pub(super) session_viewport: Viewport,
    pub(super) roadmap_viewport: Viewport,
    pub(super) sheet_viewport: Viewport,
    pub(super) agent_activity: super::AgentActivity,
    pub(super) roadmap_requests: Vec<super::RoadmapRequest>,
    pub(super) roadmap_requests_open: bool,
    pub(super) roadmap_request_scroll: usize,
    pub(super) phase: Option<DisplayPhase>,
    pub(super) rail: Rail,
    pub(super) loop_status: Option<LoopStatus>,
    pub(super) terminals: Terminals,
    /// The terminal dock: the running card's screen
    /// raised over the session, as state — which terminal, how many
    /// rows, maximized, dragging, rising.
    pub(super) term_dock: TermDock,
    /// The held kill while a key or the cap holds it:
    /// what it would kill and the step it was pressed at.
    pub(super) hold: Option<TermHold>,
    /// The decision feature's state — the ask in front is on the dock,
    /// everything the screen holds about answering it is here.
    pub(super) decision: Decision,
    pub(super) dock: Dock,
    pub(super) motion: Motion,
    pub(super) changes: Changes,
    pub(super) trace: Trace,
    pub(super) roadmap: Roadmap,
    pub(super) launcher: Option<LauncherData>,
    /// The deliverables of the shipped chunk last opened on a window,
    /// once its job delivered — read through [`Model::shelf`], which
    /// only answers while that chunk is the selected one.
    pub(super) shelf: Option<Shelf>,
    pub(super) galaxy: super::galaxy::Galaxy,
    /// A project being born, moment by
    /// moment — read through [`Model::birth`]. Native-only, like the
    /// Galaxy.
    pub(super) birth: Birth,
    pub(super) launch: Option<LaunchAction>,
    /// The session's host runs work in place: a start is a command, not
    /// a quit for an entry point to re-dispatch.
    pub(super) hosted_work: bool,
    pub(super) session_sel: Option<usize>,
    pub(super) review_sel: usize,
    pub(super) session_note: Option<String>,
    pub(super) sheet: bool,
    pub(super) roadmap_graph_rect: Option<crate::layout::Rect>,
    pub(super) roadmap_graph_px: Option<crate::layout::PxRect>,
    /// Columns the surface holds mono turn content to — see
    /// [`Model::set_mono_cols`]. `None` on every cell surface.
    mono_cols: Option<u16>,
    pub(super) rail_graph_rect: Option<crate::layout::Rect>,
    pub(super) undock_rect: Option<crate::layout::Rect>,
    /// Where the tech-tree drag last read the pointer, in the
    /// fractional cells [`Model::pointer_at`] answers.
    pub(super) roadmap_drag: Option<(f32, f32)>,
    pub(super) flight_panels: super::flight_panels::FlightPanels,
    pub(super) hovered: Option<WidgetId>,
    /// The widget the left button went down on and has not come up
    /// from — what wears the pressed treatment.
    pub(super) pressed: Option<WidgetId>,
    /// The pointer's last reported cell, which is where a hover card
    /// hangs from.
    pub(super) pointer: Option<(u16, u16)>,
    pub(super) tab: StageTab,
    pub(super) tab_explicit: bool,
    pub(super) selected: Option<Selected>,
    pub(super) pane: Pane,
    pub(super) inspector_open: bool,
    pub(super) reasoning_expanded: bool,
    pub(super) inspector_scroll: usize,
    /// Where the window's camera is — the native
    /// surface's own navigation, never derived from [`Model::tab`]: a
    /// window shows [`Place::tab`], a terminal shows its tab and has no
    /// place.
    pub(super) place: Place,
    pub(super) frames: u64,
    pub(super) ticks: u64,
    /// How far into the current tick the frame being sampled is,
    /// `0.0..1.0`. Only a renderer that paints faster than the tick ever
    /// sets it (see [`Msg::Phase`](super::Msg::Phase)); graphics
    /// terminals sample it between ticks too. Plain terminals leave it
    /// at zero so their existing cell-motion timing stays unchanged.
    pub(super) tick_phase: f32,
    pub(super) inputs: u64,
    /// The tick the cockpit last saw a key, a mouse report or a paste
    /// on. Rest is `ticks - last_input` and nothing else, which is what
    /// keeps chrome fade a pure function of
    /// [`Model::ticks`] with no clock anywhere near it.
    pub(super) last_input: u64,
    /// How long the reader was away, when the entry point knew — the
    /// one fact the Brief cannot derive from the model it reads.
    pub(super) away: Option<Duration>,
    /// The tick the Brief was raised at: where its deal is timed from.
    pub(super) brief_at: u64,
    /// The ⌘K palette, while it is up — raised only on a native
    /// surface, the Brief's rule.
    pub(super) palette: Option<Palette>,
    /// The ⌘⇧F search, while it is up — the window's other float,
    /// raised by the same rule.
    pub(super) search: Option<Search>,
    /// The ⌘, settings surface, while it is up — the window's third
    /// float, raised by the same rule.
    pub(super) settings: Option<Settings>,
    /// The values the settings panes show and edit, whether or not
    /// the surface is up — the entry point fills them from the files.
    pub(super) settings_data: SettingsData,
    pub(super) quit_requested: bool,
    pub(super) focused: bool,
    pub(super) background: Option<Color>,
    pub(super) key_releases: bool,
    pub(super) cell_pixels: Option<CellSize>,
    pub(super) banner: Option<&'static str>,
    pub(super) pixels: bool,
    pub(super) surface: Surface,
    pub(super) pointer_shapes: bool,
    pub(super) text_sizing: bool,
    pub(super) activity: VecDeque<u64>,
    pub(super) activity_bucket: u64,
    pub(super) tokens: VecDeque<u64>,
    pub(super) token_used: Option<u64>,
    /// The window's size the same update reported — the headroom's
    /// other half.
    pub(super) token_size: Option<u64>,
    pub(super) token_mark: u64,
    pub(super) recent: VecDeque<Msg>,
    pub(super) agent_events: u64,
    pub(super) engine_frames: u64,
    pub(super) feed: Option<FeedStatus>,
    pub(super) feed_lost_at: Option<Duration>,
    /// How fresh the store's picture here is, and how the last event was
    /// delivered.
    pub(super) projection: Projection,
    pub(super) agent_missing: Option<AgentMissing>,
    pub(super) server: Option<String>,
    pub(super) refused: bool,
    pub(super) jobs_done: u64,
    pub(super) last_job: Option<(JobId, JobOutput)>,
    pub(super) last_job_frame: Option<u64>,
}

impl Model {
    /// The graphics terminal uses the approved conversation-centered workspace.
    #[must_use]
    pub const fn uses_flight_recorder(&self) -> bool {
        self.pixels && !self.surface.is_native()
    }

    /// Whether recorded reasoning is open beneath the conversation.
    #[must_use]
    pub const fn reasoning_expanded(&self) -> bool {
        self.reasoning_expanded
    }

    #[must_use]
    pub fn flight_layout(&self) -> super::FlightLayout {
        super::FlightLayout::of(self)
    }

    /// Native allocations are layout feedback only; they cannot dispatch effects.
    /// Hidden panels report None. Sizes are bounded and invalid boundaries are ignored.
    pub fn report_native_workspace(
        &mut self,
        panels: [Option<super::panel_allocation::PanelAllocation>; 4],
        stage: Option<(u16, usize)>,
        details: Option<(usize, usize)>,
    ) -> bool {
        if !self.surface.is_native() {
            return false;
        }
        let panels = panels.map(|p| p.filter(|p| p.valid()));
        let stage = stage.map(|(cols, rows)| (cols.max(1), rows.min(u16::MAX as usize)));
        let details =
            details.map(|(total, visible)| (total.min(1_000_000), visible.min(1_000_000)));
        let changed = self.flight_panels.native != panels
            || stage.is_some_and(|stage| self.flight_panels.native_stage != Some(stage))
            || self.flight_panels.native_details != details;
        if !changed {
            return false;
        }
        let anchor = self.stage_anchor();
        self.flight_panels.native = panels;
        // A temporarily hidden conversation keeps its last wrapping and reading position.
        if let Some(stage) = stage {
            self.flight_panels.native_stage = Some(stage);
            self.session.turn.rewrap(stage.0);
        }
        self.flight_panels.native_details = details;
        if let Some(anchor) = anchor {
            self.hold_stage_anchor(anchor);
        }
        changed
    }

    /// Native roadmap list capacity and request text extent, reported after layout.
    /// None clears hidden geometry; these facts never submit work or alter a request.
    pub fn report_native_roadmap(
        &mut self,
        rows: Option<usize>,
        requests: Option<(usize, usize)>,
    ) -> bool {
        if !self.surface.is_native() {
            return false;
        }
        let rows = rows.map(|rows| rows.min(u16::MAX as usize));
        let requests =
            requests.map(|(total, visible)| (total.min(1_000_000), visible.min(1_000_000)));
        let changed = self.flight_panels.native_roadmap_rows != rows
            || self.flight_panels.native_requests != requests;
        self.flight_panels.native_roadmap_rows = rows;
        self.flight_panels.native_requests = requests;
        self.roadmap.set_full_graph(rows.is_some());
        changed
    }

    #[must_use]
    pub fn panel_fraction(
        &self,
        divider: super::flight_divider::FlightDivider,
        default: f32,
    ) -> f32 {
        self.flight_panels.fraction(divider, default)
    }

    #[must_use]
    pub fn active_panel(&self) -> Option<super::flight_divider::FlightDivider> {
        self.flight_panels.active()
    }

    #[must_use]
    pub const fn inspector_offset(&self) -> usize {
        self.inspector_scroll
    }

    /// Keep row storage measured by the same fonts and allocation as the pixel painter.
    pub(super) fn sync_flight_wrap(&mut self) {
        let cell = self.cell_size();
        let layout = self.uses_flight_recorder().then(|| self.flight_layout());
        let width = layout
            .as_ref()
            .map(|layout| (layout.left_width - 42.0).max(1.0).floor() as u32);
        self.session.turn.set_conversation_compact(
            layout.is_some_and(|layout| layout.conversation_height < 380.0),
        );
        self.session.turn.set_pixel_wrap(width, cell);
    }

    /// Whether the inspector fits beside the stage at its minimum readable width.
    #[must_use]
    pub const fn inspector_stacked(&self) -> bool {
        !self.uses_flight_recorder()
            && !self.surface.is_native()
            && stage_width(self.size, self.rail.cols(self.size.cols))
                .saturating_sub(self.gutter_cells())
                .saturating_sub(self.undock_cols())
                < Space::INSPECTOR_COLS + Space::GAP + Space::STAGE_COLS
    }

    /// Outer height reserved below the stage, bounded on very short screens.
    #[must_use]
    pub fn inspector_stack_rows(&self) -> u16 {
        if !self.inspector_stacked() {
            return 0;
        }
        let available = self.body_stage_rows();
        Space::INSPECTOR_ROWS.min(available.saturating_sub(3) as u16)
    }

    /// Inspector text width, using the same allocation as the view.
    #[must_use]
    pub const fn inspector_width(&self) -> u16 {
        let outer = if self.inspector_stacked() {
            self.size
                .cols
                .saturating_sub(self.rail.cols(self.size.cols))
                .saturating_sub(self.undock_cols())
                .saturating_sub(if self.pixels { 2 * Space::PAD } else { 0 })
        } else {
            Space::INSPECTOR_COLS
        };
        outer.saturating_sub(2 + 2 * Space::PAD)
    }

    /// Requests kept on Roadmap, including their original wording and response range.
    #[must_use]
    pub fn roadmap_requests(&self) -> &[super::RoadmapRequest] {
        &self.roadmap_requests
    }

    /// Whether Roadmap shows requests instead of the selected item's details.
    #[must_use]
    pub const fn roadmap_requests_open(&self) -> bool {
        self.roadmap_requests_open
    }

    /// The request pane's independent scroll position.
    #[must_use]
    pub const fn roadmap_request_scroll(&self) -> usize {
        self.roadmap_request_scroll
    }

    /// Whether a roadmap request still awaits admission or a response.
    #[must_use]
    pub fn roadmap_request_pending(&self) -> bool {
        self.roadmap_requests
            .last()
            .is_some_and(|request| request.phase.is_active())
    }

    /// Whether the current agent response belongs to a roadmap request.
    #[must_use]
    pub fn roadmap_request_running(&self) -> bool {
        self.roadmap_requests
            .last()
            .is_some_and(|request| request.phase == super::RequestPhase::Working)
    }

    /// The observed activity; live requests take precedence over saved completion state.
    #[must_use]
    pub fn agent_activity(&self) -> super::AgentActivity {
        let mut activity = self.agent_activity.clone();
        let implementation = self
            .loop_status
            .as_ref()
            .filter(|status| status.ended().is_none());
        if !self.roadmap_request_running() {
            if let Some(status) = implementation {
                let work = self
                    .rail
                    .chunk()
                    .and_then(|id| self.roadmap.chunk(id))
                    .map(|chunk| {
                        if chunk.title.is_empty() {
                            chunk.name.as_str()
                        } else {
                            chunk.title.as_str()
                        }
                    });
                activity.task = work.map_or_else(
                    || "Working on a work item".to_owned(),
                    |name| format!("Working on {name}"),
                );
                activity.active = true;
                if status.is_paused() {
                    activity.status = "Paused";
                    activity.active = false;
                } else {
                    match status.stage() {
                        super::LoopStage::Connecting => activity.status = "Starting agent",
                        super::LoopStage::Gates => activity.status = "Checking work",
                        super::LoopStage::Judge => activity.status = "Reviewing work",
                        super::LoopStage::Agent if !self.agent_activity.active => {
                            activity.status = "Waiting for response"
                        }
                        super::LoopStage::Agent => {}
                    }
                }
            } else if !activity.active {
                if let Some(request) = self
                    .roadmap_requests
                    .last()
                    .filter(|request| request.phase == super::RequestPhase::Waiting)
                {
                    activity.task = "Updating roadmap".to_owned();
                    activity.status = request.phase.label();
                    activity.active = true;
                } else if self.session.notice == Some("Work failed") {
                    activity.status = "Work failed";
                } else if self.session.notice == Some("Connection error") {
                    activity.status = "Connection error";
                } else if self
                    .loop_status
                    .as_ref()
                    .is_some_and(|status| status.ended().is_some())
                    && activity.status == "Ready"
                {
                    activity.status = "Stopped";
                }
            }
        }
        if !self.dock.is_empty() {
            activity.status = "Needs your answer";
            activity.active = false;
        } else if self.agent_missing.is_some() {
            activity.status = "Agent unavailable";
            activity.active = false;
        } else if !activity.active
            && activity.status != "Work failed"
            && self.feed == Some(FeedStatus::Disconnected)
            && !implementation.is_some_and(|status| status.stage().is_between_agents())
            && self
                .loop_status
                .as_ref()
                .is_none_or(|status| status.ended().is_none())
        {
            activity.status = "Disconnected";
        } else if !activity.active
            && activity.status == "Ready"
            && self.projection.freshness() == super::Freshness::Stale
        {
            activity.status = "Waiting for updates";
        }
        activity
    }

    /// The stable object identity shared by every terminal view.
    #[must_use]
    pub const fn selected(&self) -> Option<&Selected> {
        self.selected.as_ref()
    }

    /// The pane receiving navigation keys.
    #[must_use]
    pub const fn pane(&self) -> Pane {
        self.pane
    }

    /// Whether the inspector reveals all fields.
    #[must_use]
    pub const fn inspector_open(&self) -> bool {
        self.inspector_open
    }

    /// Select by identity; the per-view cursors remain navigation history.
    /// Repeating a selection preserves the object, disclosure and scroll.
    pub(super) fn inspect(&mut self, selected: Selected) {
        if self.selected.as_ref() != Some(&selected) {
            self.inspector_open = false;
            self.inspector_scroll = 0;
        }
        match &selected {
            Selected::Chunk(id) => {
                self.roadmap
                    .select(id, self.now(), self.theme.motion().is_full());
            }
            Selected::Step(number) => {
                self.trace.expand_step(*number);
            }
            Selected::Task(_)
            | Selected::Tool(_)
            | Selected::Change(_)
            | Selected::Signal(_)
            | Selected::Receipt(_)
            | Selected::Run(_) => {
                self.trace.deselect();
            }
        }
        self.trace.close_viewer();
        self.changes.close_viewer();
        self.selected = Some(selected);
        if self.uses_flight_recorder() {
            self.pane = Pane::Inspector;
        }
    }

    /// A fresh cockpit at `size`, focused, nothing received yet, in the
    /// dark theme until the terminal answers OSC 11 (see
    /// [`Model::with_theme`]).
    #[must_use]
    pub fn new(size: Size) -> Self {
        let rail = Rail::default();
        let undock = if size.cols >= UNDOCK_MIN_COLS {
            UNDOCK_COLS + 1
        } else {
            0
        };
        let mut model = Self {
            size,
            theme: Theme::default(),
            title: None,
            session_started: None,
            debug: false,
            session: Session::new(
                stage_width(size, rail.cols(size.cols))
                    .saturating_sub(undock)
                    .saturating_sub(Accent::GUTTER_COLS),
            ),
            session_viewport: Viewport::new(),
            roadmap_viewport: Viewport::at_top(),
            sheet_viewport: Viewport::at_top(),
            agent_activity: super::AgentActivity::default(),
            roadmap_requests: Vec::new(),
            roadmap_requests_open: false,
            roadmap_request_scroll: 0,
            phase: None,
            rail,
            loop_status: None,
            terminals: Terminals::default(),
            term_dock: TermDock::new(),
            hold: None,
            decision: Decision::default(),
            dock: Dock::default(),
            motion: Motion::default(),
            changes: Changes::default(),
            trace: Trace::default(),
            roadmap: Roadmap::default(),
            launcher: None,
            shelf: None,
            galaxy: super::galaxy::Galaxy::default(),
            birth: Birth::default(),
            session_sel: None,
            review_sel: 0,
            session_note: None,
            launch: None,
            hosted_work: false,
            sheet: false,
            roadmap_graph_rect: None,
            roadmap_graph_px: None,
            mono_cols: None,
            rail_graph_rect: None,
            undock_rect: None,
            roadmap_drag: None,
            flight_panels: super::flight_panels::FlightPanels::default(),
            hovered: None,
            pressed: None,
            pointer: None,
            tab: StageTab::Roadmap,
            tab_explicit: false,
            selected: None,
            pane: Pane::Stage,
            inspector_open: false,
            reasoning_expanded: false,
            inspector_scroll: 0,
            place: Place::Session,
            frames: 0,
            ticks: 0,
            tick_phase: 0.0,
            inputs: 0,
            last_input: 0,
            away: None,
            brief_at: 0,
            feed_lost_at: None,
            agent_missing: None,
            server: None,
            refused: false,
            palette: None,
            search: None,
            settings: None,
            settings_data: SettingsData::default(),
            quit_requested: false,
            focused: true,
            background: None,
            key_releases: false,
            cell_pixels: None,
            banner: None,
            pixels: false,
            surface: Surface::Terminal,
            pointer_shapes: false,
            text_sizing: false,
            activity: VecDeque::with_capacity(ACTIVITY),
            activity_bucket: 0,
            tokens: VecDeque::with_capacity(ACTIVITY),
            token_used: None,
            token_size: None,
            token_mark: 0,
            recent: VecDeque::with_capacity(RIBBON),
            agent_events: 0,
            engine_frames: 0,
            feed: None,
            projection: Projection::default(),
            jobs_done: 0,
            last_job: None,
            last_job_frame: None,
        };
        model.session.turn.rewrap(model.turn_width());
        model
    }

    /// Start in `theme` — what the entry point decided from
    /// `MAGISTR_THEME`, `MAGISTR_ICONS` and `MAGISTR_CVD`; a forced theme
    /// ignores the terminal's answer. The turn's code highlighting
    /// follows the theme's mode and its status glyphs the theme's icons.
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.set_theme(theme);
        self
    }

    /// Draw with `theme` from here on: the turn's code highlighting
    /// follows its mode and its status glyphs its icons — the one
    /// place the theme moves, so the two can never disagree.
    pub(super) fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.session.turn.set_mode(theme.mode());
        self.session.turn.set_icons(theme.icons());
    }

    /// Choose a theme card: the sky forces night, the chart forces
    /// day, Follow OS goes back to what the surface's background says
    /// — each keeping the glyph, CVD and motion choices, which are
    /// the reader's own.
    pub(super) fn choose_theme(&mut self, choice: ThemeChoice) {
        self.settings_data.theme = choice;
        let theme = match choice.forced() {
            Some(mode) => Theme::forced(mode),
            None => Theme::detect(self.background, None),
        }
        .with_icons(self.theme.icon_mode())
        .with_cvd(self.theme.cvd())
        .with_motion(self.theme.motion());
        self.set_theme(theme);
    }

    /// Start with `data` in the settings panes — what the entry point
    /// read from the user, project and team files.
    #[must_use]
    pub fn with_settings(mut self, data: SettingsData) -> Self {
        self.settings_data = data;
        self
    }

    /// The host behind this session starts one item or the loop in place
    /// when asked ([`Cmd::Launch`](super::Cmd::Launch)): the start verbs
    /// are offered in a live session and never quit it.
    #[must_use]
    pub const fn with_hosted_work(mut self) -> Self {
        self.hosted_work = true;
        self
    }

    /// Open the loop's first choice inside this hosted session.
    #[must_use]
    pub fn with_loop_track_picker(mut self) -> Self {
        self.hosted_work = true;
        self.palette = Some(super::Palette::tracks());
        self
    }

    /// Whether a start is carried out in place by the session's host.
    #[must_use]
    pub const fn hosts_work(&self) -> bool {
        self.hosted_work
    }

    /// Set the project this host has opened, before project discovery completes.
    #[must_use]
    pub fn with_project(mut self, name: impl Into<String>, dir: impl Into<String>) -> Self {
        self.galaxy.set_current(name.into(), dir.into());
        self
    }

    /// Show `title` in the header — the entry point's session line
    /// (project · backend · model), instead of the generic name.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Name when this session started — the local `HH:MM` the entry
    /// point read as it opened the window, which the header's session
    /// stop wears (`session · 09:02`, ). Supplied, never
    /// read here: the core has no clock.
    #[must_use]
    pub fn with_session_started(mut self, started: impl Into<String>) -> Self {
        self.session_started = Some(started.into());
        self
    }

    /// When this session started, once the entry point said.
    #[must_use]
    pub fn session_started(&self) -> Option<&str> {
        self.session_started.as_deref()
    }

    /// Start the rail, the Roadmap tab and the reasoning model on `snapshot` —
    /// what the entry point read off the engines before taking the
    /// terminal over, so an objective recorded before the feed connected
    /// shows its plan (and the roadmap its chunks, and the trace its
    /// steps) on the first frame.
    #[must_use]
    pub fn with_snapshot(mut self, snapshot: &NativeSnapshot) -> Self {
        self.seed(snapshot);
        self
    }

    /// Take the store's snapshot as the rail's, the Roadmap tab's and the
    /// reasoning model's state, and mark the projection current.
    pub(super) fn seed(&mut self, snapshot: &NativeSnapshot) {
        self.rail.seed(snapshot);
        self.roadmap.seed(&snapshot.chunks, &snapshot.signals);
        self.trace.seed(snapshot);
        self.projection.seeded();
    }

    /// Set the time away used to open the native catch-up overlay.
    /// The entry point supplies the gap; [`Model::brief`] derives the overlay's
    /// content from current model state.
    #[must_use]
    pub const fn with_away(mut self, away: Duration) -> Self {
        self.away = Some(away);
        self.brief_at = self.ticks;
        self
    }

    /// The agent binary the entry point could not find — what raises
    /// the bad day's agent card (view 14). Supplied like
    /// [`Model::with_away`]: the entry point looked before the surface
    /// came up, and the cockpit never searches PATH itself.
    #[must_use]
    pub fn with_agent_missing(mut self, missing: AgentMissing) -> Self {
        self.agent_missing = Some(missing);
        self
    }

    /// The server this cockpit is attached to, by name — what the bad
    /// day's connection card says it lost. Supplied by the attach entry
    /// point, which is the one that dialled it.
    #[must_use]
    pub fn with_server(mut self, server: impl Into<String>) -> Self {
        self.server = Some(server.into());
        self
    }

    /// The missing agent, when the entry point reported one.
    #[must_use]
    pub const fn agent_missing(&self) -> Option<&AgentMissing> {
        self.agent_missing.as_ref()
    }

    /// The attached server's name, when one was given.
    #[must_use]
    pub fn server(&self) -> Option<&str> {
        self.server.as_deref()
    }

    /// The loop's elapsed when the feed hung up — the stamp the
    /// connection card ages by. `None` while attached, or when no loop
    /// clock was running to stamp it.
    #[must_use]
    pub const fn feed_lost_at(&self) -> Option<Duration> {
        self.feed_lost_at
    }

    /// Whether the last turn ended in the model's refusal, and no turn
    /// has been sent since.
    #[must_use]
    pub const fn refused(&self) -> bool {
        self.refused
    }

    /// The bad day, when anything is wrong: a fresh reading of the
    /// model's own failures ([`BadDay::of`]), or `None`. A window's
    /// reading only — the cards are native overlays, and a terminal
    /// keeps its error lines on the stage.
    #[must_use]
    pub fn bad_day(&self) -> Option<BadDay> {
        if !self.surface.is_native() {
            return None;
        }
        BadDay::of(self)
    }

    /// The state `chunk`'s star wears: its status, unless a fault
    /// rings it — then Blocked, whatever the roadmap says, so the sky
    /// and the card cannot disagree about what is stopped.
    #[must_use]
    pub fn star_state(&self, chunk: &super::roadmap_chunk::RoadmapChunk) -> StarState {
        if self
            .bad_day()
            .is_some_and(|day| day.stars().any(|star| star == chunk.id))
        {
            return StarState::Blocked;
        }
        StarState::of(chunk.state, false)
    }

    /// The murmur's note for `chunk`, when a fault rings it —
    /// `Requires agent`.
    #[must_use]
    pub fn star_note(&self, chunk: &str) -> Option<String> {
        self.bad_day()?.star_note(chunk).map(str::to_owned)
    }

    /// The constellation's nodes at `now`, with every star a fault
    /// rings forced Blocked — what the window draws, so the sky agrees
    /// with the cards over it.
    #[must_use]
    pub fn sky_nodes(&self, now: f32) -> Vec<DagNode> {
        let mut nodes = self.roadmap.dag_nodes_at(now);
        if let Some(day) = self.bad_day() {
            let ringed: Vec<&str> = day.stars().collect();
            for (node, id) in nodes.iter_mut().zip(self.roadmap.graph_ids()) {
                if ringed.contains(&id) {
                    node.state = StarState::Blocked;
                }
            }
        }
        nodes
    }

    /// The Brief, when one is raised: a fresh reading of the model's
    /// own ledger, or `None`.
    ///
    /// Four things have to hold. The surface is a window — the Brief is
    /// one of the native overlays and a terminal has no altitude to raise it
    /// over. An away time was supplied. Nothing is *asking* — a pending
    /// decision overrides the catch-up, because a question the agent is
    /// blocked on is more urgent than a summary of what already
    /// happened, and own words are that the card comes to
    /// you. And there is news: [`Brief::of`] answers `None` on a quiet
    /// project.
    #[must_use]
    pub fn brief(&self) -> Option<Brief> {
        // The away check first, and not for style: `brief` is read on
        // every fold (through `is_sky` and the animation set) and
        // `Brief::of` sorts the roadmap's rows. With no away supplied —
        // which is every session but the first frames of one — this is
        // a field test and nothing else.
        let away = self.away?;
        if !self.surface.is_native() || self.has_question() || self.bad_day().is_some() {
            return None;
        }
        Brief::of(self, away)
    }

    /// The ⌘K palette, while it is up.
    #[must_use]
    pub const fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    /// The ⌘⇧F search, while it is up.
    #[must_use]
    pub const fn search(&self) -> Option<&Search> {
        self.search.as_ref()
    }

    /// The ⌘, settings surface, while it is up.
    #[must_use]
    pub const fn settings(&self) -> Option<&Settings> {
        self.settings.as_ref()
    }

    /// The values the settings panes show.
    #[must_use]
    pub const fn settings_data(&self) -> &SettingsData {
        &self.settings_data
    }

    /// Put the Brief away — `true` when there was one to dismiss, so a
    /// key handler can tell whether Esc was spent here.
    pub const fn dismiss_brief(&mut self) -> bool {
        let had = self.away.is_some();
        self.away = None;
        had
    }

    /// How far the Brief's card at `index` has risen, `0..1` — `None`
    /// for a card that has landed, and then it is drawn exactly as it
    /// settles.
    ///
    /// The *rise*: cards are dealt
    /// [`Motion::RISE_STAGGER`] apart, each fading in over
    /// [`Motion::ARRIVAL_STEPS`], so the whole deal is a hand laying
    /// cards down. Timed from the tick the Brief was raised at, so it
    /// is a pure function of [`Model::ticks`] like every other curve;
    /// under reduced motion the tick never runs and every card is
    /// simply landed.
    #[must_use]
    pub fn brief_rise(&self, index: usize) -> Option<f32> {
        if !self.theme.motion().is_full() {
            return None;
        }
        let dealt = self.brief_at as f32 + (index as u64 * Motion::RISE_STAGGER) as f32;
        let elapsed = self.now() - dealt;
        if elapsed < 0.0 {
            return Some(0.0);
        }
        (elapsed < Motion::ARRIVAL_STEPS as f32).then_some(elapsed / Motion::ARRIVAL_STEPS as f32)
    }

    /// Whether the Brief's deal is still running, so the runtime keeps
    /// its tick.
    #[must_use]
    pub fn brief_dealing(&self) -> bool {
        self.brief()
            .is_some_and(|brief| (0..brief.cards().len()).any(|i| self.brief_rise(i).is_some()))
    }

    /// Whether something is waiting on a person right now: a decision
    /// raised as a Moment, a change raised as a review, or a permission
    /// still queued on the dock. The one predicate the Brief and rest
    /// both stand down for.
    #[must_use]
    pub fn has_question(&self) -> bool {
        self.moment().is_some() || self.review().is_some() || self.permissions_pending() > 0
    }

    /// Whether the sky is the surface rest applies to: a window, at
    /// altitude 1 (the Roadmap tab is the Sky), with no overlay of its
    /// own up and nothing asking.
    #[must_use]
    pub fn is_sky(&self) -> bool {
        self.surface.is_native()
            // The Sky itself — not every place that draws the Roadmap
            // tab's surface: the inbox, the Report and a project being
            // born share the tab and none of them rests into the
            // constellation.
            && self.place == Place::Sky
            && !self.sheet
            && self.brief().is_none()
            && !self.has_question()
            // A bad day is not rest: the cards hold the room and the
            // constellation stands still under them (view 14).
            && self.bad_day().is_none()
    }

    /// Whether the idle countdown and chrome fade have both finished.
    /// The native window then shows only the roadmap and status rows. Reduced
    /// motion keeps the chrome visible because the animation tick does not advance.
    #[must_use]
    pub fn sky_at_rest(&self) -> bool {
        self.is_sky() && self.idle_ticks() >= Motion::REST_STEPS + Motion::CHROME_FADE_STEPS
    }

    /// Where the sky's chrome fade stands, `0..1` from gone to whole —
    /// `None` when the chrome is simply *there*, and `None` at rest,
    /// when it is simply not.
    ///
    /// One curve serves both directions. Exhaling, the countdown has
    /// run out and the fade walks 1 down to 0. Returning, the input
    /// that restarted the countdown puts the frame back at the top of
    /// the same window, and the first [`Motion::CHROME_FADE_STEPS`]
    /// walk 0 up to 1 — 150 ms after input.
    #[must_use]
    pub fn sky_chrome_fade(&self) -> Option<f32> {
        if !self.is_sky() || !self.theme.motion().is_full() {
            return None;
        }
        let idle = self.now() - self.last_input as f32;
        let fade = Motion::CHROME_FADE_STEPS as f32;
        if idle < fade {
            return (self.last_input > 0).then(|| (idle / fade).clamp(0.0, 1.0));
        }
        let out = idle - Motion::REST_STEPS as f32;
        (out >= 0.0 && out < fade).then(|| 1.0 - out / fade)
    }

    /// Ticks since the last input — what rest is measured in.
    #[must_use]
    pub const fn idle_ticks(&self) -> u64 {
        self.ticks.saturating_sub(self.last_input)
    }

    /// Show the diagnostics chrome — the frame counter in the header and
    /// the terminal's answers in the footer. The entry points set this
    /// from `MAGISTR_TUI_DEBUG`.
    #[must_use]
    pub const fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// The header's session title, when the entry point gave one.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Whether the diagnostics chrome shows.
    #[must_use]
    pub const fn is_debug(&self) -> bool {
        self.debug
    }

    /// The terminal size the model was last told.
    #[must_use]
    pub const fn size(&self) -> Size {
        self.size
    }

    /// The theme the view draws with.
    #[must_use]
    pub const fn theme(&self) -> Theme {
        self.theme
    }

    /// Everything the agent's terminal `id` has written so far, raw —
    /// the bytes a live pane paints (acp-terminal-pty-live).
    #[must_use]
    pub fn terminal_output(&self, id: &str) -> Option<&[u8]> {
        self.terminals.bytes(id)
    }

    /// The screen terminal `id` — the agent's by its id, a gate's by its
    /// name — has painted so far, once it has written anything: what a
    /// pane shows.
    #[must_use]
    pub fn term(&self, id: &str) -> Option<&Term> {
        self.terminals.term(id)
    }

    /// The terminal dock, as state: up or down, its
    /// rows, maximized, a drag in progress.
    #[must_use]
    pub const fn term_dock(&self) -> &TermDock {
        &self.term_dock
    }

    /// The dock's screen at this frame — the docked terminal and the
    /// rows the dock shows of it — while any of it stands above the
    /// composer's edge: raised, rising, or still lowering.
    #[must_use]
    pub fn docked_term(&self) -> Option<(&Term, usize)> {
        let now = self.now();
        let id = self.term_dock.term(now)?;
        Some((self.terminals.term(id)?, self.term_dock.height_at(now)))
    }

    /// The rows the stage would show with the dock down.
    fn stage_rows_undocked(&self) -> usize {
        self.stage_rows()
            .saturating_add(self.term_dock.taken(self.now()))
    }

    /// The dock's first rise: a third of the stage, in the dock's rows.
    #[must_use]
    pub fn dock_first_rows(&self) -> u16 {
        TermDock::clamp(u16::try_from(self.stage_rows_undocked() / 3).unwrap_or(u16::MAX))
    }

    /// The rows a maximized dock takes: the stage less the grip row,
    /// held to the dock's limits.
    #[must_use]
    pub fn dock_max_rows(&self) -> u16 {
        TermDock::clamp(
            u16::try_from(self.stage_rows_undocked().saturating_sub(1)).unwrap_or(u16::MAX),
        )
    }

    /// Raise the running card's terminal into the dock: at the height
    /// this project remembers, else a third of the stage. Nothing — and
    /// `false` — without a card that ran on a terminal that has written
    /// something.
    pub(super) fn raise_term_dock(&mut self) -> bool {
        let Some(id) = self.session.turn.running_terminal().map(str::to_owned) else {
            return false;
        };
        if self.terminals.term(&id).is_none() {
            return false;
        }
        let rows = self
            .settings_data
            .dock_rows
            .unwrap_or_else(|| self.dock_first_rows());
        let (now, animate) = (self.now(), self.theme.motion().is_full());
        self.term_dock.raise(&id, rows, now, animate);
        self.sync_dock_window();
        true
    }

    /// Lower the dock; `false` when it was down.
    pub(super) fn lower_term_dock(&mut self) -> bool {
        let (now, animate) = (self.now(), self.theme.motion().is_full());
        if !self.term_dock.lower(now, animate) {
            return false;
        }
        self.sync_dock_window();
        true
    }

    /// Maximize the dock — raising it first when it is down — or, when
    /// it is maximized, back to the dragged height. The rail stays.
    pub(super) fn maximize_term_dock(&mut self) {
        if !self.term_dock.is_up() && !self.raise_term_dock() {
            return;
        }
        if self.term_dock.is_maximized() {
            self.term_dock.restore();
        } else {
            let max = self.dock_max_rows();
            let (now, animate) = (self.now(), self.theme.motion().is_full());
            self.term_dock.maximize(max, now, animate);
        }
        self.sync_dock_window();
    }

    /// Size the docked terminal's window to the dock: its target rows
    /// while up, a card's pane once lowered — so the dock and the card
    /// read one screen at one anchor.
    pub(super) fn sync_dock_window(&mut self) {
        let rows = if self.term_dock.is_up() {
            usize::from(self.term_dock.target_rows())
        } else {
            Term::PANE_ROWS
        };
        if let Some(term) = self
            .term_dock
            .id()
            .and_then(|id| self.terminals.term_mut(id))
        {
            term.set_window(rows);
        }
    }

    /// The workflow phase the engine feed last implied, once it did.
    #[must_use]
    pub const fn phase(&self) -> Option<DisplayPhase> {
        self.phase
    }

    /// The left rail: the plan, the roadmap's position, the pending
    /// signals, and its geometry.
    #[must_use]
    pub const fn rail(&self) -> &Rail {
        &self.rail
    }

    /// The rail, to seed before the runtime takes the model: the
    /// session's charter rows and bound budget are derived once at
    /// construction (they have no live feed), and a fixture seeds the
    /// spine the same way. Folding stays `update`'s alone.
    pub const fn rail_mut(&mut self) -> &mut Rail {
        &mut self.rail
    }

    /// Where the iteration loop stands, while one runs.
    #[must_use]
    pub const fn loop_status(&self) -> Option<&LoopStatus> {
        self.loop_status.as_ref()
    }

    /// Whether a loop is running or held at a boundary: one started and
    /// has not ended. The start verbs are offered only while this is
    /// false, the loop's own verbs only while it is true.
    #[must_use]
    pub fn loop_is_live(&self) -> bool {
        self.loop_status
            .as_ref()
            .is_some_and(|status| status.ended().is_none())
    }

    /// Rows the prompt box takes: the text's lines, capped at
    /// [`PROMPT_MAX_ROWS`].
    #[must_use]
    pub fn prompt_rows(&self) -> usize {
        if self.pixels && !self.surface.is_native() {
            self.session
                .prompt
                .wrapped_rows(self.size.cols.saturating_sub(8))
                .max(if self.size.rows >= 30 { 2 } else { 1 })
                .min(PROMPT_MAX_ROWS.min(usize::from(self.size.rows / 5)).max(1))
        } else {
            self.session.prompt.rows().min(PROMPT_MAX_ROWS)
        }
    }

    /// The Needs-you dock: the typed cards waiting for the human.
    #[must_use]
    pub const fn dock(&self) -> &Dock {
        &self.dock
    }

    /// The checkpoint a review is scoped since — the row the reader is
    /// previewing when the port confirmed it (a checkpoint row's
    /// Preview, ), otherwise the newest state the port
    /// confirmed reproducible, or `None` when the spine holds no
    /// verified state and the review is the journal's own list. An
    /// unverified preview scopes nothing new: a reading since a state
    /// nobody has confirmed would be a reading since nothing.
    #[must_use]
    pub fn review_since(&self) -> Option<&str> {
        let spine = self.rail.spine();
        spine
            .previewed()
            .filter(|cp| cp.is_verified())
            .or_else(|| spine.verified_head())
            .map(Checkpoint::id)
    }

    /// The review's scope line — `Since cp-3 · 3 files`, or
    /// `Journal · 3 files` when nothing is checkpointed yet.
    #[must_use]
    pub fn review_scope(&self) -> String {
        let files = self.review_files().len();
        let plural = if files == 1 { "" } else { "s" };
        match self.review_since() {
            Some(id) => format!("Since {id} · {files} file{plural}"),
            None => format!("Journal · {files} file{plural}"),
        }
    }

    /// The files a review covers: the workbench journal as the Changes
    /// tab holds it.
    ///
    /// One list, not a second one taken at review time: the journal is
    /// already the authoritative record of what the session wrote, so a
    /// review reading anything else could show a file the journal model
    /// does not.
    #[must_use]
    pub fn review_files(&self) -> &[JournalEntry] {
        self.changes.entries()
    }

    /// Which reviewed file's diff shows — the first, until one is
    /// clicked. Clamped to the list, so a journal refresh that drops
    /// files can never leave the pane pointing past the end.
    #[must_use]
    pub fn review_file(&self) -> Option<&JournalEntry> {
        let files = self.review_files();
        files.get(self.review_sel.min(files.len().saturating_sub(1)))
    }

    /// The index of the file whose diff shows, clamped the same way.
    #[must_use]
    pub fn review_index(&self) -> usize {
        self.review_sel
            .min(self.review_files().len().saturating_sub(1))
    }

    /// Show the diff of the file at `index`.
    pub(super) fn select_review_file(&mut self, index: usize) {
        if index < self.review_files().len() {
            self.review_sel = index;
        }
    }

    /// The agent's one-line *why*, riding under the diff — the newest
    /// thing it thought this turn.
    #[must_use]
    pub fn review_note(&self) -> Option<&str> {
        self.session.turn.last_thought()
    }

    /// The journal model's state: the journal, the last run, the viewer.
    #[must_use]
    pub const fn changes(&self) -> &Changes {
        &self.changes
    }

    /// The reasoning model's state: the think feed, the ship timeline, the
    /// checkpoint pane.
    #[must_use]
    pub const fn trace(&self) -> &Trace {
        &self.trace
    }

    /// The Roadmap tab's state: the chunks, the signals inbox, the
    /// selection and the tech-tree's pan.
    #[must_use]
    pub const fn roadmap(&self) -> &Roadmap {
        &self.roadmap
    }

    /// The launcher's card data, once its job delivered.
    #[must_use]
    pub const fn launcher(&self) -> Option<&LauncherData> {
        self.launcher.as_ref()
    }

    /// The selected chunk's deliverables shelf, once its job delivered
    /// — `None` while no chunk is selected, while the selected chunk is
    /// not the one the shelf was gathered for, or while the chunk is
    /// not shipped: a shelf is what a *Done* chunk left behind, and the
    /// gallery never shows one chunk's receipts under another's name.
    #[must_use]
    pub fn shelf(&self) -> Option<&Shelf> {
        let selected = self.roadmap.selected()?;
        if !selected.state.is_done() {
            return None;
        }
        self.shelf
            .as_ref()
            .filter(|shelf| shelf.chunk == selected.id)
    }

    /// The selected shipped chunk's chapter (view 09) — the report's
    /// document, derived from the roadmap on every read so it cannot
    /// go stale under a re-seed. `None` while no chunk is selected or
    /// while the selected one has not shipped: a chapter is a Done
    /// chunk's, like its shelf.
    #[must_use]
    pub fn chapter(&self) -> Option<Chapter> {
        let selected = self.roadmap.selected()?;
        selected
            .state
            .is_done()
            .then(|| Chapter::of(selected, &self.roadmap))
    }

    /// The Galaxy — every project this machine knows, the selection
    /// and the zoom. Empty until its job
    /// delivered; a terminal never draws it.
    #[must_use]
    pub const fn galaxy(&self) -> &super::galaxy::Galaxy {
        &self.galaxy
    }

    /// The project being born: which
    /// moment the flow is at and everything it has gathered. Native-
    /// only, like the Galaxy: a terminal never opens it.
    #[must_use]
    pub const fn birth(&self) -> &Birth {
        &self.birth
    }

    /// Whether the window is on Birth's flow and the flow has motion
    /// at this tick — the survey's rows landing under the sweep, the
    /// stars being born — so the runtime keeps its tick while it does
    /// and stops the moment it settles.
    #[must_use]
    pub fn birth_moving(&self) -> bool {
        self.surface.is_native() && self.place == Place::Birth && self.birth.is_moving(self.now())
    }

    /// Whether the Sky's own drawing is the room at this moment: first
    /// light draws the accepted roadmap as the constellation it is,
    /// with the field the composition reserves for it.
    #[must_use]
    pub fn birth_shows_sky(&self) -> bool {
        self.place() == Some(Place::Birth) && self.birth.moment() == BirthMoment::FirstLight
    }

    /// What the launcher asked the entry point to start, when the quit
    /// was one of its action buttons or one of its session rows.
    #[must_use]
    pub const fn launch(&self) -> Option<&LaunchAction> {
        self.launch.as_ref()
    }

    /// The launcher's selected session row, once the arrows moved onto
    /// one — what Enter resumes.
    #[must_use]
    pub const fn session_selected(&self) -> Option<usize> {
        self.session_sel
    }

    /// Why the last session row refused to launch, when one did — the
    /// sessions card carries it under the rows.
    #[must_use]
    pub fn session_note(&self) -> Option<&str> {
        self.session_note.as_deref()
    }

    /// Whether the accelerator sheet covers the stage.
    #[must_use]
    pub const fn is_sheet_open(&self) -> bool {
        self.sheet
    }

    /// Where the tech-tree block landed this frame — the runtime reports
    /// it after layout ([`Model::set_roadmap_graph_rect`]) so a mouse
    /// event over the graph can be taken in block-relative cells.
    #[must_use]
    pub const fn roadmap_graph_rect(&self) -> Option<crate::layout::Rect> {
        self.roadmap_graph_rect
    }

    /// The runtime's report of the tech-tree's placed rect (or `None`
    /// while another tab shows or graphics are off). The tab's layout
    /// scales to it, so the sub-state learns the viewport too.
    pub fn set_roadmap_graph_rect(&mut self, rect: Option<crate::layout::Rect>) {
        self.roadmap_graph_rect = rect;
        self.roadmap.set_viewport(rect.map(|r| (r.width, r.height)));
    }

    /// Exact physical bounds shared by roadmap drawing and pointer input.
    #[must_use]
    pub const fn roadmap_graph_px(&self) -> Option<crate::layout::PxRect> {
        self.roadmap_graph_px
    }

    /// Report the physical graph allocation without rounding to terminal cells.
    pub fn set_roadmap_graph_px(&mut self, rect: Option<crate::layout::PxRect>) {
        self.roadmap_graph_px = rect;
        self.roadmap.set_pixel_viewport(
            rect.map(|r| (r.width, r.height)),
            f32::from(self.cell_size().height),
        );
    }

    /// Where the rail's mini-DAG block landed this frame — `None` while
    /// the rail is collapsed, another mode paints, or graphics are off.
    /// While `None` the block keeps its two-line text summary.
    #[must_use]
    pub const fn rail_graph_rect(&self) -> Option<crate::layout::Rect> {
        self.rail_graph_rect
    }

    /// The runtime's report of the mini-DAG's placed rect after layout.
    pub const fn set_rail_graph_rect(&mut self, rect: Option<crate::layout::Rect>) {
        self.rail_graph_rect = rect;
    }

    /// Where the undocked third column landed this frame — `None` while
    /// the screen is too narrow to undock anything, or before the first
    /// frame. A wheel inside it moves that column's own rows rather than
    /// the stage's, the pane-under-the-pointer rule
    /// every multiplexer settled on.
    #[must_use]
    pub const fn undock_rect(&self) -> Option<crate::layout::Rect> {
        self.undock_rect
    }

    /// The runtime's report of the third column's placed rect after
    /// layout. Unlike the tech-tree's, this one feeds no layout of its
    /// own — the column's width is fixed and its window is the stage's —
    /// so the report costs no follow-up frame.
    pub const fn set_undock_rect(&mut self, rect: Option<crate::layout::Rect>) {
        self.undock_rect = rect;
    }

    /// Whether the rail sits on the right of the stage.
    ///
    /// The one arrangement of the split the surface decides: the
    /// design's Session view puts the turn on the left as the primary pane and the
    /// supervision rail on the right; a terminal is read from the top
    /// left and keeps its rail there. Same widget ids either way.
    #[must_use]
    pub const fn rail_on_right(&self) -> bool {
        self.surface.is_native()
    }

    /// The rail width a divider dragged to column `x` asks for — the
    /// column itself on the left, the columns right of it on the right
    /// (the divider keeps its own). `x` is [`Model::pointer_at`]'s
    /// reading, so a terminal's `col + 0.5` floors back to the column.
    #[must_use]
    pub fn rail_drag_width(&self, x: f32) -> u16 {
        let col = u16::try_from(x.floor().max(0.0) as u32).unwrap_or(u16::MAX);
        if self.rail_on_right() {
            self.size.cols.saturating_sub(col).saturating_sub(1)
        } else {
            col
        }
    }

    /// Where a mouse report puts the pointer, in cells — fractional.
    ///
    /// On a window the report carries the pointer's physical pixel and
    /// the model knows the cell it was declared (`Msg::CellSize`),
    /// so the answer is exact: `x / cell_width`, `y / cell_height`,
    /// which is what a drag and a pan need on a surface whose rows are
    /// not all one cell tall. Without a pixel — every terminal report —
    /// it is the cell's own centre, `col + 0.5`, which is the reading
    /// the tech-tree's hit test always took, so the terminal's
    /// arithmetic is unchanged by construction rather than by a golden.
    #[must_use]
    pub fn pointer_at(&self, event: &MouseEvent) -> (f32, f32) {
        match (event.px, self.cell_pixels()) {
            (Some((x, y)), Some((w, h))) if w > 0 && h > 0 => {
                (x as f32 / f32::from(w), y as f32 / f32::from(h))
            }
            _ => (f32::from(event.col) + 0.5, f32::from(event.row) + 0.5),
        }
    }

    /// The widget under the pointer — the hit registry's target on the
    /// last mouse move. How the launcher's session rows light up.
    #[must_use]
    pub fn hovered(&self) -> Option<WidgetId> {
        self.flight_panels
            .dragged()
            .map(|divider| divider.widget().id())
            .or(self.hovered)
    }

    /// The hover card the pointer has earned, if any.
    ///
    /// Four conditions, and each removes it *structurally* rather than
    /// by a caller remembering to ask. It is a **window** surface only,
    /// because a terminal has no overlay layer worth hanging a card on
    /// and its cell grid is the arrangement itself. It needs full
    /// motion, so `MAGISTR_MOTION=reduced` has no tooltip at all — the
    /// dwell is a timed reveal and a timed reveal with no tick would
    /// either never come or come instantly. It needs the **dwell**
    /// ([`Motion::hover_dwelt`](super::Motion::hover_dwelt)), so a
    /// pointer crossing the cockpit on its way somewhere leaves nothing
    /// behind it. And it needs a widget that is a *control*: an
    /// [`HoverRole::Inert`] panel is registered because the layout gives
    /// every node an id, not because it is something you can do
    /// anything to, and naming it would be describing the scaffolding.
    #[must_use]
    pub fn tooltip(&self) -> Option<Tooltip> {
        if (!self.surface.is_native() && !self.uses_flight_recorder())
            || !self.theme.motion().is_full()
        {
            return None;
        }
        if !self.motion.hover_dwelt(self.now()) {
            return None;
        }
        let id = self.hovered?;
        if HoverRole::of(id) == HoverRole::Inert {
            return None;
        }
        let text = Tooltip::label(id)?;
        let (col, row) = self.pointer?;
        (!text.is_empty()).then_some(Tooltip { text, col, row })
    }

    /// The widget the pointer is holding down, if any.
    ///
    /// Beside [`Model::hovered`] rather than folded into it: a press is
    /// a *second* state a widget can be in, and a control that is held
    /// has to look different from one that is merely under the pointer
    /// or the gesture has no feedback at all. Answered by the same one
    /// pass over the finished tree ([`hover::answer`](super::hover)), so
    /// "every hittable answers the button" is a property of the pass and
    /// not of forty call sites.
    #[must_use]
    pub const fn pressed(&self) -> Option<WidgetId> {
        self.pressed
    }

    /// The stage tab showing.
    #[must_use]
    pub const fn tab(&self) -> StageTab {
        self.tab
    }

    /// Start on `tab` — the roadmap entry point opens its own surface.
    #[must_use]
    pub const fn with_tab(mut self, tab: StageTab) -> Self {
        self.tab = tab;
        self.tab_explicit = true;
        self.place = Place::of_tab(tab);
        self
    }

    /// Start at `place` — for a window built to open somewhere other
    /// than where its tab would put it (a composition's own scene).
    #[must_use]
    pub const fn with_place(mut self, place: Place) -> Self {
        self.place = place;
        self.tab = place.tab();
        self.tab_explicit = true;
        self
    }

    /// The kind of screen this cockpit is being drawn on.
    ///
    /// [`view`](super::view::view) reads it to arrange the same widget
    /// ids for a window rather than for a cell grid. Nothing else in the
    /// cockpit reads it: `update` folds the same messages either way.
    #[must_use]
    pub const fn surface(&self) -> Surface {
        self.surface
    }

    /// The view drawn by this renderer. Shared navigation keeps the tab and
    /// place together; desktop-only places use their corresponding shared tab.
    #[must_use]
    pub const fn shown_tab(&self) -> StageTab {
        if self.surface.is_native() {
            self.place.tab()
        } else {
            self.tab
        }
    }

    /// Where the window's camera is — `None` on a terminal, which
    /// navigates by [`Model::tab`] and has no place.
    #[must_use]
    pub const fn place(&self) -> Option<Place> {
        if self.surface.is_native() {
            Some(self.place)
        } else {
            None
        }
    }

    /// The altitude path the window's header wears, derived from the
    /// [`Place`]:
    /// `projects` at the Galaxy; `projects › <project>` at the Sky;
    /// `projects › <project> › signals` at the Signals inbox, the
    /// project's second room; `projects › <project> › <chunk>` at the
    /// Session, the chunk being
    /// the rail's "you are here" — with no chunk named the project
    /// stays the here stop; `projects › <project> › <chunk> › <record>`
    /// at a record (the Changes journal, the Trace feed), the record
    /// the here stop and the chunk pressing back to the Session. The
    /// project is [`Model::title`] (the brand's own name while no
    /// session titled it). Empty on a terminal, which has no path.
    #[must_use]
    pub fn path(&self) -> Vec<PathStop> {
        let Some(place) = self.place() else {
            return Vec::new();
        };
        if place == Place::Galaxy {
            return vec![PathStop::here(Widget::CrumbProjects, "projects")];
        }
        let project = self.title().unwrap_or("magistr");
        let mut stops = vec![PathStop::back(Widget::CrumbProjects, "projects")];
        // A project being born (the setup interview): the here stop is
        // the flow's own word for where it stands — `new project ·
        // <folder>` while the ground is read, `new project · <name>`
        // through the interview, `<name> · Proposed roadmap` on the
        // charting, and the name alone at first light. F1 floats over
        // the Galaxy and has no path of its own.
        if place == Place::Birth {
            let birth = &self.birth;
            let name = birth.name();
            let here = match birth.moment() {
                BirthMoment::Ground => {
                    return vec![PathStop::here(Widget::CrumbProjects, "projects")];
                }
                BirthMoment::Survey => format!("new project \u{b7} {}", birth.dir()),
                BirthMoment::Interview => format!("new project \u{b7} {name}"),
                BirthMoment::Charting => format!("{name} \u{b7} Proposed roadmap"),
                BirthMoment::FirstLight => name.to_owned(),
            };
            stops.push(PathStop::here(Widget::CrumbProject, here));
            return stops;
        }
        // The project's second room (view 12): the inbox hangs off the
        // project and never off a session, so its path is three stops
        // whatever chunk the rail names — the project presses back to
        // the Sky, and `signals` is the here stop.
        if place == Place::Signals {
            stops.push(PathStop::back(Widget::CrumbProject, project));
            stops.push(PathStop::here(Widget::CrumbRecord, "signals"));
            return stops;
        }
        // A shipped star opened (view 09): the report hangs off the
        // project like the inbox does, and its here stop is the chunk
        // with the state its star wears — ` · Done`.
        if place == Place::Report {
            stops.push(PathStop::back(Widget::CrumbProject, project));
            let here = self.roadmap.selected().map_or_else(
                || "report".to_owned(),
                |chunk| format!("{} \u{b7} {}", chunk.id, StarState::Done.label()),
            );
            stops.push(PathStop::here(Widget::CrumbRecord, here));
            return stops;
        }
        let record = match place {
            Place::Changes => Some("changes"),
            Place::Trace => Some("trace"),
            Place::Galaxy
            | Place::Sky
            | Place::Signals
            | Place::Birth
            | Place::Session
            | Place::Report => None,
        };
        match (place, self.rail.chunk(), record) {
            (Place::Sky, _, _) | (_, None, None) => {
                stops.push(PathStop::here(Widget::CrumbProject, project));
            }
            (_, Some(chunk), None) => {
                stops.push(PathStop::back(Widget::CrumbProject, project));
                stops.push(PathStop::here(Widget::CrumbSession, chunk));
            }
            (_, chunk, Some(record)) => {
                stops.push(PathStop::back(Widget::CrumbProject, project));
                if let Some(chunk) = chunk {
                    stops.push(PathStop::back(Widget::CrumbSession, chunk));
                }
                stops.push(PathStop::here(Widget::CrumbRecord, record));
            }
        }
        // The started stop: at the Session, after the here stop, the
        // session named by when it started — a plain stop, not the
        // here one, as it appears on screen; its widget is the
        // session's, so pressing it holds. A record's path keeps the
        // chunk pressing back and no started stop, so the session
        // element id stays one per header.
        if place == Place::Session
            && let Some(started) = &self.session_started
        {
            stops.push(PathStop::back(
                Widget::CrumbSession,
                format!("session \u{b7} {started}"),
            ));
        }
        stops
    }

    /// Whether the native Sky owns launcher keyboard navigation.
    #[must_use]
    pub const fn is_launcher(&self) -> bool {
        self.surface.is_native() && matches!(self.place, Place::Sky)
    }

    /// Open the journal's desktop place or the terminal's unified session.
    pub(super) const fn show_changes(&mut self) {
        self.tab = StageTab::Session;
        self.tab_explicit = true;
        self.place = Place::Changes;
    }

    /// Keep a desktop destination and its shared view in agreement.
    pub(super) fn show_place(&mut self, place: Place) {
        self.show(place.tab());
        self.place = place;
    }

    /// Open a terminal tab or its corresponding desktop place.
    pub(super) fn show(&mut self, tab: StageTab) {
        self.tab = tab;
        self.tab_explicit = true;
        self.place = Place::of_tab(tab);
        if tab == StageTab::Roadmap && self.selected.is_none() {
            let selected = self
                .roadmap
                .selected()
                .or_else(|| self.roadmap.rows().first().copied())
                .map(|item| Selected::Chunk(item.id.clone()));
            if let Some(selected) = selected {
                self.inspect(selected);
            }
        }
        self.pane = Pane::Stage;
    }

    /// Draw this model for `surface`.
    ///
    /// Declared by whoever owns the screen — the native runtime for its
    /// window, nobody for a terminal, since [`Surface::Terminal`] is the
    /// default. A builder rather than a `Msg` because the answer is
    /// known before the first frame and never changes underneath a
    /// running cockpit, unlike the pixel probe's late reply.
    #[must_use]
    pub fn with_surface(mut self, surface: Surface) -> Self {
        self.set_surface(surface);
        self
    }

    /// Report the measured size of one cell without dispatching a command.
    ///
    /// Renderers may provide bounded layout feedback through this method.
    /// It changes image wrapping only; session and workflow state stay unchanged.
    pub fn report_cell_size(&mut self, cell: Option<CellSize>) {
        self.cell_pixels = cell.filter(|cell| cell.width > 0 && cell.height > 0);
        self.session.turn.set_cell_size(self.cell_size());
    }

    /// Declare the surface on a model already in hand.
    ///
    /// The `&mut` form of [`Model::with_surface`], for the renderer that
    /// owns the screen and is handed the model rather than building it.
    pub fn set_surface(&mut self, surface: Surface) {
        self.surface = surface;
        // A surface has its own room for a picture (see
        // `image_row_cap`), and the renderer declares itself late — on
        // its first frame, over a model that may already hold one.
        self.session.turn.set_row_cap(self.image_row_cap());
    }

    /// Rows a window keeps clear of a stream image: the header, the
    /// tab bar, the footer, the prompt, the card the picture belongs to
    /// and a few rows of the turn around it, so the tallest picture
    /// still shows what it is a picture *of*.
    pub const NATIVE_IMAGE_MARGIN_ROWS: u16 = 8;

    /// The tallest a stream image may stand, in rows, on this surface.
    ///
    /// A terminal keeps [`MAX_IMAGE_ROWS`]: a screenshot on a 24-row
    /// cockpit must not bury the turn, and the number is what every
    /// cell golden and byte-exact capture was measured on. A window has
    /// the room, so it lets a picture stand at most of its height —
    /// which is what shows a 720 px screenshot at its own size instead
    /// of squashed into 400 — and never under the terminal's allowance,
    /// whatever the window was shrunk to.
    #[must_use]
    pub fn image_row_cap(&self) -> u16 {
        match self.surface {
            Surface::Terminal => MAX_IMAGE_ROWS,
            Surface::Native => self
                .size
                .rows
                .saturating_sub(Self::NATIVE_IMAGE_MARGIN_ROWS)
                .max(MAX_IMAGE_ROWS),
        }
    }

    /// Show Session beside other views on wide terminals, without duplicating it.
    #[must_use]
    pub const fn undocked_tab(&self) -> Option<StageTab> {
        if !self.surface.is_native()
            || self.size.cols < UNDOCK_MIN_COLS
            || matches!(self.tab, StageTab::Session)
        {
            None
        } else {
            Some(StageTab::Session)
        }
    }

    /// Columns the undocked column and its divider take off the stage.
    const fn undock_cols(&self) -> u16 {
        if self.undocked_tab().is_some() {
            UNDOCK_COLS + 1
        } else {
            0
        }
    }

    /// Columns the stage wraps text to: the width less the stage's
    /// padding, the rail's columns and the undocked column.
    #[must_use]
    pub fn stage_width(&self) -> u16 {
        if self.uses_flight_recorder() {
            return ((self.flight_layout().left_width - 42.0).max(0.0)
                / f32::from(self.cell_size().width))
            .floor() as u16;
        }
        stage_width(self.size, self.rail.cols(self.size.cols))
            .saturating_sub(self.undock_cols())
            .saturating_sub(self.gutter_cells())
            .saturating_sub(if self.surface.is_native() || self.inspector_stacked() {
                0
            } else {
                Space::INSPECTOR_COLS + Space::GAP
            })
    }

    /// Monospaced content width; the pixel conversation has no terminal accent gutter.
    /// Paragraphs in the Flight Recorder use the separate shared glyph measure.
    #[must_use]
    pub fn turn_width(&self) -> u16 {
        if self.surface.is_native()
            && let Some((cols, _)) = self.flight_panels.native_stage
        {
            return cols;
        }
        self.stage_width()
            .saturating_sub(if self.uses_flight_recorder() {
                0
            } else {
                Accent::GUTTER_COLS
            })
    }

    /// Columns the surface holds MONO turn content to, when its own box
    /// is narrower than the prose measure — `None` on a cell surface.
    #[must_use]
    pub const fn mono_cols(&self) -> Option<u16> {
        self.mono_cols
    }

    /// Report the columns the surface's turn column holds in MONO
    /// cells, and rewrap the turn's code, tool heads and diffs to them.
    ///
    /// The window measures its turn column in pixels and draws two
    /// faces inside it; see [`Turn::set_mono_width`](crate::turn::Turn::set_mono_width)
    /// for why one column count cannot serve both. The renderer lays
    /// the column out, counts what its mono cell holds and tells the
    /// model — the direction the sky field's rect already reports in.
    pub fn set_mono_cols(&mut self, cols: Option<u16>) {
        if self.mono_cols == cols {
            return;
        }
        let anchor = self.stage_anchor();
        self.mono_cols = cols;
        self.session.turn.set_mono_width(cols);
        if let Some(anchor) = anchor {
            self.hold_stage_anchor(anchor);
        }
    }

    /// Rows the chrome takes around the stage, on the surface this
    /// cockpit is drawn for: a terminal answers the count it always
    /// did; a window answers the breadcrumb header, the cap row, the
    /// composer, its margin and the bottom bar — with no tab bar, no
    /// panel ring, no prompt line, no phase strip and no footer in it.
    /// Every viewport window, the turn's wrap and the
    /// undock column's row identity are built on this one count.
    #[must_use]
    pub const fn chrome_rows(&self) -> u16 {
        if self.surface.is_native() {
            NATIVE_CHROME_ROWS
        } else {
            CHROME_ROWS + self.gutter_cells() + if self.banner.is_some() { 1 } else { 0 }
                - if self.pixels && self.size.rows < 30 {
                    2 * Space::PAD
                } else {
                    0
                }
        }
    }

    /// Cells the pixel arrangement adds on each axis: the one-cell
    /// gutter around the whole cockpit and the padding inside the stage
    /// panel, top and bottom (or left and right) — four in all, and
    /// none on a plain terminal or a window. Every row and column count
    /// the stage is measured by comes through here, so what the model
    /// believes fits is what the layout actually shows.
    const fn gutter_cells(&self) -> u16 {
        if self.pixels && !self.surface.is_native() {
            4 * Space::PAD
        } else {
            0
        }
    }

    /// Rows the stage shows: the height less the chrome, the dock, the
    /// prompt box's growth past its first row and the completion popup.
    /// The dock and the growing prompt box are terminal chrome — a
    /// window lays out no dock strip (a pending decision becomes a
    /// Moment, ) and its composer pill holds one sliding cursor
    /// line at a fixed height, so neither takes rows there. The
    /// terminal dock's rows come off both surfaces:
    /// the feed keeps its tail above the raised screen.
    #[must_use]
    pub fn stage_rows(&self) -> usize {
        if self.surface.is_native()
            && let Some((_, rows)) = self.flight_panels.native_stage
        {
            return rows;
        }
        if self.uses_flight_recorder() {
            return self.flight_layout().conversation_rows(self);
        }
        self.body_stage_rows()
            .saturating_sub(usize::from(self.inspector_stack_rows()))
    }

    fn body_stage_rows(&self) -> usize {
        let (dock, grown) = if self.surface.is_native() {
            (0, 0)
        } else {
            let dock = if self.pixels && self.dock.is_empty() {
                0
            } else {
                self.dock.rows()
            };
            let composer = if self.pixels {
                if self.size.rows >= 30 { 6 } else { 3 }
            } else {
                0
            };
            (dock, self.prompt_rows() - 1 + composer)
        };
        let taken = dock
            .saturating_add(grown)
            .saturating_add(self.term_dock.taken(self.now()))
            .saturating_add(
                self.session
                    .completion
                    .as_ref()
                    .map_or(0, |c| c.items().len()),
            );
        let taken = u16::try_from(taken).unwrap_or(u16::MAX);
        self.size
            .rows
            .saturating_sub(self.chrome_rows().saturating_add(taken)) as usize
    }

    /// The scroll range uses physical pixels in the graphics conversation.
    #[must_use]
    pub fn conversation_window(&self) -> (usize, usize) {
        if self.uses_flight_recorder() {
            (
                self.session.turn.conversation().height(),
                self.conversation_height(),
            )
        } else {
            (self.session.turn.len(), self.stage_rows())
        }
    }

    /// Physical height of the conversation beneath its objective and panel heading.
    #[must_use]
    pub fn conversation_height(&self) -> usize {
        let layout = self.flight_layout();
        let line = f32::from(self.cell_size().height);
        let heading = (line + 14.0).max(34.0)
            + 26.0
            + line
            + 8.0
            + if layout.conversation_height >= 380.0 {
                line + 4.0
            } else {
                0.0
            };
        (layout.conversation_height - heading).max(0.0).floor() as usize
    }

    /// Position of one source row in the current scroll coordinate system.
    #[must_use]
    pub fn conversation_row_offset(&self, row: usize) -> usize {
        if self.uses_flight_recorder() {
            self.session.turn.conversation().offset(row)
        } else {
            row
        }
    }

    /// Bring a source row to the top of the conversation, clamped at the tail.
    /// Keeps selection and the message draft intact; later appends preserve this
    /// source anchor through the same viewport used by mouse and keyboard scrolling.
    pub fn reveal_conversation(&mut self, row: usize) -> bool {
        if row >= self.session.turn.len() {
            return false;
        }
        let (total, height) = self.conversation_window();
        let destination = self.conversation_row_offset(row);
        let origin = self.session.viewport.top(total, height);
        let delta = isize::try_from(destination)
            .unwrap_or(isize::MAX)
            .saturating_sub(isize::try_from(origin).unwrap_or(isize::MAX));
        self.session
            .viewport
            .scroll_by(delta, total, height, self.now(), false);
        true
    }

    /// Distance covered by a wheel notch in the current scroll coordinate system.
    #[must_use]
    pub fn conversation_wheel(&self) -> isize {
        let scale = if self.uses_flight_recorder() {
            usize::from(self.cell_size().height)
        } else {
            1
        };
        (Viewport::WHEEL * scale) as isize
    }

    /// Where the next scroll begins after an automatically aligned tail.
    #[must_use]
    pub(super) fn conversation_scroll_origin(&self) -> usize {
        let (total, height) = self.conversation_window();
        if self.session.viewport.is_following() && !self.session.viewport.is_gliding(self.now()) {
            self.conversation_top() as usize
        } else {
            self.session.viewport.top(total, height)
        }
    }

    /// Physical top, interpolated only by model motion ticks.
    #[must_use]
    pub fn conversation_top(&self) -> f32 {
        let (total, height) = self.conversation_window();
        let top = self.session.viewport.top_between(total, height, self.now());
        if self.uses_flight_recorder()
            && self.session.viewport.is_following()
            && !self.session.viewport.is_gliding(self.now())
        {
            let geometry = self.session.turn.conversation();
            if let Some(row) = geometry.rows().get(geometry.row_at(top.floor() as usize))
                && (row.top as f32) < top
                && row.height < height
            {
                // A settled tail starts with a complete line or card. Keep
                // fractional clipping while the user scrolls, and retain the
                // unmodified source anchors when the viewport is pinned.
                return (row.top + row.height) as f32;
            }
        }
        top
    }

    /// The first visible row of the turn — the row the *paint* starts
    /// at, so a scroll still gliding shows where it came from. A cell
    /// surface reads the row held for its tick ([`Viewport::top_at`]);
    /// a window reads the whole part of the row the paint is between
    /// ([`Viewport::top_between`]), the fraction riding
    /// [`Model::stage_scroll_fraction`].
    #[must_use]
    pub fn top_row(&self) -> usize {
        if self.uses_flight_recorder() {
            return self
                .session
                .turn
                .conversation()
                .row_at(self.conversation_top().floor() as usize);
        }
        let (total, height) = (self.session.turn.len(), self.stage_rows());
        if self.surface.is_native() {
            let between = self.session.viewport.top_between(total, height, self.now());
            return between.floor() as usize;
        }
        self.session.viewport.top_at(total, height, self.now())
    }

    /// How far into [`Model::top_row`] the window's paint sits while a
    /// scroll glides, as a fraction of a row in `0..1` — what a window
    /// shifts its feed by to paint *between* rows
    /// (stage-scroll-anchor-and-rate). Zero on a cell surface, and zero
    /// the moment the glide is over, so a settled frame is the frame it
    /// always was.
    #[must_use]
    pub fn stage_scroll_fraction(&self) -> f32 {
        if !self.surface.is_native() {
            return 0.0;
        }
        let between = self.session.viewport.top_between(
            self.session.turn.len(),
            self.stage_rows(),
            self.now(),
        );
        between - between.floor()
    }

    /// The graph leaves at least two selectable rows on short stages.
    #[must_use]
    pub fn trace_graph_rows(&self) -> u16 {
        let rows = self.trace.pinned_rows(self.pixels).saturating_sub(1);
        rows.min(self.stage_rows().saturating_sub(3)) as u16
    }

    /// The Session timeline's scrolling body: how many rows it has and how many
    /// of them fit under the pinned heading (and branch graph). The
    /// window the view paints and the one the scroll keys move are this
    /// same pair.
    #[must_use]
    pub fn session_window(&self) -> (usize, usize) {
        if self.place() == Some(Place::Changes) {
            return (
                self.changes.entries().len().max(1),
                self.stage_rows().max(1),
            );
        }
        if self.uses_flight_recorder() {
            let layout = self.flight_layout();
            let height = if layout.compact {
                layout.body_height
            } else {
                layout.detail_height
            };
            return (
                super::session_item::SessionItem::collect(&self.trace, &self.changes)
                    .len()
                    .max(1),
                super::FlightRecords::shown(self, height, false),
            );
        }
        let height = self
            .stage_rows()
            .saturating_sub(1 + usize::from(self.trace_graph_rows()));
        (
            super::session_item::SessionItem::collect(&self.trace, &self.changes)
                .len()
                .max(1),
            height,
        )
    }

    /// Columns available to the Session timeline: the undocked
    /// column's inner width while the trace sits there, else the stage's
    /// width (which already gives up the pixel arrangement's gutters).
    #[must_use]
    pub fn trace_width(&self) -> u16 {
        if self.undocked_tab() == Some(StageTab::Session) {
            return UNDOCK_COLS.saturating_sub(2 * Space::PAD);
        }
        self.stage_width()
    }

    /// The first visible row of the Session timeline.
    #[must_use]
    pub fn session_top(&self) -> usize {
        let (total, height) = self.session_window();
        self.session_viewport.top_at(total, height, self.now())
    }

    /// The accelerator sheet's scrolling body: how many rows the
    /// registry draws and how many of them fit under the sheet's pinned
    /// heading.
    ///
    /// The total is [`Binding::sheet_rows`] — derived from the registry,
    /// so the window can never disagree with what the sheet draws, and a
    /// binding added to `Binding::ALL` widens the scroll rather than
    /// falling off the bottom of it.
    #[must_use]
    pub fn sheet_window(&self) -> (usize, usize) {
        (Binding::sheet_rows(), self.stage_rows().saturating_sub(1))
    }

    /// The first visible row of the accelerator sheet's body.
    #[must_use]
    pub fn sheet_top(&self) -> usize {
        let (total, height) = self.sheet_window();
        self.sheet_viewport.top_at(total, height, self.now())
    }

    /// Rows reserved for the graph beside the table, or above it on a
    /// narrow terminal. Text-only mode needs just its workstream legend
    /// when stacked. The pinned block also anchors the pixel surface.
    #[must_use]
    pub fn roadmap_graph_rows(&self) -> u16 {
        if self.surface.is_native() {
            return u16::try_from(self.stage_rows()).unwrap_or(u16::MAX);
        }
        if self.roadmap.is_empty() {
            return 0;
        }
        let rows = self
            .stage_rows()
            .saturating_sub(self.roadmap_heading_rows());
        if self.roadmap_side_by_side() {
            return u16::try_from(rows).unwrap_or(u16::MAX);
        }
        // Leave the filter, column header and at least one selectable row.
        let available = rows.saturating_sub(3);
        let graph = if self.has_pixels() {
            (rows / 3).clamp(2, 10)
        } else {
            self.roadmap.band_rows().len()
        };
        u16::try_from(graph.min(available)).unwrap_or(0)
    }

    /// On a short stage the rail supplies the summary; keep rows for the actual work.
    #[must_use]
    pub fn roadmap_heading_rows(&self) -> usize {
        usize::from(self.stage_rows() > 6)
    }

    /// Room for a useful graph beside all four table columns.
    #[must_use]
    pub fn roadmap_side_by_side(&self) -> bool {
        !self.uses_flight_recorder()
            && !self.surface.is_native()
            && self.size.cols >= 132
            && self.stage_width() >= 58
    }

    /// The list owns two thirds of a wide stage; both allocations share this value.
    #[must_use]
    pub fn roadmap_table_width(&self) -> u16 {
        if self.uses_flight_recorder() {
            let l = self.flight_layout();
            let width = l.left_width;
            return ((width - 34.0).max(0.0) / f32::from(self.cell_size().width)).floor() as u16;
        }
        if self.roadmap_side_by_side() {
            self.stage_width().saturating_sub(Space::GAP) * 2 / 3
        } else {
            self.stage_width()
        }
    }

    /// The Roadmap tab's scrolling body: how many rows it has and how
    /// many of them fit under the pinned heading and tech-tree block.
    /// The window the view paints and the one the scroll keys and the
    /// wheel move are this same pair.
    #[must_use]
    pub fn roadmap_window(&self) -> (usize, usize) {
        if self.surface.is_native()
            && let Some(rows) = self.flight_panels.native_roadmap_rows
        {
            return (
                self.roadmap.rows().len() + self.roadmap.signals().len(),
                rows,
            );
        }
        if self.uses_flight_recorder() {
            let height = super::FlightRoadmapPage::list_height(self);
            return (
                self.roadmap.rows().len() + self.roadmap.signals().len(),
                super::FlightRecords::shown(self, height, true),
            );
        }
        let graph = if self.roadmap_side_by_side() {
            0
        } else {
            usize::from(self.roadmap_graph_rows())
        };
        let height = self
            .stage_rows()
            .saturating_sub(self.roadmap_heading_rows() + 2 + graph);
        (self.roadmap.body_rows(self.roadmap_table_width()), height)
    }

    /// The first visible row of the Roadmap tab's body.
    #[must_use]
    pub fn roadmap_top(&self) -> usize {
        let (total, height) = self.roadmap_window();
        self.roadmap_viewport.top_at(total, height, self.now())
    }

    /// The rows on the stage: the viewport's window onto the turn — one
    /// row more while a window's paint sits between rows
    /// ([`Model::stage_scroll_fraction`]), since a fraction of the first
    /// row scrolled off the top is a fraction of one more row on at the
    /// bottom.
    #[must_use]
    pub fn visible_rows(&self) -> &[Row] {
        let rows = self.session.turn.rows();
        let top = self.top_row().min(rows.len());
        if self.uses_flight_recorder() {
            let end = self.conversation_top().ceil() as usize + self.conversation_height();
            let bottom = self
                .session
                .turn
                .conversation()
                .rows()
                .partition_point(|row| row.top < end)
                .min(rows.len());
            return &rows[top..bottom.max(top)];
        }
        let between = usize::from(self.stage_scroll_fraction() > 0.0);
        let bottom = top
            .saturating_add(self.stage_rows())
            .saturating_add(between)
            .min(rows.len());
        &rows[top..bottom]
    }

    /// Frames drawn so far (`FrameDue` messages received).
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// How long one animation step lasts — the runtime's tick while
    /// anything that reads [`Model::ticks`] moves. It is
    /// [`Animation::Motion`]'s period, and it is the finest any
    /// consumer declares, which is what lets every tick-derived motion
    /// read one tick as this long however coarse the rest of the
    /// declared set is.
    pub const TICK_PERIOD: Duration = Animation::Motion.period();

    /// How long one quarter turn of a spinner lasts: four of these turn
    /// it once around.
    pub const SPIN_PERIOD: Duration = Duration::from_millis(120);

    /// Animation steps per spinner frame: a spinner turns a quarter every
    /// [`Model::SPIN_PERIOD`] on a tick of [`Model::TICK_PERIOD`].
    pub const SPIN_STEPS: u64 =
        (Self::SPIN_PERIOD.as_millis() / Self::TICK_PERIOD.as_millis()) as u64;

    /// The graphics workspace's leading indicator, including quiet waits
    /// between agents. Pauses, completion and permission requests are still.
    #[must_use]
    pub fn has_activity_motion(&self) -> bool {
        self.uses_flight_recorder() && self.theme.motion().is_full() && self.agent_activity().active
    }

    /// Everything on screen that moves right now, each named with the
    /// period it wants steps at; the runtime reconciles the set to one
    /// interval ([`Animations::period`]).
    ///
    /// [`Animation::Motion`] is declared by everything that reads
    /// [`Model::ticks`]: the turn streaming (the header's spinner and
    /// the shimmer on the streaming block), a gate LED still running, a
    /// fill, transient or sweep in the motion layer still settling, a
    /// viewport still gliding, or the tech-tree's camera still moving
    /// onto a focused chunk.
    ///
    /// The active chunk's glow is deliberately *not* on that list. Its
    /// wave is a pure function of [`Model::ticks`] like everything else
    /// here, but it never asks for a tick of its own: it breathes
    /// whenever the cockpit is already moving and holds its phase when
    /// nothing is, which is what keeps an idle Roadmap tab at zero
    /// frames a second. [`Animation::Clock`] is declared by the
    /// two clocks — the header's `mm:ss` and the loop status's elapsed —
    /// while a turn streams or a loop is still running; they read the
    /// wall clock rather than the tick count, so a second is all the
    /// resolution they need and a quiet minute no longer shows a stale
    /// time.
    ///
    /// Under reduced motion the set is empty, always — the tick never
    /// starts, so every surface draws its final frame and the clocks
    /// advance only when other news forces a frame.
    #[must_use]
    pub fn animations(&self) -> Animations {
        // The held kill is a timer the reader's finger drives, not
        // decoration: without its tick a hold could
        // never complete, so reduced motion keeps this one animation
        // and nothing else.
        let holding = self.hold.is_some();
        if !self.theme.motion().is_full() {
            return Animations::NONE.with_if(Animation::Motion, holding);
        }
        let streaming = matches!(self.session.state, TurnState::Streaming);
        Animations::NONE
            .with_if(
                Animation::Motion,
                streaming
                    || self.has_activity_motion()
                    || holding
                    || self.has_running_gate()
                    || self.motion.is_settling(self.now())
                    // The terminal dock rising or lowering: bounded, seven steps either way.
                    || self.term_dock.is_settling(self.now())
                    || self.is_scrolling()
                    || self.roadmap.is_gliding(self.now())
                    // The Brief's deal, and the sky's countdown into
                    // rest. Both are bounded: the deal ends when the
                    // last card lands, and the countdown ends at rest —
                    // after which a window on a settled sky is back to
                    // zero frames a second, which is the whole point of
                    // rest being a *state* and not a loop.
                    || self.brief_dealing()
                    || (self.is_sky() && !self.sky_at_rest())
                    // The window breathes while it works — a tool running
                    // for minutes, a thought being thought, the loop
                    // between agents. Plain terminals keep their clock-only
                    // cadence; graphics terminals declare their activity
                    // indicator above.
                    || (self.surface == Surface::Native && self.is_working())
                    // A project being born: the survey's sweep runs while its rows
                    // land, the charting while its stars are born —
                    // both bounded, both the window's alone.
                    || self.birth_moving()
                    // Live project clusters animate while any project has a running session.
                    // This tick is native-only and stops when no project is active.
                    || self.galaxy_breathing(),
            )
            .with_if(Animation::Clock, streaming || self.has_running_loop())
    }

    /// Whether `animation` is one the cockpit declares now — what a
    /// surface asks before drawing a frame of it, so a spinner stops
    /// dead under reduced motion and never turns on a clock's tick.
    #[must_use]
    pub fn animates(&self, animation: Animation) -> bool {
        self.animations().contains(animation)
    }

    /// Whether anything on screen animates, so the runtime should keep
    /// its tick running.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        !self.animations().is_empty()
    }

    /// Whether the window is at the Galaxy with a project that is live
    /// — a session running or a question pending — whose cluster
    /// breathes, so the field asks for the tick the breath needs.
    fn galaxy_breathing(&self) -> bool {
        self.surface.is_native()
            && self.place == Place::Galaxy
            && self.galaxy.projects().iter().any(GalaxyProject::is_live)
    }

    /// Whether this is a screen that waits on the human and therefore
    /// moves nothing:
    /// a Moment or a review up, a bad day's cards, the search float,
    /// the settings surface, a chapter, the inbox, and Birth's ground,
    /// interview and first light — the moments of the flow that ask
    /// rather than work. The caret would be the one motion such a
    /// screen keeps ([`MotionName::Caret`]); nothing blinks it yet.
    fn holds_still(&self) -> bool {
        self.moment().is_some()
            || self.review().is_some()
            || self.ask().is_some()
            || self.bad_day().is_some()
            || self.search.is_some()
            || self.settings.is_some()
            || matches!(self.place(), Some(Place::Report | Place::Signals))
            || (self.place() == Some(Place::Birth)
                && matches!(
                    self.birth.moment(),
                    BirthMoment::Ground | BirthMoment::Interview | BirthMoment::FirstLight
                ))
    }

    /// Motion active at this tick, with each [`MotionName`] listed once.
    ///
    /// The registry derives names from streaming, running work, transient effects,
    /// scrolling, navigation, and completion state. Ambient effects are included
    /// only while another animation keeps ticks running; twinkle is dark-mode only.
    /// Decision screens and reduced motion return an empty set.
    #[must_use]
    pub fn motions_in_flight(&self) -> Vec<MotionName> {
        if !self.theme.motion().is_full() || self.holds_still() {
            return Vec::new();
        }
        let now = self.now();
        let native = self.surface.is_native();
        let night = self.theme.mode() == ThemeMode::Dark;
        let streaming = matches!(self.session.state, TurnState::Streaming);
        let mut set: Vec<MotionName> = Vec::new();
        let mut add = |name: MotionName, moving: bool| {
            if moving && !set.contains(&name) {
                set.push(name);
            }
        };
        add(MotionName::Sweep, streaming);
        add(
            MotionName::Breathe,
            if native {
                self.is_working()
            } else {
                streaming || self.has_running_gate() || self.has_activity_motion()
            },
        );
        for name in self.motion.in_flight(now) {
            add(name, true);
        }
        add(
            MotionName::Parallax,
            self.is_scrolling() || self.roadmap.is_panning(now),
        );
        // The dock rising from the composer's edge is a rise,
        // the Brief's cards being dealt the other.
        add(
            MotionName::Rise,
            self.brief_dealing() || self.term_dock.is_settling(now),
        );
        if native && self.place == Place::Birth {
            match self.birth.moment() {
                BirthMoment::Survey => {
                    add(MotionName::Radar, true);
                    let rows = self.birth.survey().map_or(0, |report| report.rows());
                    add(
                        MotionName::Rise,
                        (0..rows).any(|index| self.birth.row_rise(index, now, true).is_some()),
                    );
                }
                BirthMoment::Charting => add(MotionName::Born, self.birth.is_being_born(now)),
                BirthMoment::Ground | BirthMoment::Interview | BirthMoment::FirstLight => {}
            }
        }
        if let Some(run) = self.roadmap.ship_run(now) {
            add(MotionName::Ignite, run.ignite < 1.0);
            add(MotionName::EdgeRun, run.ignite >= 1.0);
        }
        add(MotionName::ChromeFade, self.sky_chrome_fade().is_some());
        if self.animates(Animation::Motion) {
            let sky = self.is_sky();
            // The terminal's tech-tree carries the active chunk's glow
            // on the same wave the sky's working star breathes on.
            if sky || (!native && self.tab == StageTab::Roadmap) {
                let nodes = self.roadmap.dag_nodes();
                add(
                    MotionName::Breathe,
                    nodes.iter().any(|node| node.state.breathes()),
                );
                if sky {
                    add(MotionName::Skeleton, nodes.is_empty());
                    add(
                        MotionName::Twinkle,
                        night && nodes.iter().any(|node| node.state.twinkles()),
                    );
                    add(
                        MotionName::ReadyPulse,
                        nodes.iter().any(|node| node.state == StarState::Ready),
                    );
                }
            }
            if native && self.place == Place::Galaxy {
                let projects = self.galaxy.projects();
                add(
                    MotionName::Breathe,
                    projects.iter().any(GalaxyProject::is_live),
                );
                add(
                    MotionName::Twinkle,
                    night
                        && projects
                            .iter()
                            .any(|project| project.done.is_some_and(|n| n > 0)),
                );
            }
        }
        MotionName::ALL
            .into_iter()
            .filter(|name| set.contains(name))
            .collect()
    }

    /// Whether `name` is in flight at this tick — what a surface asks
    /// before drawing a frame of it, so the window and the registry
    /// cannot disagree about what moves.
    #[must_use]
    pub fn moves(&self, name: MotionName) -> bool {
        self.motions_in_flight().contains(&name)
    }

    /// The one motion that leads this screen — the news ahead of the texture, the ship
    /// moment ahead of everything — or `None` for a screen that holds
    /// still.
    #[must_use]
    pub fn motion_hero(&self) -> Option<MotionName> {
        MotionName::hero(self.motions_in_flight())
    }

    /// Whether a loop is running: one was started and has not ended.
    /// Paused counts — the elapsed clock keeps counting through a pause.
    #[must_use]
    pub fn has_running_loop(&self) -> bool {
        self.loop_status
            .as_ref()
            .is_some_and(|status| status.ended().is_none())
    }

    /// Whether work is in flight: a streaming turn, a running gate, or a loop
    /// connecting to or judging an agent while neither paused nor ended.
    /// A disconnected feed during the gap between agents is not itself a fault.
    #[must_use]
    pub fn is_working(&self) -> bool {
        self.roadmap_request_pending()
            || matches!(self.session.state, TurnState::Streaming)
            || self.has_running_gate()
            || self
                .loop_status
                .as_ref()
                .is_some_and(|status| status.ended().is_none() && !status.is_paused())
    }

    /// Whether any of the five viewports is still catching up with a
    /// scroll. Each glide is a pure function of
    /// [`Model::ticks`] like the rest of the motion layer, so this is
    /// what keeps the tick alive for exactly as long as one runs.
    /// Whether an independently scrolling surface still has a curve in flight.
    #[must_use]
    pub fn is_scrolling(&self) -> bool {
        [
            self.session.viewport,
            self.session_viewport,
            self.roadmap_viewport,
            self.sheet_viewport,
        ]
        .iter()
        .any(|viewport| viewport.is_gliding(self.now()))
    }

    /// Whether a gate LED on the status strip is still waiting for its
    /// job.
    #[must_use]
    pub fn has_running_gate(&self) -> bool {
        self.loop_status
            .as_ref()
            .is_some_and(|status| status.gates().iter().any(|gate| gate.passed().is_none()))
    }

    /// The motion layer's state: the eased fills and the transitions in
    /// flight, read against [`Model::ticks`].
    #[must_use]
    pub const fn motion(&self) -> &Motion {
        &self.motion
    }

    /// What the motion layer watches for change across a fold.
    #[must_use]
    pub(super) fn motion_key(&self) -> MotionKey {
        let tasks = self.rail.ship().tasks();
        MotionKey {
            tab: self.shown_tab(),
            dock: self.dock.len(),
            rail: self.rail.progress(),
            roadmap: (self.roadmap.done_count(), self.roadmap.active_count()),
            rows: self.session.turn.rows().len(),
            gates: self.loop_status.as_ref().map_or(0, |status| {
                status
                    .gates()
                    .iter()
                    .filter(|gate| gate.passed().is_some())
                    .count()
            }),
            tasks: (
                tasks
                    .iter()
                    .filter(|task| task.state == TaskState::Completed)
                    .count(),
                tasks
                    .iter()
                    .rposition(|task| task.state == TaskState::Completed)
                    .unwrap_or(0),
            ),
            trace: self.trace.steps().len(),
            // Whichever series the monitor is plotting (`Model::monitor`),
            // capped at the slot's width on purpose: past that the header
            // shows the newest `SPARK_COLS` samples, so a ninth sample
            // changes what the line *says* but not how much of the slot
            // is drawn — and only the latter is a sweep. See
            // `Motion::spark_sweep`.
            activity: self.monitor().len().min(usize::from(view::SPARK_COLS)),
            // A window only: see `MotionKey::hover`.
            hover: match self.surface {
                Surface::Native => self.hovered,
                Surface::Terminal => None,
            },
            // A window only, for the same reason: the chrome fade is a
            // window's motion, and a terminal that never stamps it keeps
            // its idle guarantee by construction.
            rail_open: match self.surface {
                Surface::Native => Some(!self.rail.is_collapsed()),
                Surface::Terminal => None,
            },
        }
    }

    /// The spinner's frame at this step: it turns a quarter every
    /// [`Model::SPIN_PERIOD`], so every spinner on screen — the
    /// header's, a tool call's, a gate LED's — is on the same frame.
    #[must_use]
    pub const fn spin_step(&self) -> u64 {
        self.ticks / Self::SPIN_STEPS
    }

    /// Animation steps taken so far (`Tick` messages received) — what a
    /// spinner picks its frame by, so `view` stays a pure function of
    /// the model rather than of the wall clock.
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.ticks
    }

    /// The motion clock: animation steps as a **rational** number,
    /// `ticks + phase`.
    ///
    /// Every curve in the motion layer is sampled here rather than at
    /// [`Model::ticks`], which is the whole of "re-timed for a display
    /// rather than a tick": the tick still decides *when a fold's stamp
    /// is taken* and how long a transient lasts, and the phase decides
    /// where between two ticks the frame being drawn sits. Graphics
    /// terminals and windows fold [`Msg::Phase`](super::Msg::Phase)
    /// between ticks for smooth pixel motion. Plain terminals leave
    /// the phase at zero and retain their existing cell-motion timing.
    ///
    /// Deliberately *not* what [`Model::spin_step`] reads: a spinner is
    /// a sequence of four glyphs and a cell has no half frame, so
    /// sampling it faster would only quantize back to the same glyph.
    #[must_use]
    pub fn now(&self) -> f32 {
        self.ticks as f32 + self.tick_phase
    }

    /// Inputs received so far: every key, mouse report and paste, whether
    /// or not the ribbon kept it.
    #[must_use]
    pub const fn inputs(&self) -> u64 {
        self.inputs
    }

    /// Whether `update` has answered with [`Cmd::Quit`](super::Cmd::Quit).
    #[must_use]
    pub const fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Whether the terminal window has focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// The surface's observed background color.
    #[must_use]
    pub const fn background(&self) -> Option<Color> {
        self.background
    }

    /// Whether this surface can report release, so a held action can be
    /// cancelled when the user lets go. Native windows always can.
    #[must_use]
    pub fn reports_key_releases(&self) -> bool {
        self.surface.is_native() || self.key_releases
    }

    /// The held kill, while one is held.
    #[must_use]
    pub const fn term_hold(&self) -> Option<&TermHold> {
        self.hold.as_ref()
    }

    /// How far the head of the card on terminal `id` has filled at this
    /// frame, while a hold targets it — what both painters read.
    #[must_use]
    pub fn hold_fraction(&self, id: &str) -> Option<f32> {
        let hold = self.hold.as_ref()?;
        (hold.target().id() == id).then(|| hold.fraction(self.now()))
    }

    /// The fill on the card whose head is the turn's absolute row `at`,
    /// while a hold targets that card's program.
    #[must_use]
    pub fn hold_at_row(&self, at: usize) -> Option<f32> {
        let (line, _) = self.session.turn.card_at_row(at)?;
        self.hold_fraction(line.record().terminal.as_deref()?)
    }

    /// What a held kill would reach right now: the running card's
    /// program — the newest card naming a terminal whose screen exists
    /// and has not ended — as the agent's terminal or a gate's child.
    /// `None` when nothing is running on a pane.
    #[must_use]
    pub fn kill_target(&self) -> Option<KillCmd> {
        let id = self.session.turn.running_terminal()?;
        let term = self.terminals.term(id)?;
        if term.is_finished() {
            return None;
        }
        Some(if self.session.turn.terminal_is_gate(id) {
            KillCmd::Gate(id.to_owned())
        } else {
            KillCmd::Terminal(id.to_owned())
        })
    }

    /// The one-line plain-text banner naming a graphics-capable
    /// terminal, once the runtime decided [`Mode::MinimalText`]; `None`
    /// with pixels (see [`crate::capability::Mode::banner`]).
    ///
    /// [`Mode::MinimalText`]: crate::capability::Mode::MinimalText
    #[must_use]
    pub const fn banner(&self) -> Option<&'static str> {
        self.banner
    }

    /// One cell's measured width and height in pixels, supplied by the surface.
    #[must_use]
    pub fn cell_pixels(&self) -> Option<(u16, u16)> {
        self.cell_pixels.map(|cell| (cell.width, cell.height))
    }

    /// The measured cell size, or the shared initial estimate.
    #[must_use]
    pub fn cell_size(&self) -> CellSize {
        self.cell_pixels.unwrap_or(CellSize::DEFAULT)
    }

    /// Whether the pixel layer is live: the startup probe found a
    /// graphics protocol ([`Msg::Mode`]), so the driver composites pixel
    /// surfaces over the cells. The view reads it to stand its cell
    /// drawings down — the brand glyph, the header's block sparkline,
    /// the rail's `▰▱` bar — for the surface that covers them.
    #[must_use]
    pub const fn has_pixels(&self) -> bool {
        self.pixels
    }

    /// Whether the terminal renders kitty's text sizing protocol
    /// ([`Msg::TextSizing`], from the startup probe's handshake): a run
    /// of text can be scaled across a block of cells, which is what a
    /// headline is made of.
    ///
    /// Independent of [`Model::has_pixels`] on purpose — a sixel
    /// terminal has graphics and no text sizing, and kitty 0.39 had the
    /// graphics protocol before it had this one.
    #[must_use]
    pub const fn has_text_sizing(&self) -> bool {
        self.text_sizing
    }

    /// Whether the terminal answers kitty's pointer-shape protocol: the
    /// startup probe found the *kitty* graphics protocol ([`Msg::Mode`]),
    /// which is the same terminal capability. The driver reads it before
    /// writing an OSC 22 — sixel and plain-text sessions get the hover
    /// colour alone rather than a string they would print.
    #[must_use]
    pub const fn has_pointer_shapes(&self) -> bool {
        self.pointer_shapes
    }

    /// How busy each of the last [`ACTIVITY`] *busy* frames was: one
    /// count of agent events and engine frames per painted frame that
    /// carried any, oldest first. The header's sparkline is this series.
    ///
    /// A painted frame is the honest bucket here — the cockpit has no
    /// clock of its own (its tick runs only while something animates),
    /// and it paints when something happens. Quiet frames are not
    /// recorded at all, so the ribbon is the recent shape of the work
    /// rather than a timeline with a tail of zeros — and a cockpit that
    /// is merely being looked at leaves the sparkline, and the pixel
    /// surface behind it, completely still.
    pub fn activity(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.activity.iter().copied()
    }

    /// What each of the last [`ACTIVITY`] turns added to the agent's
    /// context window, oldest first — the series the header's monitor
    /// prefers, and empty until the agent reports usage at all.
    ///
    /// ACP's `UsageUpdate` carries `used`, "tokens currently in
    /// context": a *level*, not a running total. So the per-turn figure
    /// is how far that level moved between one turn's end and the next,
    /// and a turn the window did not grow over is not a sample — the
    /// same rule [`Model::activity`] applies to a quiet frame. That
    /// makes this honest about what a turn *added to the context*, not
    /// about what it cost: an agent that compacts burns tokens while
    /// the window shrinks. Billing is the cost ledger's job.
    pub fn tokens(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        self.tokens.iter().copied()
    }

    /// The context-window level the agent last reported, in tokens —
    /// what the native bottom bar prints as `38,204 tok`. `None` until
    /// the agent reports usage at all, and the bar's slot is absent
    /// with it: a `0 tok` nothing measured would be furniture.
    #[must_use]
    pub const fn token_level(&self) -> Option<u64> {
        self.token_used
    }

    /// How much of the context window is left, in tokens — the foot's
    /// `162k left` beside the level. The
    /// window's size rides the same usage update as the level, so both
    /// halves are `None` until the agent reports at all; a level past
    /// the window (a report mid-compaction) reads as no headroom, never
    /// as a wrap.
    #[must_use]
    pub fn token_headroom(&self) -> Option<u64> {
        Some(self.token_size?.saturating_sub(self.token_used?))
    }

    /// The series the header's activity monitor plots: the per-turn
    /// token counts once the agent has reported usage, the busy-frame
    /// event count until then.
    ///
    /// The single site that decides between the two. Both the painted
    /// line ([`activity_series`](view::activity_series)) and how far it
    /// has drawn itself in ([`MotionKey::activity`](super::MotionKey))
    /// read it, so the sweep can never be measuring a series the slot
    /// is not showing.
    pub fn monitor(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        let series = if self.tokens.is_empty() {
            &self.activity
        } else {
            &self.tokens
        };
        series.iter().copied()
    }

    /// One more thing happened this frame.
    pub(super) const fn note_activity(&mut self) {
        self.activity_bucket = self.activity_bucket.saturating_add(1);
    }

    /// The agent reported its context window: remember the level, which
    /// the next turn boundary measures against, and the window's size,
    /// which the headroom is read against.
    pub(super) const fn note_usage(&mut self, used: u64, size: u64) {
        self.token_used = Some(used);
        self.token_size = Some(size);
    }

    /// A turn ended: the context's growth since the last boundary
    /// closes onto the token series. A turn with no usage behind it,
    /// or one the window did not grow over, is not a sample — but the
    /// mark still follows the level, so the turn after a compaction
    /// measures from where the window actually is.
    pub(super) fn roll_tokens(&mut self) {
        let Some(used) = self.token_used else {
            return;
        };
        let added = used.saturating_sub(self.token_mark);
        self.token_mark = used;
        if added == 0 {
            return;
        }
        if self.tokens.len() == ACTIVITY {
            self.tokens.pop_front();
        }
        self.tokens.push_back(added);
    }

    /// A frame was drawn: a bucket that caught anything closes onto the
    /// ribbon; an empty one is not a sample.
    pub(super) fn roll_activity(&mut self) {
        if self.activity_bucket == 0 {
            return;
        }
        if self.activity.len() == ACTIVITY {
            self.activity.pop_front();
        }
        self.activity.push_back(self.activity_bucket);
        self.activity_bucket = 0;
    }

    /// The last few inputs, oldest first — the harness's witness that
    /// inputs reach the model in order with the right targets.
    pub fn recent(&self) -> impl Iterator<Item = &Msg> {
        self.recent.iter()
    }

    /// Events the agent's session delivered so far.
    #[must_use]
    pub const fn agent_events(&self) -> u64 {
        self.agent_events
    }

    /// Typed events the engines delivered so far, duplicates excluded.
    #[must_use]
    pub const fn engine_frames(&self) -> u64 {
        self.engine_frames
    }

    /// How fresh the store's picture here is, and how the last event was
    /// delivered.
    #[must_use]
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }

    /// The feed's attachment to the broadcast socket, once it reported one.
    #[must_use]
    pub const fn feed(&self) -> Option<FeedStatus> {
        self.feed
    }

    /// Job results received so far.
    #[must_use]
    pub const fn jobs_done(&self) -> u64 {
        self.jobs_done
    }

    /// The last job result, if any.
    #[must_use]
    pub const fn last_job(&self) -> Option<&(JobId, JobOutput)> {
        self.last_job.as_ref()
    }

    /// How many frames had been drawn when the last job result arrived —
    /// the witness that the cockpit kept painting while the job ran.
    #[must_use]
    pub const fn last_job_frame(&self) -> Option<u64> {
        self.last_job_frame
    }

    pub(super) fn remember(&mut self, msg: Msg) {
        if self.recent.len() == RIBBON {
            self.recent.pop_front();
        }
        self.recent.push_back(msg);
    }
}

const fn stage_width(size: Size, rail_cols: u16) -> u16 {
    size.cols
        .saturating_sub(STAGE_FRAME_COLS)
        .saturating_sub(rail_cols)
}
