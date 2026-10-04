//! Roadmap records and derived browsing projections share snapshot, selection and
//! dependency authority; the retained geometry and tracks change with those records.

#[path = "roadmap_tracks.rs"]
mod tracks;
pub use tracks::{TrackCounts, TrackFocus, TrackRow};

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use super::RoadmapSort;
use magistr_workflow_contracts::roadmap::{Readiness, assess_readiness, next_ready};
use magistr_workflow_contracts::{Chunk, ChunkStatus, Signal};
use magistr_workflow_contracts::{NativeEvent, RoadmapFrame, SignalFrame};

use super::camera::Camera;
use super::chunk_state::ChunkState;
use super::dag_layout::DagLayout;
use super::dag_node::DagNode;
use super::dag_path::DagPath;
use super::edge_kind::EdgeKind;
use super::proposal_kind::ProposalKind;
use super::roadmap_chunk::RoadmapChunk;
use super::ship_run::ShipRun;
use super::signal_row::SignalRow;
use super::star_state::StarState;
use crate::theme::Icons;

/// A bounded roadmap projection and edges between its local node indices.
pub type MiniPixels = (Vec<(usize, DagNode)>, Vec<(usize, usize)>);

/// How many of the newest shipped chunks the sky keeps beside the
/// frontier. The fixture has seven shipped stars
/// and the working chunk — is that *recent* work is the constellation;
/// deep history collapses into the done counter, exactly as the list
/// collapses it into [`Roadmap::done_count`]. At or under this many
/// ships the graph is untouched, which is what keeps every fixture and
/// golden byte-identical. "Newest" is store order: chunks carry no
/// timestamps on this wire, and creation order is the recency proxy.
const SKY_RECENT_SHIPS: usize = 12;
/// Cells between dependency layers on the raw (unscaled) grid.
const COL_STEP: f32 = 6.0;
/// Minimum cells between nodes in one dependency layer on the raw grid.
const ROW_STEP: f32 = 2.0;
/// The grid's padding from the block's edge.
const MARGIN: f32 = 1.5;
/// How close (in cells) a click or hover must land to take a node.
const HIT_RADIUS: f32 = 1.6;
/// Widest the fit lets one dependency column sit from the next (cells) —
/// a three-chunk tree must not stretch across the whole block.
const MAX_COL_PITCH: f32 = 15.0;
/// Narrowest, so neighbouring dots stay distinct and clickable.
const MIN_COL_PITCH: f32 = 3.0;
/// Tallest the fit lets one grid row sit from the next (rows).
const MAX_ROW_PITCH: f32 = 2.0;
/// Shortest — half a row keeps a hundred dots apart; past this the tree
/// overflows the block and the pan takes over.
const MIN_ROW_PITCH: f32 = 0.5;
/// Nodes the rail's mini-DAG keeps: you-are-here and its closest
/// neighbours over the dependency graph.
const MINI_NODES: usize = 10;
/// Closest to the block's edge a focus counts a node as framed, in
/// cells. Under [`MARGIN`] on purpose: the grid's own origin row and
/// column sit exactly one margin in, and a pad wider than that would
/// call them off screen and re-frame the tree every time one was
/// selected.
const FOCUS_PAD: f32 = 1.0;
/// Zoomed out past this, a focus comes back to it on the way: a camera
/// that arrives at a dot too small to read has not brought it into
/// view. Zoomed in further than the fit, a focus leaves the scale alone
/// — the reader chose it.
const FOCUS_ZOOM: f32 = 1.0;

/// What the Roadmap tab shows: the chunks as the store seeded them and
/// the engines' typed events (from the feed or the engine port's own
/// answers) keep them, the signals inbox, the selection whose detail
/// card is open, the hover, and the tech-tree's camera.
///
/// Pure state: the store's snapshot arrives through [`Roadmap::seed`],
/// events through [`Roadmap::apply`]; nothing here talks to an engine.
/// Derived graph geometry is shared by clones and retained until chunk
/// data changes. Camera, viewport, hover and selection transform that
/// geometry without rerunning the dependency layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roadmap {
    chunks: Vec<RoadmapChunk>,
    geometry: GraphGeometry,
    signals: Vec<SignalRow>,
    selected: Option<String>,
    hovered: Option<String>,
    filter: Option<ChunkState>,
    sort: RoadmapSort,
    pub(super) browse: tracks::TrackBrowser,
    descending: bool,
    camera: Camera,
    viewport: Option<(f32, f32)>,
    /// A physical graph uses pixel camera units and font-sized spacing.
    pixel_line: Option<f32>,
    /// A dedicated native roadmap keeps the complete dependency graph.
    full_graph: bool,
    /// The ship moment in flight, if one is — recorded by
    /// [`Roadmap::apply`] and read back through [`Roadmap::ship_run`].
    shipped: Option<Shipped>,
    /// Every chunk this cockpit has watched complete, oldest first —
    /// one id per `chunk_completed` frame folded, deduplicated.
    ///
    /// The ship *moment* is one chunk and lives for the length of its
    /// run; the log is the whole session's and never expires, because
    /// the Brief's job is to say what happened while nobody was
    /// looking. Kept whatever the motion mode: reduced motion suppresses
    /// the light running down the edges, not the fact that the work
    /// shipped.
    shipped_log: Vec<String>,
}

/// Lazy geometry and readiness shared by viewport projections of one snapshot.
#[derive(Clone, Debug, Default)]
struct GraphGeometry {
    full: Arc<OnceLock<Geometry>>,
    frontier: Arc<OnceLock<Geometry>>,
    /// Node readiness and controls share one complete-snapshot projection.
    ready: Arc<OnceLock<BTreeSet<String>>>,
}

// Warming a derived cache does not change the roadmap's semantic state.
impl PartialEq for GraphGeometry {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Debug)]
struct Geometry {
    layout: DagLayout,
    placed: Vec<(f32, f32, usize)>,
}

/// A completion and the items it unlocked, kept by id across graph changes.
#[derive(Clone, Debug, Default, PartialEq)]
struct Shipped {
    chunk: String,
    unlocked: Vec<String>,
    since: f32,
}

impl Roadmap {
    pub(super) fn set_full_graph(&mut self, full: bool) {
        self.full_graph = full;
    }

    /// Tracks containing unfinished work, independent of the visible row filter.
    #[must_use]
    pub fn loop_tracks(&self) -> Vec<String> {
        let mut tracks: Vec<_> = self
            .chunks
            .iter()
            .filter(|chunk| chunk.is_active())
            .filter_map(|chunk| chunk.group.clone())
            .collect();
        tracks.sort();
        tracks.dedup();
        tracks
    }

    /// All unfinished work for the side panel, independent of table filters.
    /// The indices use the graph's existing selection route.
    #[must_use]
    pub fn overview_items(&self) -> Vec<(usize, &RoadmapChunk)> {
        let mut items: Vec<_> = self
            .graph_chunks()
            .into_iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.is_active() && !chunk.state.is_done())
            .collect();
        let rank = |chunk: &RoadmapChunk| {
            if chunk.state.is_working() {
                0
            } else if self.can_start(chunk) {
                1
            } else if chunk.blocker.is_some() || chunk.state == ChunkState::Blocked {
                2
            } else if chunk.state == ChunkState::Backlog {
                4
            } else {
                3
            }
        };
        items.sort_by(|(_, a), (_, b)| {
            rank(a)
                .cmp(&rank(b))
                .then(a.priority.cmp(&b.priority))
                .then(a.name.cmp(&b.name))
        });
        items
    }

    /// Assess a projected record against this complete snapshot.
    #[must_use]
    pub fn readiness<'a>(&self, chunk: &'a RoadmapChunk) -> Readiness<'a> {
        assess_readiness(
            chunk.state.status(),
            chunk.blocker.as_deref(),
            &chunk.deps,
            |id| self.chunk(id).map(|dep| dep.state.status()),
        )
    }

    /// Plain labels shared by work lists and selected documents.
    #[must_use]
    pub fn status_label(&self, chunk: &RoadmapChunk) -> &'static str {
        match self.readiness(chunk) {
            Readiness::Ready => "Ready",
            Readiness::Blocked(_) => "Blocked",
            Readiness::Waiting(_) => "Waiting",
            Readiness::Inactive(status) => match status {
                ChunkStatus::Backlog => "Planned",
                ChunkStatus::InProgress => "In progress",
                ChunkStatus::Blocked => "Blocked",
                ChunkStatus::Done => "Completed",
                ChunkStatus::Obsoleted => "Replaced",
                ChunkStatus::Pending => "Waiting",
            },
        }
    }

    /// Plain-language progress and the recorded reason an item is waiting.
    #[must_use]
    pub fn progress_detail(&self, chunk: &RoadmapChunk) -> String {
        match self.readiness(chunk) {
            Readiness::Ready => "Ready to start".into(),
            Readiness::Blocked(reason) => format!("Blocked: {reason}"),
            Readiness::Waiting(deps) => format!(
                "Waiting for {}",
                deps.iter()
                    .map(|dep| {
                        self.chunk(dep.id)
                            .map_or("an unavailable work item", |c| c.name.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Readiness::Inactive(status) => match status {
                ChunkStatus::Blocked => "Needs attention. Open this work item for details.",
                ChunkStatus::InProgress => "Work is underway",
                ChunkStatus::Backlog => "Planned for later",
                ChunkStatus::Done => "Completed",
                ChunkStatus::Obsoleted => "Replaced",
                ChunkStatus::Pending => "Waiting",
            }
            .into(),
        }
    }

    #[must_use]
    pub fn overview_summary(&self) -> String {
        let (mut working, mut ready, mut blocked, mut waiting, mut planned) = (0, 0, 0, 0, 0);
        for (_, chunk) in self.overview_items() {
            match self.readiness(chunk) {
                Readiness::Ready => ready += 1,
                Readiness::Blocked(_) | Readiness::Inactive(ChunkStatus::Blocked) => blocked += 1,
                Readiness::Waiting(_) | Readiness::Inactive(ChunkStatus::Pending) => waiting += 1,
                Readiness::Inactive(ChunkStatus::InProgress) => working += 1,
                Readiness::Inactive(ChunkStatus::Backlog) => planned += 1,
                Readiness::Inactive(ChunkStatus::Done | ChunkStatus::Obsoleted) => {}
            }
        }
        [
            (working, "working"),
            (ready, "ready"),
            (blocked, "blocked"),
            (waiting, "waiting"),
            (planned, "planned"),
        ]
        .into_iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>()
        .join(" · ")
    }

    /// Restrict table rows by status without changing the graph or selection.
    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            None => Some(ChunkState::InProgress),
            Some(ChunkState::InProgress) => Some(ChunkState::Pending),
            Some(ChunkState::Pending) => Some(ChunkState::Blocked),
            Some(ChunkState::Blocked) => Some(ChunkState::Backlog),
            Some(ChunkState::Backlog) => Some(ChunkState::Done),
            Some(ChunkState::Done) => Some(ChunkState::Obsoleted),
            _ => None,
        };
    }

    #[must_use]
    pub const fn filter_label(&self) -> &'static str {
        match self.filter {
            None => "Active",
            Some(ChunkState::InProgress) => "In progress",
            Some(ChunkState::Stuck) => "Stuck",
            Some(ChunkState::Pending) => "Pending",
            Some(ChunkState::Blocked) => "Blocked",
            Some(ChunkState::Backlog) => "Backlog",
            Some(ChunkState::Done) => "Done",
            Some(ChunkState::Obsoleted) => "Obsoleted",
        }
    }

    /// Repeating a column reverses it; another column starts ascending.
    pub fn sort_by(&mut self, column: RoadmapSort) {
        self.descending = self.sort == column && !self.descending;
        self.sort = column;
    }

    #[must_use]
    pub const fn sort(&self) -> RoadmapSort {
        self.sort
    }

    #[must_use]
    pub const fn descending(&self) -> bool {
        self.descending
    }

    /// Take the store's state whole — what the entry point read before
    /// the terminal was taken over, and what the loop driver (or the
    /// periodic re-read of an unattached cockpit) re-reads later.
    /// Selection, pan and zoom stay: a re-seed must not move the tab
    /// under the human looking at it.
    pub fn seed(&mut self, chunks: &[Chunk], signals: &[Signal]) {
        let chunks: Vec<_> = chunks.iter().map(RoadmapChunk::from_chunk).collect();
        if self.chunks != chunks {
            self.chunks = chunks;
            self.geometry = GraphGeometry::default();
        }
        self.signals = signals.iter().map(SignalRow::from_signal).collect();
    }

    /// Fold one typed event in: a roadmap chunk event upserts its chunk,
    /// a signal event its signal. Every other event is ignored.
    ///
    /// A frame that puts a chunk *into* progress also moves the camera
    /// onto it ([`Roadmap::focus`]) — the completion frame the tech-tree
    /// is meant to follow. Only the transition does: a chunk that was
    /// already in progress re-broadcasting its detail leaves the view
    /// where the reader put it.
    ///
    /// A `chunk_completed` frame additionally opens the ship moment
    /// ([`Roadmap::ship_run`]): the chunks whose every dependency has
    /// *now* shipped and which were not ready before are what the
    /// completion unlocked, so the edge-run's targets are read off the
    /// same [`assess_readiness`] predicate the constellation's own
    /// [`StarState`] is derived through — the light cannot run to a
    /// star the sky draws as still waiting.
    ///
    /// Under reduced motion no run is recorded at all. That is what
    /// settles the view structurally rather
    /// than a curve pinned at its end: there is nothing to pin.
    pub fn apply(&mut self, event: &NativeEvent, now: f32, animate: bool) {
        match event {
            NativeEvent::Roadmap(frame) => {
                let (chunk, completed) = match frame {
                    RoadmapFrame::ChunkAdded { chunk }
                    | RoadmapFrame::ChunkChanged { chunk }
                    | RoadmapFrame::ChunkObsoleted { chunk } => (chunk, false),
                    RoadmapFrame::ChunkCompleted { chunk } => (chunk, true),
                    RoadmapFrame::RefreshRecorded { .. } => return,
                };
                let chunk = RoadmapChunk::from_chunk(chunk);
                let arrived = chunk.is_in_progress();
                let id = chunk.id.clone();
                let ships = completed && animate;
                let before = ships.then(|| self.ready_ids());
                let was = match self.chunks.iter_mut().find(|c| c.id == chunk.id) {
                    Some(existing) => {
                        let was = existing.is_in_progress();
                        if *existing != chunk {
                            *existing = chunk;
                            self.geometry = GraphGeometry::default();
                        }
                        was
                    }
                    None => {
                        self.chunks.push(chunk);
                        self.geometry = GraphGeometry::default();
                        false
                    }
                };
                if completed && !self.shipped_log.contains(&id) {
                    self.shipped_log.push(id.clone());
                }
                if let Some(before) = before {
                    let unlocked = self
                        .ready_ids()
                        .into_iter()
                        .filter(|id| !before.contains(id))
                        .collect();
                    self.shipped = Some(Shipped {
                        chunk: id.clone(),
                        unlocked,
                        since: now,
                    });
                }
                if arrived && !was {
                    self.focus(&id, now, animate);
                }
            }
            NativeEvent::Signal(frame) => {
                let (SignalFrame::SignalCaptured { signal }
                | SignalFrame::SignalChanged { signal }
                | SignalFrame::SignalDismissed { signal }) = frame;
                let signal = SignalRow::from_signal(signal);
                match self.signals.iter_mut().find(|s| s.id == signal.id) {
                    Some(existing) => *existing = signal,
                    None => self.signals.push(signal),
                }
            }
            NativeEvent::Think(_) | NativeEvent::Ship(_) | NativeEvent::Magistr(_) => {}
        }
    }

    /// Visible table rows, filtered and sorted independently of selection.
    /// Active work is the default; the Done filter exposes shipped items.
    #[must_use]
    pub fn rows(&self) -> Vec<&RoadmapChunk> {
        let mut rows: Vec<_> = self
            .chunks
            .iter()
            .filter(|c| {
                if !self.browse.query.is_empty() {
                    let query = self.browse.query.to_lowercase();
                    format!(
                        "{} {} {} {}",
                        c.name,
                        c.title,
                        c.description,
                        self.track_of(c)
                    )
                    .to_lowercase()
                    .contains(&query)
                } else {
                    self.browse.track.as_deref().is_none_or(|track| {
                        c.group.as_deref().map(str::trim).unwrap_or("") == track
                    }) && match self.filter {
                        None => c.is_active() && !c.state.is_done(),
                        Some(ChunkState::InProgress) => c.state.is_working(),
                        Some(state) => c.state == state,
                    }
                }
            })
            .collect();
        rows.sort_by(|a, b| {
            let order = match self.sort {
                RoadmapSort::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                RoadmapSort::Status => {
                    readiness_rank(&self.readiness(a)).cmp(&readiness_rank(&self.readiness(b)))
                }
                RoadmapSort::Priority => a.priority.cmp(&b.priority),
                RoadmapSort::Workstream => a.group.cmp(&b.group),
            }
            .then(a.priority.cmp(&b.priority))
            .then(a.id.cmp(&b.id));
            if self.descending {
                order.reverse()
            } else {
                order
            }
        });
        rows
    }

    /// The shipped chunks in store order — the book's chapters, what
    /// ⌘⇧F's `reports` reads. (Not [`Roadmap::shipped`], which is the
    /// ids that shipped *since the reader left*.)
    #[must_use]
    pub fn chapters(&self) -> Vec<&RoadmapChunk> {
        self.chunks.iter().filter(|c| c.state.is_done()).collect()
    }

    /// Chunks done — the list shows them as one count, not rows.
    #[must_use]
    pub fn done_count(&self) -> usize {
        self.chunks.iter().filter(|c| c.state.is_done()).count()
    }

    /// Chunks still in the plan — the launcher's progress denominator.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.chunks.iter().filter(|c| c.is_active()).count()
    }

    /// The next ready chunk, as the engine would pick it: pending, every
    /// dep the tab knows already done, lowest priority first — the
    /// launcher's `next` line.
    #[must_use]
    pub fn next_ready(&self) -> Option<&RoadmapChunk> {
        next_ready(
            &self.chunks,
            |chunk| self.can_start(chunk),
            |chunk| chunk.priority,
        )
    }

    /// Whether the selected work can ask to start from the current snapshot.
    /// The command owner still validates permissions and freshness at execution.
    #[must_use]
    pub fn can_start(&self, chunk: &RoadmapChunk) -> bool {
        self.geometry
            .ready
            .get_or_init(|| {
                let done: BTreeSet<_> = self
                    .chunks
                    .iter()
                    .filter(|c| c.state.is_done())
                    .map(|c| c.id.as_str())
                    .collect();
                self.chunks
                    .iter()
                    .filter(|c| {
                        assess_readiness(c.state.status(), c.blocker.as_deref(), &c.deps, |id| {
                            done.contains(id).then_some(ChunkStatus::Done)
                        })
                        .is_ready()
                    })
                    .map(|c| c.id.clone())
                    .collect()
            })
            .contains(&chunk.id)
    }

    /// Whether the selected work can ask to complete; verification remains authoritative.
    #[must_use]
    pub fn can_complete(&self, chunk: &RoadmapChunk) -> bool {
        chunk.state == ChunkState::InProgress && chunk.blocker.is_none()
    }

    /// The heading's one-line accounting: done, the working set by
    /// status, and the open proposals and signals when any wait.
    #[must_use]
    pub fn summary_line(&self) -> String {
        let count = |state: ChunkState| self.chunks.iter().filter(|c| c.state == state).count();
        let mut line = format!(
            "{} done · {} active · {} pending · {} backlog",
            self.done_count(),
            count(ChunkState::InProgress),
            count(ChunkState::Pending),
            count(ChunkState::Backlog),
        );
        let proposals = self.proposals().len();
        if proposals > 0 {
            line.push_str(&format!(" · {proposals} proposals"));
        }
        let signals = self.signals().len();
        if signals > 0 {
            line.push_str(&format!(" · {signals} signals"));
        }
        line
    }

    /// The chunk by id, when the tab has it.
    #[must_use]
    pub fn chunk(&self, id: &str) -> Option<&RoadmapChunk> {
        self.chunks.iter().find(|c| c.id == id)
    }

    /// The state the chunk's star wears — its status, with readiness
    /// derived from what it stands on — or `None` for a chunk the
    /// snapshot does not contain. The same complete-snapshot derivation
    /// governs graph nodes, work cards and start controls.
    #[must_use]
    pub fn star_state(&self, id: &str) -> Option<StarState> {
        self.chunk(id)
            .map(|chunk| star_state(chunk, self.can_start(chunk)))
    }

    /// The active chunks still to come that stand on `id` directly —
    /// what shipping it unlocked, in store order.
    #[must_use]
    pub fn unlocked_by(&self, id: &str) -> Vec<&RoadmapChunk> {
        self.chunks
            .iter()
            .filter(|c| c.is_active() && !c.state.is_done() && c.deps.iter().any(|dep| dep == id))
            .collect()
    }

    /// The chunk a cross-reference names, when the tab has one.
    ///
    /// Three shapes reach the same chunk, because the three families
    /// spell the same edge three ways. An explicit `chunk:<id>` and a
    /// bare id are the chunk naming itself. The third is the one that
    /// matters in a real trace: a think step's `execution_ref` is a
    /// *ship* entity (`task:…`, `check:…`), never a chunk id, and the
    /// edge exists because `roadmap_link` wrote that ref into the
    /// chunk's own `cross_refs` — so the chunk points back, and the
    /// resolution follows the arrow the other way.
    ///
    /// `None` for a ref no chunk answers. That is what keeps an
    /// unresolvable ref plain text rather than a link that goes
    /// nowhere, and done chunks stay reachable on purpose: they leave
    /// [`Roadmap::rows`] but a step that points at one must still get
    /// there.
    #[must_use]
    pub fn chunk_for_ref(&self, reference: &str) -> Option<&RoadmapChunk> {
        if reference.is_empty() {
            return None;
        }
        self.chunk(reference.strip_prefix("chunk:").unwrap_or(reference))
            .or_else(|| {
                self.chunks
                    .iter()
                    .find(|c| c.cross_refs.iter().any(|r| r == reference))
            })
    }

    /// The chunk whose detail card is open.
    #[must_use]
    pub fn selected(&self) -> Option<&RoadmapChunk> {
        self.selected.as_deref().and_then(|id| self.chunk(id))
    }

    /// A click on the list row at visible `index`: its detail opens, or
    /// folds when it already shows — one detail at a time. Opening one
    /// also brings its node into the tech-tree's frame
    /// ([`Roadmap::focus`]); folding leaves the camera where it is,
    /// since nothing new needs looking at.
    pub fn toggle_row(&mut self, index: usize, now: f32, animate: bool) {
        let Some(id) = self.rows().get(index).map(|c| c.id.clone()) else {
            return;
        };
        if self.selected.as_deref() == Some(&id) {
            self.selected = None;
            return;
        }
        self.selected = Some(id.clone());
        self.focus(&id, now, animate);
    }

    /// Select the chunk by id, when the tab has it, and bring it into
    /// the tech-tree's frame; whether it did. The programmatic way in —
    /// a jump from the trace, a chunk named from elsewhere.
    pub fn select(&mut self, id: &str, now: f32, animate: bool) -> bool {
        if self.chunk(id).is_some() {
            self.selected = Some(id.to_owned());
            self.focus(id, now, animate);
            return true;
        }
        false
    }

    /// Move the camera until the tech-tree's frame contains the node for
    /// `id`, gliding there over [`Eased::STEPS`](super::Eased::STEPS)
    /// when `animate`; whether the graph has such a node.
    ///
    /// The rule is containment, not showmanship. A node already inside
    /// the block by [`FOCUS_PAD`] at a zoom of at least [`FOCUS_ZOOM`]
    /// is left exactly where it is — a camera that lunges at something
    /// the reader can already see is noise. Otherwise the node goes to
    /// the middle of the block (clamped to the pan's room, so the edges
    /// of the tree still butt up against the edges of the block) and a
    /// camera that was zoomed out past the fit comes back to it.
    ///
    /// A tab with no graph block — the cell-only path, where the
    /// tech-tree is a legend rather than a picture — has no camera to
    /// move, so a focus there is a no-op and never asks for a tick.
    pub fn focus(&mut self, id: &str, now: f32, animate: bool) -> bool {
        let Some(target) = self.frame_for(id) else {
            return false;
        };
        self.camera.glide_to(target.0, target.1, now, animate);
        true
    }

    /// The `(pan, zoom)` a focus on `id` wants, or `None` when the graph
    /// has no such node or already frames it.
    fn frame_for(&self, id: &str) -> Option<((f32, f32), f32)> {
        let (cols, rows) = self.viewport?;
        let graph = self.graph_chunks();
        let at = graph.iter().position(|c| c.id == id)?;
        let placed = &self.geometry().placed;
        let (gx, gy, _) = placed[at];
        let zoom = self.camera.zoom().max(FOCUS_ZOOM);
        let (sx, sy) = self.scale_with(placed, self.camera.zoom());
        let (pan_x, pan_y) = self.camera.pan();
        let margin = self.margin();
        let (x, y) = (margin + gx * sx - pan_x, margin + gy * sy - pan_y);
        let pad = self.pixel_line.map_or(FOCUS_PAD, |line| line + 12.0);
        let inside = |v: f32, span: f32| v >= pad && v <= span - pad;
        if inside(x, cols) && inside(y, rows) && zoom <= self.camera.zoom() {
            return None;
        }
        let (sx, sy) = self.scale_with(placed, zoom);
        let (cx, cy) = (cols / 2.0, rows / 2.0);
        let (max_x, max_y) = self.pan_limits_with(zoom);
        Some((
            (
                (margin + gx * sx - cx).clamp(0.0, max_x),
                (margin + gy * sy - cy).clamp(0.0, max_y),
            ),
            zoom,
        ))
    }

    /// The hovered chunk's line for the tooltip row: `name — title`.
    #[must_use]
    pub fn hovered_title(&self) -> Option<String> {
        let chunk = self.hovered.as_deref().and_then(|id| self.chunk(id))?;
        Some(format!("{} — {}", chunk.name, chunk.title))
    }

    /// Every open proposal across the roadmap: the chunk, the kind, the
    /// suggested value and the reason, in list order.
    #[must_use]
    pub fn proposals(&self) -> Vec<(&RoadmapChunk, ProposalKind, &str, &str)> {
        self.chunks
            .iter()
            .filter(|chunk| chunk.is_active() && !chunk.state.is_done())
            .flat_map(|chunk| {
                chunk
                    .proposals
                    .iter()
                    .map(move |(kind, suggestion, reason)| {
                        (chunk, *kind, suggestion.as_str(), reason.as_str())
                    })
            })
            .collect()
    }

    /// The signals still in triage, capture order — the list every
    /// triage button indexes into.
    #[must_use]
    pub fn signals(&self) -> Vec<&SignalRow> {
        self.signals.iter().filter(|s| s.is_open()).collect()
    }

    /// The whole inbox as the Signals view reads it (view 12): every
    /// signal but the dismissed, capture order — a promoted signal stays
    /// forever, naming the star it became. Verbs are minted off
    /// [`Roadmap::signals`], so a promoted card ahead of an open one
    /// cannot shift a click.
    #[must_use]
    pub fn inbox(&self) -> Vec<&SignalRow> {
        self.signals
            .iter()
            .filter(|s| s.status != "dismissed")
            .collect()
    }

    /// Whether the tab has nothing to show yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.signals.is_empty()
    }

    /// Table items and signal rows. Empty filters retain a note; headers are
    /// pinned, and selected object fields/actions occupy the inspector.
    #[must_use]
    pub fn body_rows(&self, _width: u16) -> usize {
        self.track_rows().len().max(1)
    }

    /// One line per workstream in tier order — what the tech-tree block
    /// shows where the pixel DAG would sit: `«group» done/total`.
    #[must_use]
    pub fn band_rows(&self) -> Vec<String> {
        self.bands()
            .into_iter()
            .map(|(group, done, total)| format!("«{group}» {done}/{total}"))
            .collect()
    }

    /// The workstreams in tier order — `(group, done, total)` each —
    /// the order the tech-tree numbers its regions in, so the legend's
    /// n-th band wears the painter's n-th tint.
    #[must_use]
    pub fn bands(&self) -> Vec<(String, usize, usize)> {
        let graph = self.graph_chunks();
        let mut bands: Vec<(&str, u32, usize, usize)> = Vec::new();
        for chunk in &graph {
            let group = chunk.group.as_deref().unwrap_or("");
            let done = usize::from(chunk.state.is_done());
            match bands.iter_mut().find(|(name, ..)| *name == group) {
                Some((_, best, done_count, total)) => {
                    *best = (*best).min(chunk.priority);
                    *done_count += done;
                    *total += 1;
                }
                None => bands.push((group, chunk.priority, done, 1)),
            }
        }
        bands.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)));
        bands
            .into_iter()
            .map(|(group, _, done, total)| {
                let name = if group.is_empty() { "ungrouped" } else { group };
                (name.to_owned(), done, total)
            })
            .collect()
    }

    /// The graph block's placed size in cells — what the runtime reports
    /// after layout. The tech-tree scales to fit it (pitch capped and
    /// floored), so the pan only matters when the floor overflows; the
    /// pan re-clamps to the new limits.
    pub fn set_viewport(&mut self, viewport: Option<(u16, u16)>) {
        self.set_graph_viewport(viewport.map(|(w, h)| (f32::from(w), f32::from(h))), None);
    }

    /// Exact physical graph size. The camera, routes and picking share pixels.
    pub fn set_pixel_viewport(&mut self, viewport: Option<(f32, f32)>, line_height: f32) {
        self.set_graph_viewport(viewport, Some(line_height.max(1.0)));
    }

    fn set_graph_viewport(&mut self, viewport: Option<(f32, f32)>, pixel_line: Option<f32>) {
        if self.viewport == viewport && self.pixel_line == pixel_line {
            return;
        }
        if self.pixel_line != pixel_line {
            self.camera = Camera::new();
        }
        self.viewport = viewport;
        self.pixel_line = pixel_line;
        self.pan_by(0.0, 0.0);
        // Selection can arrive before the first layout or survive a resize.
        // Place it immediately: the previous viewport is no longer drawable.
        if let Some(id) = self.selected.clone() {
            self.focus(&id, 0.0, false);
        }
    }

    /// Drag on the tech-tree: the view under the pan moves with the
    /// pointer, clamped to the overflow past the block — zero when the
    /// whole tree fits. Instant, whatever a focus still owes.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        let limits = self.pan_limits();
        self.camera.drag(dx, dy, limits);
    }

    /// The settled pan in the graph's units: pixels on the full plan, cells otherwise —
    /// which is where it already is unless a focus is still gliding.
    #[must_use]
    pub const fn pan(&self) -> (f32, f32) {
        self.camera.pan()
    }

    /// Whether the tab still has a frame to draw at step `now`, so the
    /// runtime keeps its tick: the camera catching up with a focus, or
    /// a ship moment still running. Nothing periodic belongs here — the
    /// twinkle and the breath hold their phase on a still cockpit
    /// rather than asking for a tick of their own.
    #[must_use]
    pub fn is_gliding(&self, now: f32) -> bool {
        self.is_panning(now) || self.is_shipping(now)
    }

    /// Whether the camera alone is still catching up with a focus at
    /// step `now` — the pan *parallax* is coupled to,
    /// without the ship moment [`Roadmap::is_gliding`] folds in.
    #[must_use]
    pub fn is_panning(&self, now: f32) -> bool {
        self.camera.is_gliding(now)
    }

    /// Whether a ship moment is running at step `now` — the cheap half
    /// of [`Roadmap::ship_run`], which is the one the per-frame tick
    /// predicate asks. Resolving the run's graph indices places the
    /// whole tree, and a completion that has long since settled must
    /// not pay for that on every frame for the rest of the session.
    fn is_shipping(&self, now: f32) -> bool {
        self.shipped
            .as_ref()
            .is_some_and(|shipped| ShipRun::is_live(shipped.since, now))
    }

    /// A wheel notch on the tech-tree at block-relative cell `(x, y)`:
    /// the scale multiplies by `factor` (clamped to [`MIN_ZOOM`] ..
    /// [`MAX_ZOOM`]) about that point, so whatever sits under the
    /// pointer stays under it. Every placement — the nodes, the pan
    /// limits and the hit test — shares the same viewport scale, so
    /// they all follow.
    pub fn zoom_at(&mut self, factor: f32, x: f32, y: f32) {
        // The pan's room depends on the zoom the notch is about to set,
        // so the notch runs unclamped and the re-clamp follows it.
        self.camera
            .wheel(factor, x, y, self.margin(), (f32::INFINITY, f32::INFINITY));
        self.pan_by(0.0, 0.0);
    }

    /// The zoom in force: 1.0 is the fit the viewport picked. The
    /// settled one, like [`Roadmap::pan`].
    #[must_use]
    pub const fn zoom(&self) -> f32 {
        self.camera.zoom()
    }

    /// A keyboard zoom step on the tech-tree: the scale multiplies by
    /// `factor` about the block's own centre, since a key press has no
    /// pointer to keep a point under. The paint glides in from the
    /// scale it was showing ([`Camera::zoom_step`]).
    ///
    /// Says whether there was a camera to move. A cell-only tab has
    /// none — the tech-tree is a legend there rather than a picture,
    /// the same rule [`Roadmap::focus`] follows — and answers `false`,
    /// which is how the caller knows to leave the key to the prompt.
    pub fn zoom_step(&mut self, factor: f32, now: f32, animate: bool) -> bool {
        let Some((cols, rows)) = self.viewport else {
            return false;
        };
        let (cx, cy) = (cols / 2.0, rows / 2.0);
        self.camera
            .zoom_step(factor, cx, cy, self.margin(), now, animate);
        self.pan_by(0.0, 0.0);
        true
    }

    /// Put the tech-tree back to the fit: the zoom to 1.0 and the pan to
    /// nothing, glided like any other move nothing is pointing at. Says
    /// whether there was a camera to reset, like [`Roadmap::zoom_step`].
    pub fn reset_view(&mut self, now: f32, animate: bool) -> bool {
        if self.viewport.is_none() {
            return false;
        }
        self.camera.glide_to((0.0, 0.0), FOCUS_ZOOM, now, animate);
        true
    }

    /// The tech-tree's scale as the tab shows it — `zoom 300%`, a share
    /// of the fit — or `None` at the fit itself, so the ordinary tab
    /// grows no chrome for a zoom nobody used.
    ///
    /// A share reads at every scale a multiplier does not: `×0.2` at
    /// [`Camera::MIN_ZOOM`] rounds a quarter away, `25%` does not. And
    /// the silence is decided on the *rounded* percentage rather than on
    /// the float, which is what makes a pair of notches that cancel
    /// leave the tab exactly as it was rather than printing `zoom 100%`
    /// off a residue.
    #[must_use]
    pub fn scale_label(&self) -> Option<String> {
        let percent = (self.camera.zoom() * 100.0).round();
        ((percent - 100.0).abs() >= 0.5).then(|| format!("zoom {percent:.0}%"))
    }

    /// A hover at graph-relative `(x, y)` in its camera units: the nearest node within
    /// reach becomes the tooltip's chunk; past every node it clears.
    pub fn hover_at(&mut self, x: f32, y: f32, now: f32) {
        self.hovered = self.node_at(x, y, now).map(|(_, chunk)| chunk.id.clone());
    }

    /// A click at graph-relative `(x, y)` in its camera units: the nearest node within
    /// reach is selected; whether one was.
    pub fn click_at(&mut self, x: f32, y: f32, now: f32) -> bool {
        if let Some((_, chunk)) = self.node_at(x, y, now) {
            self.selected = Some(chunk.id.clone());
            return true;
        }
        false
    }

    /// The tech-tree's nodes in graph order (parallel to
    /// [`Roadmap::dag_edges`] indices), scaled to the viewport and
    /// placed in the graph's camera units with the pan applied.
    #[must_use]
    pub fn dag_nodes(&self) -> Vec<DagNode> {
        self.dag_nodes_at(f32::INFINITY)
    }

    /// The tech-tree's nodes as they paint at step `now` — the settled
    /// placement with whatever of a focus's glide has not landed yet.
    /// The one the driver and the hit tests use; at the end
    /// of every glide it is exactly [`Roadmap::dag_nodes`].
    #[must_use]
    pub fn dag_nodes_at(&self, now: f32) -> Vec<DagNode> {
        let graph = self.graph_chunks();
        let placed = &self.geometry().placed;
        let (sx, sy) = self.scale_with(placed, self.camera.zoom_at(now));
        let (pan_x, pan_y) = self.camera.pan_at(now);
        graph
            .iter()
            .zip(placed)
            .map(|(chunk, &(x, y, region))| DagNode {
                x: self.margin() + x * sx - pan_x,
                y: self.margin() + y * sy - pan_y,
                state: star_state(chunk, self.can_start(chunk)),
                status: chunk.state,
                region,
                here: chunk.is_in_progress(),
                selected: self.selected.as_deref() == Some(chunk.id.as_str()),
            })
            .collect()
    }

    /// Routed dependency paths, transformed by the same fit and camera as nodes.
    #[must_use]
    pub fn dag_paths_at(&self, now: f32) -> Vec<DagPath> {
        let geometry = self.geometry();
        let (sx, sy) = self.scale_with(&geometry.placed, self.camera.zoom_at(now));
        let (pan_x, pan_y) = self.camera.pan_at(now);
        geometry
            .layout
            .paths
            .iter()
            .cloned()
            .map(|mut path| {
                for (x, y) in &mut path.points {
                    *x = self.margin() + *x * sx - pan_x;
                    *y = self.margin() + *y * sy - pan_y;
                }
                path
            })
            .collect()
    }

    /// The chunk ids in the order [`Roadmap::dag_nodes_at`] places
    /// them — how a reading that names chunks (the bad day's ringed
    /// stars) finds its node.
    #[must_use]
    pub fn graph_ids(&self) -> Vec<&str> {
        self.graph_chunks()
            .into_iter()
            .map(|chunk| chunk.id.as_str())
            .collect()
    }

    /// The dependency edges as graph-order index pairs `(dep, chunk)`.
    #[must_use]
    pub fn dag_edges(&self) -> Vec<(usize, usize)> {
        let graph = self.graph_chunks();
        let mut edges = Vec::new();
        for (to, chunk) in graph.iter().enumerate() {
            for dep in &chunk.deps {
                if let Some(from) = graph.iter().position(|c| &c.id == dep) {
                    edges.push((from, to));
                }
            }
        }
        edges
    }

    /// The dependency edges with the kind each is drawn in: an edge out
    /// of a shipped chunk is a road walked ([`EdgeKind::Done`]), and one
    /// out of anything else is still ahead ([`EdgeKind::Ahead`]).
    ///
    /// Parallel to [`Roadmap::dag_edges`], which the cell painter keeps
    /// using: a terminal draws one edge and has nowhere to put a second
    /// kind.
    #[must_use]
    pub fn dag_edge_kinds(&self) -> Vec<(usize, usize, EdgeKind)> {
        let graph = self.graph_chunks();
        self.dag_edges()
            .into_iter()
            .map(|(from, to)| {
                let kind = graph
                    .get(from)
                    .map_or(EdgeKind::Ahead, |chunk| EdgeKind::of(chunk.state));
                (from, to, kind)
            })
            .collect()
    }

    /// The ship moment the sky is drawing at step `now`, or `None`
    /// when none is running — which is every frame of a settled sky
    /// and every frame of a reduced-motion one.
    ///
    /// The stars come back as graph indices, resolved here rather than
    /// stored: a frame that arrives between the completion and the end
    /// of the run can renumber the graph, and a stale index would light
    /// the wrong star. A target whose chunk has left the graph is
    /// dropped; a shipped chunk that has is no run at all.
    #[must_use]
    pub fn ship_run(&self, now: f32) -> Option<ShipRun> {
        if !self.is_shipping(now) {
            return None;
        }
        let shipped = self.shipped.as_ref()?;
        let graph = self.graph_chunks();
        let index = |id: &str| graph.iter().position(|c| c.id == id);
        let star = index(&shipped.chunk)?;
        let region = self.dag_nodes().get(star)?.region;
        let unlocked = shipped.unlocked.iter().filter_map(|id| index(id)).collect();
        ShipRun::at(star, region, unlocked, shipped.since, now)
    }

    /// Every chunk this cockpit watched complete, oldest first — what
    /// the Brief's `Done · N` card is dealt from. Empty on a cockpit
    /// that only ever read a seed: a store full of done chunks is
    /// history, and only a completion the cockpit was there for is
    /// news.
    #[must_use]
    pub fn shipped(&self) -> &[String] {
        &self.shipped_log
    }

    /// The active chunks whose every dependency has shipped, by id —
    /// the readiness [`StarState`] is derived through, taken as a set
    /// so a completion can be diffed across.
    fn ready_ids(&self) -> Vec<String> {
        self.chunks
            .iter()
            .filter(|chunk| self.can_start(chunk))
            .map(|chunk| chunk.id.clone())
            .collect()
    }

    /// The chunk at graph `index` — the index space
    /// [`Roadmap::dag_nodes`] and [`Roadmap::dag_edges`] share, and the
    /// one a sky star's widget id carries.
    #[must_use]
    pub fn graph_chunk(&self, index: usize) -> Option<&RoadmapChunk> {
        self.graph_chunks().get(index).copied()
    }

    /// Retain topology geometry independently of camera and viewport transforms.
    fn geometry(&self) -> &Geometry {
        let slot = if self.full_graph || self.browse.dependencies || self.pixel_line.is_some() {
            &self.geometry.full
        } else {
            &self.geometry.frontier
        };
        slot.get_or_init(|| {
            let graph = self.graph_chunks();
            let layout = graph_layout(&graph);
            let placed = place_layout(&graph, &layout);
            Geometry { layout, placed }
        })
    }

    /// The graph's chunks in store order — the index space `dag_nodes`
    /// and `dag_edges` share, and the one a sky star's widget id
    /// carries.
    ///
    /// At real scale the constellation is the **frontier**: everything
    /// living (in progress, pending, blocked, backlog), the shipped
    /// chunks the living set directly stands on, and the newest
    /// [`SKY_RECENT_SHIPS`] ships. Every other done chunk is history
    /// the counter already tells; drawing four hundred stars made the
    /// sky a hairball nobody could read. With no more ships than the
    /// floor the set is exactly the active set it always was, so a
    /// fixture-scale graph — and every golden cut from one — does not
    /// move. Edges resolve by id inside this set, so a hidden star's
    /// edges vanish with it and no index dangles.
    fn graph_chunks(&self) -> Vec<&RoadmapChunk> {
        let active: Vec<&RoadmapChunk> = self.chunks.iter().filter(|c| c.is_active()).collect();
        let ships = active.iter().filter(|c| c.state.is_done()).count();
        if self.full_graph || self.pixel_line.is_some() || ships <= SKY_RECENT_SHIPS {
            return active;
        }
        // What the living set stands on rides as context, however old.
        let context: std::collections::HashSet<&str> = active
            .iter()
            .filter(|c| !c.state.is_done())
            .flat_map(|c| c.deps.iter().map(String::as_str))
            .collect();
        // The newest ships, by store order from the tail.
        let mut recent_left = SKY_RECENT_SHIPS;
        let mut keep: Vec<bool> = vec![false; active.len()];
        for (index, chunk) in active.iter().enumerate().rev() {
            if !chunk.state.is_done() || context.contains(chunk.id.as_str()) {
                keep[index] = true;
            } else if recent_left > 0 {
                keep[index] = true;
                recent_left -= 1;
            }
        }
        active
            .into_iter()
            .zip(keep)
            .filter_map(|(chunk, kept)| kept.then_some(chunk))
            .collect()
    }

    /// The rail's mini-DAG, fitted to a `(cols, rows)` block: at most
    /// [`MINI_NODES`] chunks — you-are-here and its nearest dependency
    /// neighbours, or the working set's head when nothing is in
    /// progress — placed like the tech-tree, with the dependency edges
    /// inside the kept set. No pan, no selection: the rail is a glance,
    /// not a workbench.
    #[must_use]
    pub fn mini_dag(&self, cols: u16, rows: u16) -> (Vec<DagNode>, Vec<(usize, usize)>) {
        let graph = self.graph_chunks();
        let kept: Vec<&RoadmapChunk> = mini_set(&graph).into_iter().map(|at| graph[at]).collect();
        let placed = place(&kept);
        // Cap the pitch like the big tree, but no floor: the mini has
        // no pan, so it always fits the block — at ten nodes the dots
        // stay distinct anyway.
        let (span_x, span_y) = raw_span(&placed);
        let usable_w = (f32::from(cols) - 2.0 * MARGIN).max(1.0);
        let usable_h = (f32::from(rows) - 2.0 * MARGIN).max(1.0);
        let fit = |usable: f32, span: f32| if span > 0.0 { usable / span } else { 1.0 };
        let sx = fit(usable_w, span_x).min(MAX_COL_PITCH / COL_STEP);
        let sy = fit(usable_h, span_y).min(MAX_ROW_PITCH / ROW_STEP);
        let nodes = kept
            .iter()
            .zip(&placed)
            .map(|(chunk, &(x, y, region))| DagNode {
                x: MARGIN + x * sx,
                y: MARGIN + y * sy,
                state: star_state(chunk, self.can_start(chunk)),
                status: chunk.state,
                region,
                here: chunk.is_in_progress(),
                selected: false,
            })
            .collect();
        let mut edges = Vec::new();
        for (to, chunk) in kept.iter().enumerate() {
            for dep in &chunk.deps {
                if let Some(from) = kept.iter().position(|c| &c.id == dep) {
                    edges.push((from, to));
                }
            }
        }
        (nodes, edges)
    }

    /// The recorder's roadmap neighborhood in physical pixels. Global graph
    /// indices accompany nodes so labels and pointer targets keep the same identity.
    #[must_use]
    pub fn mini_pixels(&self, width: f32, height: f32) -> MiniPixels {
        let graph = self.graph_chunks();
        let mut indices = mini_set(&graph);
        // A compact map shows the nearby work that can remain individually
        // readable. The full list retains every item through Open Roadmap.
        let original: Vec<_> = indices.iter().map(|&index| graph[index]).collect();
        let original_positions = place(&original);
        let focus = original
            .iter()
            .position(|chunk| chunk.is_in_progress())
            .unwrap_or(0);
        let focus_x = original_positions.get(focus).map_or(0.0, |point| point.0);
        let mut ranks: Vec<f32> = original_positions.iter().map(|point| point.0).collect();
        ranks.sort_by(f32::total_cmp);
        ranks.dedup();
        ranks.sort_by(|a, b| (a - focus_x).abs().total_cmp(&(b - focus_x).abs()));
        ranks.truncate(((width / 90.0) as usize).clamp(1, 3));
        let per_rank = (((height - 20.0).max(0.0) / 20.0) as usize + 1).clamp(1, 3);
        let mut selected = Vec::new();
        for rank in ranks {
            let mut members: Vec<_> = indices
                .iter()
                .copied()
                .zip(&original_positions)
                .filter(|(_, point)| point.0 == rank)
                .map(|(index, _)| index)
                .collect();
            members
                .sort_by_key(|&index| (activity_rank(graph[index]), graph[index].priority, index));
            members.truncate(per_rank);
            selected.extend(members);
        }
        indices.retain(|index| selected.contains(index));
        let kept: Vec<_> = indices.iter().map(|&index| graph[index]).collect();
        let placed = place(&kept);
        let (span_x, span_y) = raw_span(&placed);
        let side = 15.0_f32.min(width / 4.0);
        let sx = if span_x > 0.0 {
            ((width - side * 2.0).max(0.0) / span_x).min(150.0 / COL_STEP)
        } else {
            1.0
        };
        let top = (height * 0.5).min(if span_y > ROW_STEP { 10.0 } else { 30.0 });
        let sy = if span_y > 0.0 {
            ((height - top - 10.0).max(0.0) / span_y).min(20.0 / ROW_STEP)
        } else {
            1.0
        };
        let nodes = kept
            .iter()
            .zip(&placed)
            .zip(&indices)
            .map(|((chunk, &(x, y, region)), &index)| {
                (
                    index,
                    DagNode {
                        x: if span_x > 0.0 {
                            side + x * sx
                        } else {
                            width / 2.0
                        },
                        y: if span_y > 0.0 {
                            top + y * sy
                        } else {
                            height * 0.5
                        },
                        state: star_state(chunk, self.can_start(chunk)),
                        status: chunk.state,
                        region,
                        here: chunk.is_in_progress(),
                        selected: self.selected.as_deref() == Some(chunk.id.as_str()),
                    },
                )
            })
            .collect();
        let mut edges = Vec::new();
        for (to, chunk) in kept.iter().enumerate() {
            for dep in &chunk.deps {
                if let Some(from) = kept.iter().position(|candidate| &candidate.id == dep) {
                    edges.push((from, to));
                }
            }
        }
        (nodes, edges)
    }

    /// The mini-DAG as two text lines for the cell-only rail:
    /// you-are-here (or the wait for one) and its immediate dependency
    /// neighbourhood.
    #[must_use]
    pub fn mini_summary(&self, icons: &Icons) -> [String; 2] {
        let [here, neighbours] = self.mini_glance();
        [format!("{} {here}", icons.here), neighbours]
    }

    /// The glance's two lines without the `here` mark: what stands
    /// after it — the chunk in progress, or `nothing in progress` —
    /// and its neighbours' count, or the pending count.
    #[must_use]
    pub fn mini_glance(&self) -> [String; 2] {
        let graph = self.graph_chunks();
        let Some(here) = graph.iter().find(|c| c.is_in_progress()) else {
            let pending = graph
                .iter()
                .filter(|c| c.state == ChunkState::Pending)
                .count();
            return [
                "nothing in progress".to_owned(),
                format!("{pending} pending"),
            ];
        };
        let before = here
            .deps
            .iter()
            .filter(|dep| graph.iter().any(|c| &&c.id == dep))
            .count();
        let after = graph.iter().filter(|c| c.deps.contains(&here.id)).count();
        [
            here.name.clone(),
            format!("{before} before · {after} after"),
        ]
    }

    /// The nearest node to block-relative `(x, y)` within reach.
    fn node_at(&self, x: f32, y: f32, now: f32) -> Option<(usize, &RoadmapChunk)> {
        let graph = self.graph_chunks();
        let nodes = self.dag_nodes_at(now);
        nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let (dx, dy) = (node.x - x, node.y - y);
                (index, dx.mul_add(dx, dy * dy))
            })
            .filter(|(_, distance)| {
                let radius = self
                    .pixel_line
                    .map_or(HIT_RADIUS, |line| (line * 0.6).max(12.0));
                *distance <= radius * radius
            })
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(index, _)| (index, graph[index]))
    }

    fn margin(&self) -> f32 {
        self.pixel_line.map_or(MARGIN, |line| line + 12.0)
    }

    /// The per-axis scale that fits the raw grid to the viewport:
    /// the fit ratio, capped so a small tree does not stretch across
    /// the block and floored so a dense one stays clickable, times the
    /// wheel's [`Roadmap::zoom`]. Without a viewport the raw grid stands
    /// (the tests' fixed steps), zoom and all.
    fn scale_with(&self, placed: &[(f32, f32, usize)], zoom: f32) -> (f32, f32) {
        let Some((cols, rows)) = self.viewport else {
            return (zoom, zoom);
        };
        let (sx, sy) = if let Some(line) = self.pixel_line {
            let (span_x, span_y) = raw_span(placed);
            let margin = self.margin();
            let fit = |span: f32, size: f32, min: f32, max: f32| {
                if span > 0.0 {
                    ((size - 2.0 * margin) / span).clamp(min, max)
                } else {
                    max
                }
            };
            (
                fit(span_x, cols, 0.0, (line * 9.0 + 20.0) / COL_STEP),
                fit(span_y, rows, 0.0, (line * 3.0 + 24.0) / ROW_STEP),
            )
        } else {
            fit_scale(placed, cols, rows)
        };
        (sx * zoom, sy * zoom)
    }

    /// How far the pan may go: the scaled grid's overflow past the
    /// viewport, zero when everything fits (and the whole span without
    /// a viewport, for the raw grid).
    fn pan_limits(&self) -> (f32, f32) {
        self.pan_limits_with(self.camera.zoom())
    }

    /// How far the pan could go if the camera were at `zoom` — what a
    /// focus clamps its target against, since the zoom it is gliding to
    /// changes the room the pan has.
    fn pan_limits_with(&self, zoom: f32) -> (f32, f32) {
        let placed = &self.geometry().placed;
        let (sx, sy) = self.scale_with(placed, zoom);
        let (span_x, span_y) = raw_span(placed);
        match self.viewport {
            Some((cols, rows)) => (
                (span_x * sx - (cols - 2.0 * self.margin())).max(0.0),
                (span_y * sy - (rows - 2.0 * self.margin())).max(0.0),
            ),
            None => (span_x * zoom, span_y * zoom),
        }
    }
}

/// The raw grid's span: the furthest placed coordinates.
fn raw_span(placed: &[(f32, f32, usize)]) -> (f32, f32) {
    placed.iter().fold((0.0_f32, 0.0_f32), |(w, h), (x, y, _)| {
        (w.max(*x), h.max(*y))
    })
}

/// The per-axis scale that fits the raw grid to a `(cols, rows)` block:
/// the fit ratio, capped so a small tree does not stretch across the
/// block and floored so a dense one stays distinct.
fn fit_scale(placed: &[(f32, f32, usize)], cols: f32, rows: f32) -> (f32, f32) {
    let (span_x, span_y) = raw_span(placed);
    let usable_w = (cols - 2.0 * MARGIN).max(1.0);
    let usable_h = (rows - 2.0 * MARGIN).max(1.0);
    let fit = |usable: f32, span: f32| if span > 0.0 { usable / span } else { 1.0 };
    (
        fit(usable_w, span_x).clamp(MIN_COL_PITCH / COL_STEP, MAX_COL_PITCH / COL_STEP),
        fit(usable_h, span_y).clamp(MIN_ROW_PITCH / ROW_STEP, MAX_ROW_PITCH / ROW_STEP),
    )
}

/// The indices the mini-DAG keeps: breadth-first from the in-progress
/// chunk over dependency edges in both directions, nearest first — the
/// working set's head by list rank when nothing is in progress. The
/// result comes back in store order, so bands and stacking match the
/// big tree's.
fn mini_set(graph: &[&RoadmapChunk]) -> Vec<usize> {
    let mut kept = match graph.iter().position(|c| c.is_in_progress()) {
        Some(start) => {
            let mut seen = vec![false; graph.len()];
            let mut queue = std::collections::VecDeque::from([start]);
            seen[start] = true;
            let mut kept = Vec::new();
            while let Some(at) = queue.pop_front() {
                kept.push(at);
                if kept.len() == MINI_NODES {
                    break;
                }
                for next in neighbours(graph, at) {
                    if !seen[next] {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
            kept
        }
        None => {
            let mut ranked: Vec<usize> = (0..graph.len()).collect();
            ranked.sort_by(|&a, &b| {
                activity_rank(graph[a])
                    .cmp(&activity_rank(graph[b]))
                    .then(graph[a].priority.cmp(&graph[b].priority))
                    .then(graph[a].id.cmp(&graph[b].id))
            });
            ranked.truncate(MINI_NODES);
            ranked
        }
    };
    kept.sort_unstable();
    kept
}

/// Indices sharing a dependency edge with `at`, either direction.
fn neighbours(graph: &[&RoadmapChunk], at: usize) -> Vec<usize> {
    graph
        .iter()
        .enumerate()
        .filter(|(index, chunk)| {
            *index != at
                && (graph[at].deps.contains(&chunk.id) || chunk.deps.contains(&graph[at].id))
        })
        .map(|(index, _)| index)
        .collect()
}

/// Graph visibility keeps the working set first, independently of table sorting.
fn activity_rank(chunk: &RoadmapChunk) -> usize {
    match chunk.state {
        ChunkState::InProgress => 0,
        ChunkState::Pending => 1,
        ChunkState::Blocked => 2,
        ChunkState::Stuck | ChunkState::Backlog | ChunkState::Done | ChunkState::Obsoleted => 3,
    }
}

/// List order follows displayed readiness, with underway work first.
fn readiness_rank(readiness: &Readiness<'_>) -> usize {
    match readiness {
        Readiness::Inactive(ChunkStatus::InProgress) => 0,
        Readiness::Ready => 1,
        Readiness::Blocked(_) | Readiness::Inactive(ChunkStatus::Blocked) => 2,
        Readiness::Waiting(_) | Readiness::Inactive(ChunkStatus::Pending) => 3,
        Readiness::Inactive(ChunkStatus::Backlog) => 4,
        Readiness::Inactive(ChunkStatus::Done) => 5,
        Readiness::Inactive(ChunkStatus::Obsoleted) => 6,
    }
}

/// Place every chunk on the grid: `(x, y, region)` per chunk, parallel
/// to `graph`. The column is the chunk's dependency depth (longest path
/// over deps inside the set). Alternating barycentre sweeps order each
/// layer by its neighbours and align connected nodes while maintaining
/// a minimum row gap. Workstreams identify regions without fixing rows.
fn place(graph: &[&RoadmapChunk]) -> Vec<(f32, f32, usize)> {
    place_layout(graph, &graph_layout(graph))
}

fn place_layout(graph: &[&RoadmapChunk], layout: &DagLayout) -> Vec<(f32, f32, usize)> {
    let mut bands: Vec<(&str, u32)> = Vec::new();
    for chunk in graph {
        let group = chunk.group.as_deref().unwrap_or("");
        match bands.iter_mut().find(|(name, _)| *name == group) {
            Some((_, best)) => *best = (*best).min(chunk.priority),
            None => bands.push((group, chunk.priority)),
        }
    }
    bands.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(b.0)));
    layout
        .points
        .iter()
        .copied()
        .enumerate()
        .map(|(at, (x, y))| {
            let group = graph.get(at).and_then(|c| c.group.as_deref()).unwrap_or("");
            let region = bands
                .iter()
                .position(|(name, _)| *name == group)
                .unwrap_or(0);
            (x, y, region)
        })
        .collect()
}

fn graph_layout(graph: &[&RoadmapChunk]) -> DagLayout {
    let mut order: Vec<_> = (0..graph.len()).collect();
    order.sort_by(|&a, &b| {
        graph[a]
            .priority
            .cmp(&graph[b].priority)
            .then(graph[a].id.cmp(&graph[b].id))
    });
    let edges: Vec<_> = graph
        .iter()
        .enumerate()
        .flat_map(|(to, chunk)| {
            chunk.deps.iter().filter_map(move |dep| {
                graph
                    .iter()
                    .position(|c| &c.id == dep)
                    .map(|from| (from, to))
            })
        })
        .collect();
    // Longest-path depth is a chunk's earliest column; the compaction
    // pulls it to the column just before what first needs it, so an
    // early chunk with one late dependent does not trail an edge across
    // every column between.
    let ranks = DagLayout::compact_ranks(&depths(graph), &edges);
    let mut layout = DagLayout::new(&ranks, &edges, &order);
    for (x, y) in &mut layout.points {
        *x *= COL_STEP;
        *y *= ROW_STEP;
    }
    for path in &mut layout.paths {
        for (x, y) in &mut path.points {
            *x *= COL_STEP;
            *y *= ROW_STEP;
        }
    }
    layout
}

/// Each chunk's dependency depth: 0 with no in-set deps, else one past
/// the deepest dep. A cycle reads as depth 0 rather than recursing away.
fn depths(graph: &[&RoadmapChunk]) -> Vec<usize> {
    fn depth_of(
        index: usize,
        graph: &[&RoadmapChunk],
        memo: &mut [Option<usize>],
        on_stack: &mut Vec<usize>,
    ) -> usize {
        if let Some(depth) = memo[index] {
            return depth;
        }
        if on_stack.contains(&index) {
            return 0;
        }
        on_stack.push(index);
        let depth = graph[index]
            .deps
            .iter()
            .filter_map(|dep| graph.iter().position(|c| &c.id == dep))
            .map(|dep| depth_of(dep, graph, memo, on_stack) + 1)
            .max()
            .unwrap_or(0);
        on_stack.pop();
        memo[index] = Some(depth);
        depth
    }
    let mut memo = vec![None; graph.len()];
    let mut on_stack = Vec::new();
    (0..graph.len())
        .map(|index| depth_of(index, graph, &mut memo, &mut on_stack))
        .collect()
}

/// A recorded pending blocker overrides dependency readiness.
fn star_state(chunk: &RoadmapChunk, ready: bool) -> StarState {
    if chunk.state == ChunkState::Pending && chunk.blocker.is_some() {
        StarState::Blocked
    } else {
        StarState::of(chunk.state, ready)
    }
}

#[cfg(test)]
#[path = "roadmap.tests.rs"]
mod tests;
