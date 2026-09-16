//! The widgets of the cockpit and their ids.

use super::phase::Phase;
use super::place::Place;
use super::stage_tab::StageTab;
use crate::agent::PermissionKind;
use crate::widget_id::WidgetId;

/// Every widget the cockpit lays out. The discriminant is the
/// [`WidgetId`] the layout and the hit registry report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Widget {
    /// The one-line header.
    Header,
    /// The header's brand slot — one cell: the pixel mark covers it in
    /// Graphics mode, the brand glyph fills it otherwise.
    HeaderBrand,
    /// The header's activity sparkline — the pixel line in Graphics
    /// mode, a ramp of block cells otherwise.
    HeaderSpark,
    /// The status strip: the iteration, the phases, the gates, the clock.
    Status,
    /// The strip's `iteration i/N`, shown while a loop runs.
    Iteration,
    /// Phase: Research.
    Research,
    /// Phase: Plan.
    Plan,
    /// Phase: Execute.
    Execute,
    /// Phase: Verify.
    Verify,
    /// The strip's gate LEDs, shown while a loop runs and once a gate
    /// started.
    Gates,
    /// The strip's elapsed time, shown while a loop runs.
    Elapsed,
    /// The strip's roadmap chunk in progress, once one started.
    Chunk,
    /// The left rail: the plan, the roadmap's position, the signals.
    Rail,
    /// The rail's collapse/expand toggle.
    RailToggle,
    /// The draggable divider between the rail and the stage.
    RailDivider,
    /// The rail's mini-DAG block — the rect the pixel tree covers in
    /// Graphics mode, a two-line text summary otherwise.
    RailGraph,
    /// The rail's roadmap gauge — the sub-cell pixel bar in Graphics
    /// mode, the `▰▱` cells otherwise. Laid out once a chunk exists.
    RailGauge,
    /// The stage's tab bar: Turn.
    TabTurn,
    /// The stage's tab bar: Session.
    TabSession,
    /// The stage's tab bar: Roadmap.
    TabRoadmap,
    /// The stage filling the middle.
    Stage,
    /// The undocked tab's third column, on a ≥160-column screen.
    Undock,
    /// The Needs-you dock: one line when empty, a row per card.
    Dock,
    /// The front permission card's Allow button.
    Allow,
    /// The front permission card's Always button.
    Always,
    /// The front permission card's Deny button.
    Deny,
    /// The front permission card's Never button.
    Never,
    /// The Moment's *Show images* button — the pictures the asking
    /// call carries, revealed onto the feed. Laid out only while the
    /// card has some left to show.
    ShowImages,
    /// The front proposal card's Accept button.
    Accept,
    /// The front proposal card's Reject button.
    Reject,
    /// The front blocker card's Clear button.
    Clear,
    /// The Trace tab's \[Checkpoint\] button.
    Checkpoint,
    /// The selected step's card: `More` / `Less`, the card's disclosure.
    TraceDetail,
    /// The Trace tab's branch-graph block — the rect the pixel graph
    /// covers in Graphics mode, an indented list otherwise.
    TraceGraph,
    /// The Roadmap tab's tech-tree block — the rect the pixel DAG
    /// covers in Graphics mode, a workstream summary otherwise.
    RoadmapGraph,
    /// The selected chunk's \[Start\] button.
    RoadmapStart,
    /// The selected chunk's \[Complete\] button.
    RoadmapComplete,
    /// The selected shipped chunk's \[Export\] — the deliverables shelf
    /// written as files. Laid out only on
    /// [`Surface::Native`](super::Surface::Native).
    ShelfExport,
    /// The Report's \[Share page\] — the chapter written as one page,
    /// the share route of (view 09). Native only, like
    /// every Report verb.
    ShelfShare,
    /// The Report's commit receipt verb, `View`: the commit put on the
    /// stage.
    ReportCommit,
    /// The Report's gates receipt verb, `Log`: every gate's verdict put
    /// on the stage.
    ReportGates,
    /// The Report's cost receipt verb, `Detail`: the spend put on the
    /// stage.
    ReportCost,
    /// The Report's trace receipt verb, `Open`: the Trace view with the
    /// chunk's first reasoning step expanded — the provenance chip.
    ReportTrace,
    /// The launcher's \[next iteration\] button.
    LaunchNext,
    /// The launcher's \[loop\] button.
    LaunchLoop,
    /// The launcher's \[gates\] button.
    LaunchGates,
    /// The launcher's \[roadmap\] button — a tab switch, not an exit.
    LaunchRoadmap,
    /// The prompt line.
    Prompt,
    /// The one-line footer.
    Footer,
    /// The native header's `^` crumb — one altitude up. Laid out only
    /// on [`Surface::Native`](super::Surface::Native), like every
    /// crumb: the terminal's header has no altitude path.
    CrumbUp,
    /// The native header's `projects` crumb — altitude 0, the Galaxy
    /// (drawn as the Sky's surface until builds it).
    CrumbProjects,
    /// The native header's identity crumb — the project, altitude 1,
    /// the Sky.
    CrumbProject,
    /// The native header's session crumb — the chunk in progress,
    /// altitude 2, where the window already is.
    CrumbSession,
    /// The native header's record crumb — the Changes journal or the
    /// Trace feed at altitude 3, the deepest stop, which is where the
    /// window already is: pressing it holds.
    CrumbRecord,
    /// The native composer pill at the foot of the turn column — the
    /// window's prompt, which is deliberately **not** [`Self::Prompt`]:
    /// the acceptance "no prompt line on the window" is an assertion
    /// over placed ids, so the pill needs its own.
    Composer,
    /// The Brief's alert card: the raised verb that dives to the
    /// decision a stuck chunk is waiting on. Laid out only on
    /// [`Surface::Native`](super::Surface::Native) — the Brief is one
    /// of the native overlays and a terminal has no altitude to raise it
    /// over.
    BriefAnswer,
    /// The Brief's attention card: the raised verb that dives to the
    /// change waiting to be read.
    BriefReview,
    /// The Brief's good card: the raised verb that dives to what
    /// shipped while you were away.
    BriefReports,
    /// The Brief's dismiss control — the `esc · dismiss` pair in the
    /// overlay's own head row.
    BriefDismiss,
    /// The Galaxy's field — the room the clusters are drawn in.
    /// Laid out only on
    /// [`Surface::Native`](super::Surface::Native): the wheel over it
    /// zooms the field, and a cluster on it is its own indexed id.
    GalaxyField,
    /// The Galaxy's `＋ New project` — Birth F1's door: the camera
    /// moves to [`Place::Birth`] on the ground.
    GalaxyNew,
    /// Birth F1's dashed slot — the drop target, and `click to
    /// browse`. The slot itself is inert; the id is the wrapper
    /// around it. Laid out only on a native surface at Birth, like
    /// every Birth id.
    BirthSlot,
    /// Birth F1's `Clone a repository…`.
    BirthClone,
    /// Birth F1's `Start empty`.
    BirthEmpty,
    /// Birth F3's `Continue ↵` — the current answer committed.
    BirthContinue,
    /// Birth F4's `Accept this roadmap` — the one lit exit: the
    /// proposal written to think-and-ship.
    BirthAccept,
    /// Birth F4's `Redraw` — the proposal drafted again around the
    /// pinned stars.
    BirthRedraw,
    /// Birth F4's `Edit chunks…` — the first row opened for renaming.
    BirthEdit,
    /// Birth F5's `Claim <chunk>` — the window closes and reopens on
    /// the new project's first iteration.
    BirthClaim,
    /// Birth F5's `Later` — the accepted sky, and nothing started.
    BirthLater,
    /// The ⌘, surface's `ESC · CLOSE` — the one way out by pointer
    /// (view 10). Laid out only on a native surface while settings is
    /// up, like every settings id.
    SettingsClose,
    /// A settings pane's `Open file` — the pane's source file, opened
    /// on the feed the way an edit card's \[Open\] opens its file.
    SettingsOpenFile,
    /// The bad day's agent card: `Install agent…`, the lit verb that
    /// announces the install line (view 14). Laid out only on a native
    /// surface while a fault is up, like every fault verb.
    FaultInstall,
    /// The bad day's agent card: `Details` — the receipt trail, on the
    /// stage.
    FaultDetails,
    /// The bad day's breaker card: `Increase limit` — settings, at
    /// Budget & limits.
    FaultIncreaseLimit,
    /// The bad day's breaker card: `View receipts` — the Changes
    /// journal.
    FaultReceipts,
    /// The bad day's connection card: `Reconnect` — the transport
    /// redialled.
    FaultReconnect,
    /// The ledger head's `Pause` — `Resume` while the loop holds a
    /// boundary. Laid out only on
    /// [`Surface::Native`](super::Surface::Native), on the rail's spine
    /// head, like its two siblings; a terminal has the keys.
    LoopPause,
    /// The ledger head's `Step`: one iteration, then the boundary after
    /// it.
    LoopStep,
    /// The ledger head's `Checkpoint now`: the state as it stands, on
    /// the spine.
    LoopCheckpoint,
    /// The ledger head's `Abort` — the loop ended at the boundary it
    /// holds or reaches next. The
    /// window's one abort besides the chord: its Esc climbs.
    LoopAbort,
    /// The meter's `Raise` on the rail's foot line — settings at Budget & limits, the
    /// bad day's `Increase limit` reachable before the breaker trips.
    /// Laid out only on [`Surface::Native`](super::Surface::Native).
    BudgetRaise,
    /// The terminal dock's grip: the row the running
    /// card's raised screen is dragged by. Laid out on both surfaces
    /// while the dock is up.
    TermDockGrip,
    /// The terminal dock's screen — the wheel's target while the dock
    /// is up: a notch over it moves the docked pane, never the stage.
    TermDock,
    /// The identity band's chunk chip: the navigation
    /// chip on the session's cap naming the chunk being worked, which
    /// rises to its star on the Sky with the card framed.
    /// Laid out only on [`Surface::Native`](super::Surface::Native) —
    /// the terminal's cap is a string.
    SessionChunk,
    /// The shared object inspector and its card sections.
    Inspector,
    InspectorTitle,
    InspectorStatus,
    InspectorMetadata,
    InspectorActions,
    InspectorFields,
    InspectorDetail,
    /// Open the selected agent edit's file.
    ToolOpen,
    /// Undo the selected agent edit through its existing command path.
    ToolUndo,
    /// Roadmap table and its filter and sortable column headers.
    RoadmapTable,
    RoadmapFilter,
    RoadmapSortName,
    RoadmapSortStatus,
    RoadmapSortPriority,
    RoadmapSortWorkstream,
    /// The terminal composer's Send or Stop action.
    PromptAction,
    /// Select the newest recorded decision without creating a checkpoint.
    LatestDecision,
    /// Walk the recorder when its history exceeds the available width.
    TracePrevious,
    TraceNext,
    /// Choose workspace files to include in the next message.
    PromptFiles,
    /// Open the work named by the selected decision.
    LinkedWork,
    /// Open the command menu.
    Commands,
    /// The query field in a command or search overlay.
    OverlayQuery,
    /// Prepare an editable request about the selected roadmap item.
    RoadmapRequest,
    /// Restore the full dependency graph without changing the message draft.
    RoadmapReset,
}

impl Widget {
    /// Every widget, in layout order.
    pub const ALL: [Self; 110] = [
        Self::Header,
        Self::HeaderBrand,
        Self::HeaderSpark,
        Self::Status,
        Self::Iteration,
        Self::Research,
        Self::Plan,
        Self::Execute,
        Self::Verify,
        Self::Gates,
        Self::Elapsed,
        Self::Chunk,
        Self::Rail,
        Self::RailToggle,
        Self::RailDivider,
        Self::RailGraph,
        Self::RailGauge,
        Self::TabTurn,
        Self::TabSession,
        Self::TabRoadmap,
        Self::Stage,
        Self::Undock,
        Self::Dock,
        Self::Allow,
        Self::Always,
        Self::Deny,
        Self::Never,
        Self::ShowImages,
        Self::Accept,
        Self::Reject,
        Self::Clear,
        Self::Checkpoint,
        Self::TraceDetail,
        Self::TraceGraph,
        Self::RoadmapGraph,
        Self::RoadmapStart,
        Self::RoadmapComplete,
        Self::ShelfExport,
        Self::ShelfShare,
        Self::ReportCommit,
        Self::ReportGates,
        Self::ReportCost,
        Self::ReportTrace,
        Self::LaunchNext,
        Self::LaunchLoop,
        Self::LaunchGates,
        Self::LaunchRoadmap,
        Self::Prompt,
        Self::Footer,
        Self::CrumbUp,
        Self::CrumbProjects,
        Self::CrumbProject,
        Self::CrumbSession,
        Self::CrumbRecord,
        Self::Composer,
        Self::BriefAnswer,
        Self::BriefReview,
        Self::BriefReports,
        Self::BriefDismiss,
        Self::GalaxyField,
        Self::GalaxyNew,
        Self::BirthSlot,
        Self::BirthClone,
        Self::BirthEmpty,
        Self::BirthContinue,
        Self::BirthAccept,
        Self::BirthRedraw,
        Self::BirthEdit,
        Self::BirthClaim,
        Self::BirthLater,
        Self::SettingsClose,
        Self::SettingsOpenFile,
        Self::FaultInstall,
        Self::FaultDetails,
        Self::FaultIncreaseLimit,
        Self::FaultReceipts,
        Self::FaultReconnect,
        Self::LoopPause,
        Self::LoopStep,
        Self::LoopCheckpoint,
        Self::LoopAbort,
        Self::BudgetRaise,
        Self::TermDockGrip,
        Self::TermDock,
        Self::SessionChunk,
        Self::Inspector,
        Self::InspectorTitle,
        Self::InspectorStatus,
        Self::InspectorMetadata,
        Self::InspectorActions,
        Self::InspectorFields,
        Self::InspectorDetail,
        Self::ToolOpen,
        Self::ToolUndo,
        Self::RoadmapTable,
        Self::RoadmapFilter,
        Self::RoadmapSortName,
        Self::RoadmapSortStatus,
        Self::RoadmapSortPriority,
        Self::RoadmapSortWorkstream,
        Self::PromptAction,
        Self::LatestDecision,
        Self::TracePrevious,
        Self::TraceNext,
        Self::PromptFiles,
        Self::LinkedWork,
        Self::Commands,
        Self::OverlayQuery,
        Self::RoadmapRequest,
        Self::RoadmapReset,
    ];

    /// The ledger head's verbs — laid out only on
    /// [`Surface::Native`](super::Surface::Native), on the rail's spine
    /// head, in the order draws them.
    pub const LOOP_VERBS: [Self; 4] = [
        Self::LoopPause,
        Self::LoopStep,
        Self::LoopCheckpoint,
        Self::LoopAbort,
    ];

    /// Birth's own controls — laid out
    /// only on [`Surface::Native`](super::Surface::Native), and only at
    /// [`Place::Birth`], each on its moment: the ground's three ways
    /// in, the interview's Continue, the charting's three verbs, first
    /// light's Claim and Later.
    pub const BIRTH: [Self; 9] = [
        Self::BirthSlot,
        Self::BirthClone,
        Self::BirthEmpty,
        Self::BirthContinue,
        Self::BirthAccept,
        Self::BirthRedraw,
        Self::BirthEdit,
        Self::BirthClaim,
        Self::BirthLater,
    ];

    /// The bad day's own verbs — laid out only on
    /// [`Surface::Native`](super::Surface::Native), and only while a
    /// fault is up. The order is the deal's: the agent card's pair, the
    /// breaker card's pair, the connection card's one.
    pub const FAULTS: [Self; 5] = [
        Self::FaultInstall,
        Self::FaultDetails,
        Self::FaultIncreaseLimit,
        Self::FaultReceipts,
        Self::FaultReconnect,
    ];

    /// The Brief's own controls — laid out only on
    /// [`Surface::Native`](super::Surface::Native), and only while the
    /// catch-up overlay is raised. The three verbs are in the order the
    /// deal puts them, worst first.
    pub const BRIEF: [Self; 4] = [
        Self::BriefAnswer,
        Self::BriefReview,
        Self::BriefReports,
        Self::BriefDismiss,
    ];

    /// The native header's altitude path, left to right — laid out
    /// only on [`Surface::Native`](super::Surface::Native).
    pub const CRUMBS: [Self; 5] = [
        Self::CrumbUp,
        Self::CrumbProjects,
        Self::CrumbProject,
        Self::CrumbSession,
        Self::CrumbRecord,
    ];

    /// The phases on the strip, left to right.
    pub const PHASES: [Self; 4] = [Self::Research, Self::Plan, Self::Execute, Self::Verify];

    /// The strip's loop widgets — laid out only while a loop runs (the
    /// gates once one started).
    pub const LOOP: [Self; 3] = [Self::Iteration, Self::Gates, Self::Elapsed];

    /// The dock's buttons — laid out only on the front card, and only
    /// the ones its type offers.
    pub const DOCK_BUTTONS: [Self; 7] = [
        Self::Allow,
        Self::Always,
        Self::Deny,
        Self::Never,
        Self::Accept,
        Self::Reject,
        Self::Clear,
    ];

    /// Ids at or above this raw value name a stage row by its visible
    /// index — the rows are dynamic, so they live outside the enum.
    const STAGE_ROW_BASE: u32 = 0x1_0000;
    /// An expanded edit card's \[Open\] button, by the visible index of
    /// the row it sits on.
    const EDIT_OPEN_BASE: u32 = 0x2_0000;
    /// An expanded edit card's \[Undo\] button, likewise.
    const EDIT_UNDO_BASE: u32 = 0x3_0000;
    /// A completion popup row, by its index in the popup.
    const COMPLETION_BASE: u32 = 0x4_0000;
    /// A Changes-tab entry's head row, by the entry's index in the
    /// journal list.
    const CHANGE_ROW_BASE: u32 = 0x5_0000;
    /// An expanded change's \[Open\] button, by the entry's index.
    const CHANGE_OPEN_BASE: u32 = 0x6_0000;
    /// An expanded change's \[Undo\] button, likewise.
    const CHANGE_UNDO_BASE: u32 = 0x7_0000;
    /// An expanded change's \[Approve\] button, likewise.
    const CHANGE_APPROVE_BASE: u32 = 0x8_0000;
    /// A Trace-tab step's head row, by the step's index in the feed.
    const TRACE_ROW_BASE: u32 = 0x9_0000;
    /// An expanded step's \[Pin\]/\[Unpin\] button, by the step's
    /// project-global number.
    const TRACE_PIN_BASE: u32 = 0xA_0000;
    /// A revision link on an expanded step, by the number of the step
    /// it navigates to.
    const TRACE_LINK_BASE: u32 = 0xB_0000;
    /// A Roadmap-tab chunk row, by its index in the list.
    const ROADMAP_ROW_BASE: u32 = 0xC_0000;
    /// A proposal row's \[Accept\] button, by the proposal's index.
    const PROPOSAL_ACCEPT_BASE: u32 = 0xD_0000;
    /// A proposal row's \[Reject\] button, likewise.
    const PROPOSAL_REJECT_BASE: u32 = 0xE_0000;
    /// A signal row's \[Promote\] button, by the signal's index.
    const SIGNAL_PROMOTE_BASE: u32 = 0xF_0000;
    /// A signal row's \[Ignore\] button, likewise.
    const SIGNAL_IGNORE_BASE: u32 = 0x10_0000;
    /// A signal row's \[Snooze\] button, likewise.
    const SIGNAL_SNOOZE_BASE: u32 = 0x11_0000;
    /// A launcher session row, by its index in the sessions card.
    const LAUNCH_SESSION_BASE: u32 = 0x12_0000;
    /// A footer key-cap chip, by its index along the bar. Registered so
    /// the pointer can raise it; a click on one falls through
    /// to the bar, which answers nothing.
    const FOOTER_CHIP_BASE: u32 = 0x13_0000;
    /// The chunk jump on an expanded step's execution ref, by the
    /// step's project-global number — the same key
    /// [`TRACE_LINK_BASE`](Self::TRACE_LINK_BASE) uses, in its own
    /// space, so a step can carry both links at once.
    const TRACE_CHUNK_BASE: u32 = 0x14_0000;
    /// A rail checkpoint row, by its index on the spine.
    const SPINE_ROW_BASE: u32 = 0x15_0000;
    /// A reviewed file's row on the review surface, by its index in the
    /// journal list.
    const REVIEW_FILE_BASE: u32 = 0x16_0000;
    /// A star on the native sky, by its index in the tech-tree's graph
    /// order — the index space [`Roadmap::dag_nodes`](super::Roadmap::dag_nodes)
    /// and [`Roadmap::dag_edges`](super::Roadmap::dag_edges) share.
    const SKY_STAR_BASE: u32 = 0x17_0000;
    /// A ⌘K palette result row, by its index in
    /// [`Palette::rows`](super::Palette::rows).
    const PALETTE_ROW_BASE: u32 = 0x18_0000;
    /// A cluster on the Galaxy, by its index in
    /// [`Galaxy::projects`](super::Galaxy::projects).
    const GALAXY_CLUSTER_BASE: u32 = 0x19_0000;
    /// A signal row's \[Research\] button, by the signal's index in the
    /// inbox — the Signals view's third verb.
    const SIGNAL_RESEARCH_BASE: u32 = 0x1A_0000;
    /// A ⌘⇧F result row, by its index in
    /// [`SearchResults::rows`](super::SearchResults::rows).
    const SEARCH_ROW_BASE: u32 = 0x1B_0000;
    /// A ⌘⇧F scope chip, by its index in
    /// [`SearchScope::ALL`](super::SearchScope::ALL).
    const SEARCH_SCOPE_BASE: u32 = 0x1C_0000;
    /// A ⌘, sidebar item, by its index in
    /// [`SettingsGroup::ALL`](super::SettingsGroup::ALL).
    const SETTINGS_NAV_BASE: u32 = 0x1D_0000;
    /// A ⌘, theme card, by its index in
    /// [`ThemeChoice::ALL`](super::ThemeChoice::ALL).
    const SETTINGS_THEME_BASE: u32 = 0x1E_0000;
    /// A ⌘, switch, by its index down the pane (0 the heatmap, 1 the
    /// grain).
    const SETTINGS_SWITCH_BASE: u32 = 0x1F_0000;
    /// A ⌘, density segment, by its index in
    /// [`Density::ALL`](super::density::Density::ALL).
    const SETTINGS_DENSITY_BASE: u32 = 0x20_0000;
    /// An interview chip (Birth F3), by its index on the current
    /// question.
    const BIRTH_CHIP_BASE: u32 = 0x21_0000;
    /// A proposal row (Birth F4), by its index in the list — dragged
    /// to reorder, right-clicked to rename.
    const BIRTH_ROW_BASE: u32 = 0x22_0000;
    /// A rail checkpoint row's `Restore`, by the row's index on the
    /// spine — the verb the window reveals on the row;
    /// a terminal has `/restore`.
    const SPINE_RESTORE_BASE: u32 = 0x23_0000;
    /// A rail checkpoint row's `Fork`, likewise.
    const SPINE_FORK_BASE: u32 = 0x24_0000;
    /// A rail charter row, by its index in
    /// [`Rail::charter`](super::Rail::charter) — a hit target that
    /// raises and is named; the dial that cycles a row's mode is not
    /// built.
    const CHARTER_ROW_BASE: u32 = 0x25_0000;
    /// A rail charter offer's `Delegate`, by its index in
    /// [`Rail::offers`](super::Rail::offers) — the verb that accepts
    /// the widening the reader's record supports.
    const CHARTER_OFFER_BASE: u32 = 0x26_0000;
    /// Each indexed id space is this wide — far more rows than any
    /// screen shows, and no space bleeds into the next.
    const INDEX_SPAN: u32 = 0x1_0000;
    /// A compact signal row selecting the shared inspector.
    const SIGNAL_ROW_BASE: u32 = 0x27_0000;
    /// An action/check/run row in the unified Session timeline.
    const SESSION_ROW_BASE: u32 = 0x28_0000;
    /// A dependency of the work item currently shown in the inspector.
    const ROADMAP_DEPENDENCY_BASE: u32 = 0x29_0000;

    #[must_use]
    pub const fn roadmap_dependency(index: usize) -> WidgetId {
        Self::indexed(Self::ROADMAP_DEPENDENCY_BASE, index)
    }

    #[must_use]
    pub const fn roadmap_dependency_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::ROADMAP_DEPENDENCY_BASE, id)
    }

    #[must_use]
    pub const fn session_row(index: usize) -> WidgetId {
        Self::indexed(Self::SESSION_ROW_BASE, index)
    }

    #[must_use]
    pub const fn session_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SESSION_ROW_BASE, id)
    }

    #[must_use]
    pub const fn signal_row(index: usize) -> WidgetId {
        Self::indexed(Self::SIGNAL_ROW_BASE, index)
    }

    #[must_use]
    pub const fn signal_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SIGNAL_ROW_BASE, id)
    }

    /// The id this widget carries through layout and hit-testing.
    #[must_use]
    pub const fn id(self) -> WidgetId {
        WidgetId::new(self as u32)
    }

    /// The widget behind an id the hit registry reported.
    #[must_use]
    pub fn from_id(id: WidgetId) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }

    /// The id of the stage row at visible `index` (top of the window
    /// is 0) — how a click finds the row it landed on.
    #[must_use]
    pub const fn stage_row(index: usize) -> WidgetId {
        Self::indexed(Self::STAGE_ROW_BASE, index)
    }

    /// The visible row index behind a stage-row id, if it is one.
    #[must_use]
    pub const fn stage_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::STAGE_ROW_BASE, id)
    }

    /// The id of the \[Open\] button on the edit card whose button row
    /// is at visible `index`.
    #[must_use]
    pub const fn edit_open(index: usize) -> WidgetId {
        Self::indexed(Self::EDIT_OPEN_BASE, index)
    }

    /// The visible row index behind an \[Open\] id, if it is one.
    #[must_use]
    pub const fn edit_open_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::EDIT_OPEN_BASE, id)
    }

    /// The id of the \[Undo\] button on the edit card whose button row
    /// is at visible `index`.
    #[must_use]
    pub const fn edit_undo(index: usize) -> WidgetId {
        Self::indexed(Self::EDIT_UNDO_BASE, index)
    }

    /// The visible row index behind an \[Undo\] id, if it is one.
    #[must_use]
    pub const fn edit_undo_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::EDIT_UNDO_BASE, id)
    }

    /// The id of the completion popup row at `index`.
    #[must_use]
    pub const fn completion(index: usize) -> WidgetId {
        Self::indexed(Self::COMPLETION_BASE, index)
    }

    /// The id of the Changes-tab head row for the entry at `index` in
    /// the journal list.
    #[must_use]
    pub const fn change_row(index: usize) -> WidgetId {
        Self::indexed(Self::CHANGE_ROW_BASE, index)
    }

    /// The entry index behind a Changes head-row id, if it is one.
    #[must_use]
    pub const fn change_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHANGE_ROW_BASE, id)
    }

    /// The id of the \[Open\] button on the change at `index`.
    #[must_use]
    pub const fn change_open(index: usize) -> WidgetId {
        Self::indexed(Self::CHANGE_OPEN_BASE, index)
    }

    /// The entry index behind a change's \[Open\] id, if it is one.
    #[must_use]
    pub const fn change_open_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHANGE_OPEN_BASE, id)
    }

    /// The id of the \[Undo\] button on the change at `index`.
    #[must_use]
    pub const fn change_undo(index: usize) -> WidgetId {
        Self::indexed(Self::CHANGE_UNDO_BASE, index)
    }

    /// The entry index behind a change's \[Undo\] id, if it is one.
    #[must_use]
    pub const fn change_undo_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHANGE_UNDO_BASE, id)
    }

    /// The id of the checkpoint row at `index` on the rail's spine.
    #[must_use]
    pub const fn spine_row(index: usize) -> WidgetId {
        Self::indexed(Self::SPINE_ROW_BASE, index)
    }

    /// The spine index behind a checkpoint row's id, if it is one.
    #[must_use]
    pub const fn spine_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SPINE_ROW_BASE, id)
    }

    /// The id of the `Restore` verb on the checkpoint row at `index`.
    #[must_use]
    pub const fn spine_restore(index: usize) -> WidgetId {
        Self::indexed(Self::SPINE_RESTORE_BASE, index)
    }

    /// The spine index behind a row's `Restore` id, if it is one.
    #[must_use]
    pub const fn spine_restore_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SPINE_RESTORE_BASE, id)
    }

    /// The id of the `Fork` verb on the checkpoint row at `index`.
    #[must_use]
    pub const fn spine_fork(index: usize) -> WidgetId {
        Self::indexed(Self::SPINE_FORK_BASE, index)
    }

    /// The spine index behind a row's `Fork` id, if it is one.
    #[must_use]
    pub const fn spine_fork_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SPINE_FORK_BASE, id)
    }

    /// The id of the charter row at `index` on the rail.
    #[must_use]
    pub const fn charter_row(index: usize) -> WidgetId {
        Self::indexed(Self::CHARTER_ROW_BASE, index)
    }

    /// The charter index behind a rule row's id, if it is one.
    #[must_use]
    pub const fn charter_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHARTER_ROW_BASE, id)
    }

    /// The id of the `Delegate` on the charter's offer at `index`.
    #[must_use]
    pub const fn charter_offer(index: usize) -> WidgetId {
        Self::indexed(Self::CHARTER_OFFER_BASE, index)
    }

    /// The offer index behind a `Delegate`'s id, if it is one.
    #[must_use]
    pub const fn charter_offer_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHARTER_OFFER_BASE, id)
    }

    /// The id of the ⌘K palette result row at `index`.
    #[must_use]
    pub const fn palette_row(index: usize) -> WidgetId {
        Self::indexed(Self::PALETTE_ROW_BASE, index)
    }

    /// The result index behind a palette row's id, if it is one.
    #[must_use]
    pub const fn palette_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::PALETTE_ROW_BASE, id)
    }

    /// The id of the ⌘⇧F result row at `index`.
    #[must_use]
    pub const fn search_row(index: usize) -> WidgetId {
        Self::indexed(Self::SEARCH_ROW_BASE, index)
    }

    /// The row index behind a search row's id, if it is one.
    #[must_use]
    pub const fn search_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SEARCH_ROW_BASE, id)
    }

    /// The id of the ⌘⇧F scope chip at `index`.
    #[must_use]
    pub const fn search_scope(index: usize) -> WidgetId {
        Self::indexed(Self::SEARCH_SCOPE_BASE, index)
    }

    /// The chip index behind a scope chip's id, if it is one.
    #[must_use]
    pub const fn search_scope_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SEARCH_SCOPE_BASE, id)
    }

    /// The id of the ⌘, sidebar item at `index`.
    #[must_use]
    pub const fn settings_nav(index: usize) -> WidgetId {
        Self::indexed(Self::SETTINGS_NAV_BASE, index)
    }

    /// The group index behind a sidebar item's id, if it is one.
    #[must_use]
    pub const fn settings_nav_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SETTINGS_NAV_BASE, id)
    }

    /// The id of the ⌘, theme card at `index`.
    #[must_use]
    pub const fn settings_theme(index: usize) -> WidgetId {
        Self::indexed(Self::SETTINGS_THEME_BASE, index)
    }

    /// The card index behind a theme card's id, if it is one.
    #[must_use]
    pub const fn settings_theme_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SETTINGS_THEME_BASE, id)
    }

    /// The id of the ⌘, switch at `index`.
    #[must_use]
    pub const fn settings_switch(index: usize) -> WidgetId {
        Self::indexed(Self::SETTINGS_SWITCH_BASE, index)
    }

    /// The switch index behind a switch's id, if it is one.
    #[must_use]
    pub const fn settings_switch_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SETTINGS_SWITCH_BASE, id)
    }

    /// The id of the ⌘, density segment at `index`.
    #[must_use]
    pub const fn settings_density(index: usize) -> WidgetId {
        Self::indexed(Self::SETTINGS_DENSITY_BASE, index)
    }

    /// The segment index behind a density segment's id, if it is one.
    #[must_use]
    pub const fn settings_density_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SETTINGS_DENSITY_BASE, id)
    }

    /// The id of the reviewed file row at `index`.
    #[must_use]
    pub const fn review_file(index: usize) -> WidgetId {
        Self::indexed(Self::REVIEW_FILE_BASE, index)
    }

    /// The file index behind a review file row's id, if it is one.
    #[must_use]
    pub const fn review_file_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::REVIEW_FILE_BASE, id)
    }

    /// The id of the \[Approve\] button on the change at `index`.
    #[must_use]
    pub const fn change_approve(index: usize) -> WidgetId {
        Self::indexed(Self::CHANGE_APPROVE_BASE, index)
    }

    /// The entry index behind a change's \[Approve\] id, if it is one.
    #[must_use]
    pub const fn change_approve_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::CHANGE_APPROVE_BASE, id)
    }

    /// The popup row index behind a completion id, if it is one.
    #[must_use]
    pub const fn completion_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::COMPLETION_BASE, id)
    }

    /// The id of the Trace-tab head row for the step at feed `index`.
    #[must_use]
    pub const fn trace_row(index: usize) -> WidgetId {
        Self::indexed(Self::TRACE_ROW_BASE, index)
    }

    /// The feed index behind a Trace head-row id, if it is one.
    #[must_use]
    pub const fn trace_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::TRACE_ROW_BASE, id)
    }

    /// The id of the \[Pin\]/\[Unpin\] button on step `number`.
    #[must_use]
    pub const fn trace_pin(number: u32) -> WidgetId {
        Self::indexed(Self::TRACE_PIN_BASE, number as usize)
    }

    /// The step number behind a \[Pin\] id, if it is one.
    #[must_use]
    pub fn trace_pin_step(id: WidgetId) -> Option<u32> {
        Self::index_in(Self::TRACE_PIN_BASE, id).and_then(|n| u32::try_from(n).ok())
    }

    /// The id of the revision link navigating to step `number`.
    #[must_use]
    pub const fn trace_link(number: u32) -> WidgetId {
        Self::indexed(Self::TRACE_LINK_BASE, number as usize)
    }

    /// The target step number behind a revision-link id, if it is one.
    #[must_use]
    pub fn trace_link_step(id: WidgetId) -> Option<u32> {
        Self::index_in(Self::TRACE_LINK_BASE, id).and_then(|n| u32::try_from(n).ok())
    }

    /// The id of the chunk jump on step `number`'s execution ref.
    #[must_use]
    pub const fn trace_chunk(number: u32) -> WidgetId {
        Self::indexed(Self::TRACE_CHUNK_BASE, number as usize)
    }

    /// The step number behind a chunk-jump id, if it is one. The
    /// *chunk* it reaches is not in the id: the click re-runs the same
    /// [`Roadmap::chunk_for_ref`](super::Roadmap::chunk_for_ref) the
    /// view painted the link from, so a re-seeded roadmap cannot leave
    /// a stale index behind.
    #[must_use]
    pub fn trace_chunk_step(id: WidgetId) -> Option<u32> {
        Self::index_in(Self::TRACE_CHUNK_BASE, id).and_then(|n| u32::try_from(n).ok())
    }

    /// The id of the sky's star at graph `index`.
    ///
    /// The chunk is deliberately *not* in the id: a click re-reads the
    /// same graph order the sky painted from, so a roadmap re-seeded
    /// between the paint and the press cannot leave a stale chunk
    /// behind — the trick [`Widget::trace_chunk`] plays for the same
    /// reason.
    #[must_use]
    pub const fn sky_star(index: usize) -> WidgetId {
        Self::indexed(Self::SKY_STAR_BASE, index)
    }

    /// The graph index behind a star id, if it is one.
    #[must_use]
    pub const fn sky_star_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SKY_STAR_BASE, id)
    }

    /// The id of the Galaxy's cluster at project `index`.
    ///
    /// The project is not in the id, for the star's reason: a click
    /// re-reads the same list the field painted from, so a reseed
    /// between the paint and the press cannot leave a stale project
    /// behind.
    #[must_use]
    pub const fn galaxy_cluster(index: usize) -> WidgetId {
        Self::indexed(Self::GALAXY_CLUSTER_BASE, index)
    }

    /// The project index behind a cluster id, if it is one.
    #[must_use]
    pub const fn galaxy_cluster_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::GALAXY_CLUSTER_BASE, id)
    }

    /// The id of the Roadmap-tab chunk row at list `index`.
    #[must_use]
    pub const fn roadmap_row(index: usize) -> WidgetId {
        Self::indexed(Self::ROADMAP_ROW_BASE, index)
    }

    /// The list index behind a Roadmap chunk-row id, if it is one.
    #[must_use]
    pub const fn roadmap_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::ROADMAP_ROW_BASE, id)
    }

    /// The id of the \[Accept\] button on the proposal at `index`.
    #[must_use]
    pub const fn proposal_accept(index: usize) -> WidgetId {
        Self::indexed(Self::PROPOSAL_ACCEPT_BASE, index)
    }

    /// The proposal index behind an \[Accept\] id, if it is one.
    #[must_use]
    pub const fn proposal_accept_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::PROPOSAL_ACCEPT_BASE, id)
    }

    /// The id of the \[Reject\] button on the proposal at `index`.
    #[must_use]
    pub const fn proposal_reject(index: usize) -> WidgetId {
        Self::indexed(Self::PROPOSAL_REJECT_BASE, index)
    }

    /// The proposal index behind a \[Reject\] id, if it is one.
    #[must_use]
    pub const fn proposal_reject_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::PROPOSAL_REJECT_BASE, id)
    }

    /// The id of the \[Promote\] button on the signal at `index`.
    #[must_use]
    pub const fn signal_promote(index: usize) -> WidgetId {
        Self::indexed(Self::SIGNAL_PROMOTE_BASE, index)
    }

    /// The signal index behind a \[Promote\] id, if it is one.
    #[must_use]
    pub const fn signal_promote_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SIGNAL_PROMOTE_BASE, id)
    }

    /// The id of the \[Ignore\] button on the signal at `index`.
    #[must_use]
    pub const fn signal_ignore(index: usize) -> WidgetId {
        Self::indexed(Self::SIGNAL_IGNORE_BASE, index)
    }

    /// The signal index behind an \[Ignore\] id, if it is one.
    #[must_use]
    pub const fn signal_ignore_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SIGNAL_IGNORE_BASE, id)
    }

    /// The id of the \[Snooze\] button on the signal at `index`.
    #[must_use]
    pub const fn signal_snooze(index: usize) -> WidgetId {
        Self::indexed(Self::SIGNAL_SNOOZE_BASE, index)
    }

    /// The signal index behind a \[Snooze\] id, if it is one.
    #[must_use]
    pub const fn signal_snooze_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SIGNAL_SNOOZE_BASE, id)
    }

    /// The id of the \[Research\] button on the signal at `index`.
    #[must_use]
    pub const fn signal_research(index: usize) -> WidgetId {
        Self::indexed(Self::SIGNAL_RESEARCH_BASE, index)
    }

    /// The signal index behind a \[Research\] id, if it is one.
    #[must_use]
    pub const fn signal_research_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::SIGNAL_RESEARCH_BASE, id)
    }

    /// The id of the launcher's session row at `index` — hoverable
    /// through the hit registry, like every widget with an id.
    #[must_use]
    pub const fn launch_session(index: usize) -> WidgetId {
        Self::indexed(Self::LAUNCH_SESSION_BASE, index)
    }

    /// The session index behind a launcher session-row id, if it is one.
    #[must_use]
    pub const fn launch_session_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::LAUNCH_SESSION_BASE, id)
    }

    /// The id of the interview chip at `index` on the current question
    /// (Birth F3).
    #[must_use]
    pub const fn birth_chip(index: usize) -> WidgetId {
        Self::indexed(Self::BIRTH_CHIP_BASE, index)
    }

    /// The chip index behind an interview chip's id, if it is one.
    #[must_use]
    pub const fn birth_chip_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::BIRTH_CHIP_BASE, id)
    }

    /// The id of the proposal row at `index` (Birth F4).
    #[must_use]
    pub const fn birth_row(index: usize) -> WidgetId {
        Self::indexed(Self::BIRTH_ROW_BASE, index)
    }

    /// The row index behind a proposal row's id, if it is one.
    #[must_use]
    pub const fn birth_row_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::BIRTH_ROW_BASE, id)
    }

    /// The id of the footer's key-cap chip at `index` along the bar.
    #[must_use]
    pub const fn footer_chip(index: usize) -> WidgetId {
        Self::indexed(Self::FOOTER_CHIP_BASE, index)
    }

    /// The bar index behind a footer chip's id, if it is one.
    #[must_use]
    pub const fn footer_chip_index(id: WidgetId) -> Option<usize> {
        Self::index_in(Self::FOOTER_CHIP_BASE, id)
    }

    /// The id at `index` in the space starting at `base`; an index past
    /// the span pins to its last slot rather than bleeding onward.
    const fn indexed(base: u32, index: usize) -> WidgetId {
        let capped = if index as u32 >= Self::INDEX_SPAN {
            Self::INDEX_SPAN - 1
        } else {
            index as u32
        };
        WidgetId::new(base + capped)
    }

    /// The index behind `id` when it lies in the space at `base`.
    const fn index_in(base: u32, id: WidgetId) -> Option<usize> {
        let raw = id.raw();
        if raw >= base && raw < base + Self::INDEX_SPAN {
            Some((raw - base) as usize)
        } else {
            None
        }
    }

    /// The button that answers a permission request with `kind`.
    #[must_use]
    pub const fn button(kind: PermissionKind) -> Self {
        match kind {
            PermissionKind::AllowOnce => Self::Allow,
            PermissionKind::AllowAlways => Self::Always,
            PermissionKind::RejectOnce => Self::Deny,
            PermissionKind::RejectAlways => Self::Never,
        }
    }

    /// The permission kind this widget answers with, for a button.
    #[must_use]
    pub const fn answers(self) -> Option<PermissionKind> {
        match self {
            Self::Allow => Some(PermissionKind::AllowOnce),
            Self::Always => Some(PermissionKind::AllowAlways),
            Self::Deny => Some(PermissionKind::RejectOnce),
            Self::Never => Some(PermissionKind::RejectAlways),
            _ => None,
        }
    }

    /// The widget that shows `tab` on the bar.
    #[must_use]
    pub const fn tab(tab: StageTab) -> Self {
        match tab {
            StageTab::Turn => Self::TabTurn,
            StageTab::Session => Self::TabSession,
            StageTab::Roadmap => Self::TabRoadmap,
        }
    }

    /// The tab this widget selects, for a tab on the bar or the
    /// launcher's roadmap button. A crumb targets a [`Place`]
    /// ([`Widget::place_target`]), never a tab.
    #[must_use]
    pub const fn tab_target(self) -> Option<StageTab> {
        match self {
            Self::TabTurn => Some(StageTab::Turn),
            Self::TabSession => Some(StageTab::Session),
            Self::TabRoadmap | Self::LaunchRoadmap => Some(StageTab::Roadmap),
            _ => None,
        }
    }

    /// The place this widget moves the camera to, for a crumb on the
    /// native header — `projects` the Galaxy, the project the Sky,
    /// the session the Session, the record the place you are already
    /// at (its stop is the deepest, so it holds), and `^` one altitude
    /// up from `at` (holding at the Galaxy). `None` for everything else.
    #[must_use]
    pub const fn place_target(self, at: Place) -> Option<Place> {
        match self {
            Self::CrumbProjects => Some(Place::Galaxy),
            Self::CrumbProject => Some(Place::Sky),
            Self::CrumbSession => Some(Place::Session),
            Self::CrumbRecord => Some(at),
            Self::CrumbUp => Some(match at.up() {
                Some(up) => up,
                None => at,
            }),
            _ => None,
        }
    }

    /// The label the widget shows.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::HeaderBrand => "brand",
            Self::HeaderSpark => "activity",
            Self::Status => "status",
            Self::Iteration => "iteration",
            Self::Gates => "gates",
            Self::Elapsed => "elapsed",
            Self::Chunk => "chunk",
            Self::Rail => "rail",
            Self::RailToggle => "rail toggle",
            Self::RailDivider => "rail divider",
            Self::RailGraph => "rail graph",
            Self::RailGauge => "rail gauge",
            Self::Research => "Research",
            Self::Plan => "Plan",
            Self::Execute => "Execute",
            Self::Verify => "Verify",
            Self::TabTurn => "Turn",
            Self::TabSession => "Session",
            Self::TabRoadmap => "Roadmap",
            Self::Stage => "stage",
            Self::Undock => "undock",
            Self::Dock => "dock",
            Self::Allow => "Allow",
            Self::Always => "Always",
            Self::Deny => "Deny",
            Self::Never => "Never",
            Self::ShowImages => "Show images",
            Self::Accept => "Accept",
            Self::Reject => "Reject",
            Self::Clear => "Clear",
            Self::Checkpoint => "Checkpoint",
            Self::Inspector => "inspector",
            Self::InspectorTitle => "object title",
            Self::InspectorStatus => "object status",
            Self::InspectorMetadata => "object metadata",
            Self::InspectorActions => "object actions",
            Self::InspectorFields => "object fields",
            Self::InspectorDetail => "Details",
            Self::ToolOpen => "Open",
            Self::ToolUndo => "Undo",
            Self::TraceDetail => "More",
            Self::TraceGraph => "Reasoning",
            Self::RoadmapGraph => "Dependencies",
            Self::RoadmapTable => "work items",
            Self::RoadmapFilter => "filter status",
            Self::RoadmapSortName => "sort name",
            Self::RoadmapSortStatus => "sort status",
            Self::RoadmapSortPriority => "sort priority",
            Self::RoadmapSortWorkstream => "Sort by group",
            Self::RoadmapStart => "Start",
            Self::RoadmapComplete => "Complete",
            Self::ShelfExport => "Export",
            Self::ShelfShare => "Share page",
            Self::ReportCommit => "View",
            Self::ReportGates => "Log",
            Self::ReportCost => "Detail",
            Self::ReportTrace => "Open",
            Self::LaunchNext => "next iteration",
            Self::LaunchLoop => "loop",
            Self::LaunchGates => "gates",
            Self::LaunchRoadmap => "roadmap",
            Self::Prompt => "prompt",
            Self::Footer => "footer",
            Self::CrumbUp => "up",
            Self::CrumbProjects => "projects",
            Self::CrumbProject => "project",
            Self::CrumbSession => "session",
            Self::CrumbRecord => "record",
            Self::Composer => "composer",
            Self::BriefAnswer => "Answer",
            Self::BriefReview => "Review",
            Self::BriefReports => "View reports",
            Self::BriefDismiss => "Dismiss",
            Self::GalaxyField => "galaxy",
            Self::GalaxyNew => "New project",
            Self::BirthSlot => "Drop a folder here",
            Self::BirthClone => "Clone a repository\u{2026}",
            Self::BirthEmpty => "Start empty",
            Self::BirthContinue => "Continue",
            Self::BirthAccept => "Accept this roadmap",
            Self::BirthRedraw => "Redraw",
            Self::BirthEdit => "Edit chunks\u{2026}",
            Self::BirthClaim => "Claim",
            Self::BirthLater => "Later",
            Self::SettingsClose => "Close",
            Self::SettingsOpenFile => "Open file",
            Self::FaultInstall => "Install agent\u{2026}",
            Self::FaultDetails => "Details",
            Self::FaultIncreaseLimit => "Increase limit",
            Self::FaultReceipts => "View receipts",
            Self::FaultReconnect => "Reconnect",
            Self::LoopPause => "Pause",
            Self::LoopStep => "Step",
            Self::LoopCheckpoint => "Checkpoint now",
            Self::LoopAbort => "Abort",
            Self::BudgetRaise => "Raise",
            Self::TermDockGrip => "dock grip",
            Self::TermDock => "terminal dock",
            Self::SessionChunk => "chunk",
            Self::PromptAction => "Send or stop",
            Self::LatestDecision => "Return to latest",
            Self::TracePrevious => "Previous step",
            Self::TraceNext => "Next step",
            Self::PromptFiles => "Add files",
            Self::LinkedWork => "Linked work",
            Self::Commands => "Commands",
            Self::OverlayQuery => "Search",
            Self::RoadmapRequest => "Request a change",
            Self::RoadmapReset => "Reset view",
        }
    }

    /// The widget that shows `phase` on the strip; `Done` has none.
    #[must_use]
    pub const fn phase(phase: Phase) -> Option<Self> {
        match phase {
            Phase::Research => Some(Self::Research),
            Phase::Plan => Some(Self::Plan),
            Phase::Execute => Some(Self::Execute),
            Phase::Verify => Some(Self::Verify),
            Phase::Done => None,
        }
    }
}

#[cfg(test)]
#[path = "widget.tests.rs"]
mod tests;
