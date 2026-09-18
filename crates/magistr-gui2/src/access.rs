use accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};
use magistr_cockpit::cockpit::Widget;
use magistr_cockpit::widget_id::WidgetId;

use crate::access_node::AccessNode;
use crate::element::{Element, Kind};
use crate::element_layout::ElementLayout;

/// The cockpit's accessibility tree, derived from the frame that was
/// just painted.
///
/// # Where every part of it comes from
///
/// Nothing here is a second description of the cockpit. The **ids** are
/// the cockpit's own [`WidgetId`]s — resolved from the composition's
/// element ids through the same seam the hit registry uses — so a node
/// is the same thing a click answers with and cannot drift between
/// frames the way an allocated id would. The **structure** is the
/// element tree's own nesting: a node's parent is its nearest ancestor
/// that resolves to a widget, which is exactly the containment a
/// pointer already resolves through. The **bounds** are that element's
/// placed pixel box. And the **names** are [`Widget::label`] where the
/// cockpit already has a word for the widget, and otherwise the text
/// the element actually holds — naming a row any other way would let a
/// screen reader announce something the screen does not show.
///
/// The tree is recorded as [`AccessNode`]s and converted to a
/// [`TreeUpdate`] on the way out, because AccessKit's `Node` cannot be
/// read back.
#[derive(Clone, Debug, Default)]
pub struct AccessTree {
    nodes: Vec<AccessNode>,
}

/// The walk's running state: the published nodes and the ancestor
/// stack of `(element depth, node index)` pairs.
struct Walk<'a> {
    layout: &'a ElementLayout,
    targets: &'a [Option<WidgetId>],
    scale: f64,
    nodes: Vec<AccessNode>,
    ancestors: Vec<(usize, usize)>,
    next: usize,
    clips: crate::frame_clip::FrameClips,
}

impl AccessTree {
    /// The window: the tree's root, and the one node that is not a
    /// widget.
    ///
    /// Id 0 is free for it only because every widget id is offset by
    /// one — `Widget::Header`'s raw id *is* 0, and a root sharing an id
    /// with the header would make the header its own parent.
    pub const WINDOW: NodeId = NodeId(0);

    /// The node id for a cockpit widget.
    #[must_use]
    pub const fn node_id(widget: WidgetId) -> NodeId {
        NodeId(widget.raw() as u64 + 1)
    }

    /// The tree for a painted frame: `tree` is the frame's own placed
    /// elements, `layout` is where layout put each one, `targets` is
    /// the per-element widget resolution the frame computed (the same
    /// vector the hit registration read), and `scale` is the window's
    /// scale factor, which turns physical pixels into the logical
    /// points a platform adapter expects (1.0 on a non-HiDPI display,
    /// and on the headless path).
    #[must_use]
    pub fn of(
        tree: &Element,
        layout: &ElementLayout,
        targets: &[Option<WidgetId>],
        scale: f64,
    ) -> Self {
        let mut walk = Walk {
            layout,
            targets,
            scale: if scale > 0.0 { scale } else { 1.0 },
            nodes: vec![AccessNode {
                id: Self::WINDOW,
                role: Role::Window,
                label: "magistr".to_owned(),
                value: None,
                bounds: None,
                children: Vec::new(),
            }],
            ancestors: Vec::new(),
            next: 0,
            clips: crate::frame_clip::FrameClips::of(tree, layout),
        };
        walk.descend(tree, 0);
        Self { nodes: walk.nodes }
    }

    /// The nodes, root first then in layout order.
    #[must_use]
    pub fn nodes(&self) -> &[AccessNode] {
        &self.nodes
    }

    /// The node published for `widget`, if it was on screen.
    #[must_use]
    pub fn find(&self, widget: Widget) -> Option<&AccessNode> {
        let id = Self::node_id(widget.id());
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The whole tree as one AccessKit update.
    ///
    /// Always a *full* tree: the adapter is built with handlers that
    /// return no initial tree, and AccessKit requires the first update
    /// after that to carry everything. Sending the whole tree every
    /// frame rather than a diff is the same trade the renderer makes
    /// with its draw list — the cockpit is tens of nodes, not the
    /// million-row list a diff exists for.
    #[must_use]
    pub fn update(&self, focus: Option<Widget>) -> TreeUpdate {
        let focus = focus
            .map(|widget| Self::node_id(widget.id()))
            .filter(|id| self.nodes.iter().any(|node| node.id == *id))
            .unwrap_or(Self::WINDOW);
        let nodes = self
            .nodes
            .iter()
            .map(|source| {
                let mut node = Node::new(source.role);
                node.set_label(source.label.clone());
                if let Some(value) = &source.value {
                    node.set_value(value.clone());
                }
                if let Some((x0, y0, x1, y1)) = source.bounds {
                    node.set_bounds(accesskit::Rect { x0, y0, x1, y1 });
                }
                node.set_children(source.children.clone());
                (source.id, node)
            })
            .collect();
        TreeUpdate {
            nodes,
            tree: Some(Tree {
                root: Self::WINDOW,
                toolkit_name: Some("magistr-gui2".to_owned()),
                toolkit_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            }),
            tree_id: TreeId::ROOT,
            focus,
        }
    }
}

impl Walk<'_> {
    /// Publish `element` and its subtree: an element whose id resolves
    /// to a widget becomes a node under its nearest published ancestor;
    /// everything else is transparent to the tree — an unnamed spacer
    /// is not a rung in it.
    fn descend(&mut self, element: &Element, depth: usize) {
        let index = self.next;
        self.next += 1;
        let widget = self.targets.get(index).copied().flatten();
        let [x, y, w, h] = self
            .clips
            .visible(index, self.layout.rect(index))
            .unwrap_or([0.0; 4]);
        while self.ancestors.last().is_some_and(|(d, _)| *d >= depth) {
            self.ancestors.pop();
        }
        if let Some(id) = widget.filter(|_| w > 0.0 && h > 0.0) {
            // A frame that placed one widget twice would otherwise
            // publish two nodes claiming to be the same thing, which
            // AccessKit reads as one node overwriting the other.
            if !self.nodes.iter().any(|n| n.id == AccessTree::node_id(id)) {
                let widget = Widget::from_id(id);
                let (role, word) = match widget {
                    Some(widget) => (role_of(widget), widget.label().to_owned()),
                    None => indexed_role(id),
                };
                let said = subtree_text(element);
                // A widget the cockpit has a word for is called that
                // word; one it does not is called what it says.
                let label = match (widget.is_some(), said.is_empty()) {
                    (true, _) | (false, true) => word,
                    (false, false) => said.clone(),
                };
                let (role, label) = match widget {
                    Some(Widget::Composer) if element.editor_cursor.is_some() => (
                        Role::MultilineTextInput,
                        element.label.clone().unwrap_or(label),
                    ),
                    Some(Widget::PromptAction) => (role, said.clone()),
                    _ => (role, label),
                };
                let value =
                    matches!(widget, Some(Widget::Prompt | Widget::Composer)).then_some(said);
                let point = |v: f32| f64::from(v) / self.scale;
                let node = AccessNode {
                    id: AccessTree::node_id(id),
                    role,
                    label,
                    value,
                    bounds: Some((point(x), point(y), point(x + w), point(y + h))),
                    children: Vec::new(),
                };
                let parent = self.ancestors.last().map_or(0, |(_, at)| *at);
                self.nodes[parent].children.push(node.id);
                self.ancestors.push((depth, self.nodes.len()));
                self.nodes.push(node);
            }
        }
        for child in &element.children {
            self.descend(child, depth + 1);
        }
    }
}

/// Every text run inside `element`'s subtree, joined — what the
/// element says on screen, which is what a node with no static word is
/// called and what the composer's value holds.
fn subtree_text(element: &Element) -> String {
    let mut said: Vec<&str> = Vec::new();
    element.each(&mut |_, node| {
        if let Kind::Text { content, .. } = &node.kind {
            let text = content.trim();
            if !text.is_empty() {
                said.push(text);
            }
        }
    });
    said.join(" ")
}

/// The role a named widget answers with.
///
/// The mapping lives here and not on [`Widget`] because the terminal has
/// no roles to answer to: adding one to the shared crate would be a
/// change to the cockpit for the benefit of exactly one of its two
/// renderers.
fn role_of(widget: Widget) -> Role {
    match widget {
        Widget::TabTurn | Widget::TabSession | Widget::TabRoadmap => Role::Tab,
        Widget::Allow
        | Widget::Always
        | Widget::Deny
        | Widget::Never
        | Widget::ShowImages
        | Widget::Accept
        | Widget::Reject
        | Widget::Clear
        | Widget::Checkpoint
        | Widget::RoadmapStart
        | Widget::RoadmapComplete
        | Widget::ShelfExport
        // The Report's verbs (view 09): the two header buttons and the
        // receipt rows' accent verbs.
        | Widget::ShelfShare
        | Widget::ReportCommit
        | Widget::ReportGates
        | Widget::ReportCost
        | Widget::ReportTrace
        | Widget::LaunchNext
        | Widget::LaunchLoop
        | Widget::LaunchGates
        | Widget::LaunchRoadmap
        | Widget::RailToggle
        | Widget::SettingsClose
        | Widget::SettingsOpenFile
        // The bad day's verbs are its cards' buttons (view 14).
        | Widget::FaultInstall
        | Widget::FaultDetails
        | Widget::FaultIncreaseLimit
        | Widget::FaultReceipts
        | Widget::FaultReconnect
        // The ledger head's verbs: key-hint pairs that act, so buttons.
        | Widget::LoopPause
        | Widget::LoopStep
        | Widget::LoopCheckpoint
        | Widget::LoopAbort
        // The meter's Raise on the rail's foot line.
        | Widget::BudgetRaise => Role::Button,
        Widget::TraceDetail | Widget::InspectorDetail | Widget::RequestBody => Role::ScrollView,
        Widget::Inspector | Widget::InspectorMetadata | Widget::InspectorActions
        | Widget::InspectorFields => Role::Group,
        Widget::InspectorTitle | Widget::InspectorStatus => Role::Label,
        Widget::RoadmapTable => Role::Table,
        Widget::RoadmapFilter | Widget::OverlayQuery => Role::TextInput,
        Widget::PanelColumns | Widget::PanelRoadmap | Widget::PanelReasoning
        | Widget::PanelGraph => Role::Splitter,
        Widget::ToolOpen | Widget::ToolUndo | Widget::RoadmapSortName
        | Widget::RoadmapSortStatus | Widget::RoadmapSortPriority
        | Widget::RoadmapSortWorkstream | Widget::PromptAction | Widget::LatestDecision
        | Widget::TracePrevious | Widget::TraceNext | Widget::PromptFiles
        | Widget::LinkedWork | Widget::Commands | Widget::RoadmapRequest
        | Widget::RoadmapReset | Widget::ReasoningToggle | Widget::RoadmapRequests
        | Widget::RoadmapDetails | Widget::RequestLatest => Role::Button,
        Widget::Prompt => Role::TextInput,
        // The dock's grip is a splitter like the divider; its screen is the terminal it
        // shows.
        Widget::RailDivider | Widget::TermDockGrip => Role::Splitter,
        Widget::TermDock => Role::Terminal,
        Widget::Research | Widget::Plan | Widget::Execute | Widget::Verify => Role::Label,
        // The three chrome slots and the two graph blocks are pictures:
        // a sparkline, a gauge and two graphs are drawings, and a
        // drawing with a name is exactly what `Image` means.
        Widget::HeaderBrand
        | Widget::HeaderSpark
        | Widget::RailGauge
        | Widget::RailGraph
        | Widget::TraceGraph
        | Widget::RoadmapGraph => Role::Image,
        Widget::Iteration | Widget::Elapsed | Widget::Chunk | Widget::Gates => Role::Label,
        Widget::Stage | Widget::Undock => Role::ScrollView,
        Widget::Header | Widget::Footer | Widget::Status => Role::Toolbar,
        Widget::Rail | Widget::Dock => Role::Group,
        // The altitude path: each crumb navigates, which is what `Link` means; the
        // composer is the window's prompt.
        Widget::CrumbUp
        | Widget::CrumbProjects
        | Widget::CrumbProject
        | Widget::CrumbSession
        | Widget::CrumbRecord
        // The identity band's chunk chip rises to the chunk's star: navigation, so a
        // link like the crumbs.
        | Widget::SessionChunk => Role::Link,
        Widget::Composer => Role::TextInput,
        // The Brief's three verbs are buttons; its `esc · dismiss` pair
        // navigates — it puts the overlay away rather than doing work,
        // the same reading HoverRole gives it.
        Widget::BriefAnswer | Widget::BriefReview | Widget::BriefReports => Role::Button,
        Widget::BriefDismiss => Role::Link,
        // The Galaxy's field is a drawing of every project; its one
        // verb is a button.
        Widget::GalaxyField => Role::Image,
        Widget::GalaxyNew => Role::Button,
        // Birth's controls: every one a button, the slot's wrapper included — `click to
        // browse` is a press.
        Widget::BirthSlot
        | Widget::BirthClone
        | Widget::BirthEmpty
        | Widget::BirthContinue
        | Widget::BirthAccept
        | Widget::BirthRedraw
        | Widget::BirthEdit
        | Widget::BirthClaim
        | Widget::BirthLater => Role::Button,
    }
}

/// One indexed id space: the decoder that recognises it, and the word a
/// row in it is called when the frame drew no text there.
type Space = (fn(WidgetId) -> bool, &'static str);

/// The role and fallback word for one of the indexed id spaces — the
/// rows and per-row buttons that are too dynamic to live in the
/// [`Widget`] enum.
///
/// Each space is recognised through its own public decoder, so this
/// cannot name a space that no longer exists; a space added to `Widget`
/// and not added here lands on the generic group rather than vanishing
/// from the tree.
fn indexed_role(id: WidgetId) -> (Role, String) {
    let buttons: [Space; 15] = [
        // A checkpoint row's revealed verbs, and a charter offer's Delegate.
        (|id| Widget::spine_restore_index(id).is_some(), "restore"),
        (|id| Widget::spine_fork_index(id).is_some(), "fork"),
        (|id| Widget::charter_offer_index(id).is_some(), "delegate"),
        (|id| Widget::edit_open_index(id).is_some(), "open"),
        (|id| Widget::edit_undo_index(id).is_some(), "undo"),
        (|id| Widget::change_open_index(id).is_some(), "open"),
        (|id| Widget::change_undo_index(id).is_some(), "undo"),
        (|id| Widget::change_approve_index(id).is_some(), "approve"),
        (|id| Widget::trace_pin_step(id).is_some(), "pin"),
        (|id| Widget::trace_link_step(id).is_some(), "revision"),
        (|id| Widget::trace_chunk_step(id).is_some(), "chunk"),
        (|id| Widget::proposal_accept_index(id).is_some(), "accept"),
        (|id| Widget::proposal_reject_index(id).is_some(), "reject"),
        (|id| Widget::signal_promote_index(id).is_some(), "promote"),
        (|id| Widget::signal_ignore_index(id).is_some(), "ignore"),
    ];
    if let Some((_, word)) = buttons.iter().find(|(is, _)| is(id)) {
        return (Role::Button, (*word).to_owned());
    }
    if Widget::signal_snooze_index(id).is_some() {
        return (Role::Button, "snooze".to_owned());
    }
    if Widget::signal_research_index(id).is_some() {
        return (Role::Button, "research".to_owned());
    }
    let rows: [Space; 8] = [
        // The rail's rows: a checkpoint (its click previews) and a
        // charter rule (read-only; a rule is written by an answer or
        // an accepted offer).
        (|id| Widget::spine_row_index(id).is_some(), "checkpoint"),
        (|id| Widget::charter_row_index(id).is_some(), "rule"),
        (|id| Widget::stage_row_index(id).is_some(), "row"),
        (|id| Widget::change_row_index(id).is_some(), "change"),
        (|id| Widget::trace_row_index(id).is_some(), "step"),
        (|id| Widget::roadmap_row_index(id).is_some(), "chunk"),
        (|id| Widget::launch_session_index(id).is_some(), "session"),
        (|id| Widget::completion_index(id).is_some(), "completion"),
    ];
    if let Some((_, word)) = rows.iter().find(|(is, _)| is(id)) {
        return (Role::ListItem, (*word).to_owned());
    }
    if Widget::footer_chip_index(id).is_some() {
        return (Role::Label, "key".to_owned());
    }
    // A cluster on the Galaxy navigates — a dive to the project's Sky —
    // which is what `Link` means, the same reading the crumbs get.
    if Widget::galaxy_cluster_index(id).is_some() {
        return (Role::Link, "project".to_owned());
    }
    (Role::GenericContainer, "widget".to_owned())
}

#[cfg(test)]
#[path = "access.tests.rs"]
mod tests;
