use accesskit::{Action, Node, NodeId, Role, Tree, TreeId, TreeUpdate};
use magistr_cockpit::cockpit::Widget;
use magistr_cockpit::widget_id::WidgetId;

use crate::access_node::AccessNode;
use crate::element::{Element, Kind};
use crate::element_layout::ElementLayout;

/// The cockpit's accessibility tree, derived from the frame that was
/// just painted.
///
/// Structure, names and clipped bounds come from the placed element tree.
/// Targets use the same widget resolution as pointer input. Published IDs are
/// reconciled against object identities so delayed requests cannot act on a
/// different row, permission, or command after the model changes.
#[derive(Clone, Debug, Default)]
pub struct AccessTree {
    nodes: Vec<AccessNode>,
}

/// The walk's running state: the published nodes and the ancestor
/// stack of `(element depth, node index)` pairs.
struct Walk<'a> {
    layout: &'a ElementLayout,
    targets: &'a [Option<WidgetId>],
    nodes: Vec<AccessNode>,
    ancestors: Vec<(usize, usize)>,
    next: usize,
    clips: crate::frame_clip::FrameClips,
}

impl AccessTree {
    /// Keep IDs only while the same control still names the same object.
    pub(crate) fn reconcile(
        &mut self,
        previous: &Self,
        next: &mut u64,
        model: &magistr_cockpit::cockpit::Model,
    ) {
        let mut ids = std::collections::BTreeMap::new();
        ids.insert(Self::WINDOW, Self::WINDOW);
        for node in &mut self.nodes {
            let Some(target) = node.target else { continue };
            node.identity.push_str(&identity(model, target));
            if target == Widget::OverlayQuery.id() {
                node.value = model
                    .palette()
                    .map(|p| p.query())
                    .or_else(|| model.search().map(|s| s.query()))
                    .or_else(|| model.settings().map(|s| s.query()))
                    .map(str::to_owned);
            }
            if let Some(index) = Widget::settings_switch_index(target) {
                node.role = Role::Switch;
                node.value = Some(
                    if match index {
                        0 => model.settings_data().heatmap,
                        1 => model.settings_data().grain,
                        2 => !model.theme().motion().is_full(),
                        _ => false,
                    } {
                        "On"
                    } else {
                        "Off"
                    }
                    .into(),
                );
            }
            let old_id = node.id;
            node.id = previous
                .nodes
                .iter()
                .find(|old| {
                    old.target == node.target
                        && old.identity == node.identity
                        && old.label == node.label
                        && old.role == node.role
                        && old.actions == node.actions
                        && old.disabled == node.disabled
                })
                .map(|old| old.id)
                .unwrap_or_else(|| {
                    let id = NodeId(*next);
                    *next = next
                        .checked_add(1)
                        .expect("accessibility node lifetime exhausted");
                    id
                });
            ids.insert(old_id, node.id);
        }
        for node in &mut self.nodes {
            for child in &mut node.children {
                *child = ids[child];
            }
        }
    }

    /// The window: the tree's root, and the one node that is not a
    /// widget.
    pub const WINDOW: NodeId = NodeId(0);

    /// Temporary construction ID; reconciliation replaces it before publication.
    const fn node_id(widget: WidgetId) -> NodeId {
        NodeId(widget.raw() as u64 + 1)
    }

    /// The tree for a painted frame: `tree` is the frame's own placed
    /// elements, `layout` is where layout put each one, `targets` is
    /// the per-element widget resolution the frame computed (the same
    /// vector the hit registration read). Bounds remain in physical pixels;
    /// the platform adapter performs the display-scale conversion.
    #[must_use]
    pub fn of(tree: &Element, layout: &ElementLayout, targets: &[Option<WidgetId>]) -> Self {
        let mut walk = Walk {
            layout,
            targets,
            nodes: vec![AccessNode {
                id: Self::WINDOW,
                target: None,
                identity: String::new(),
                role: Role::Window,
                label: "magistr".to_owned(),
                value: None,
                bounds: None,
                children: Vec::new(),
                actions: Vec::new(),
                disabled: false,
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
        self.nodes
            .iter()
            .find(|node| node.target == Some(widget.id()))
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
    pub fn update(&self, focus: Option<WidgetId>) -> TreeUpdate {
        let focus = focus
            .and_then(|target| self.nodes.iter().find(|node| node.target == Some(target)))
            .map(|node| node.id)
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
                if source.role == Role::Switch {
                    node.set_toggled(if source.value.as_deref() == Some("On") {
                        accesskit::Toggled::True
                    } else {
                        accesskit::Toggled::False
                    });
                }
                node.set_children(source.children.clone());
                for action in &source.actions {
                    node.add_action(*action);
                }
                if source.disabled {
                    node.set_disabled();
                }
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
                    Some(Widget::Stage) => (role, "Conversation".to_owned()),
                    _ => (role, label),
                };
                let label = element.label.clone().unwrap_or(label);
                let value = matches!(
                    widget,
                    Some(
                        Widget::Prompt
                            | Widget::Composer
                            | Widget::OverlayQuery
                            | Widget::Stage
                            | Widget::RequestBody
                            | Widget::Inspector
                            | Widget::TraceDetail
                    )
                )
                .then(|| match &element.kind {
                    Kind::Text { content, .. } => content.clone(),
                    _ => said,
                });
                let point = f64::from;
                let node = AccessNode {
                    id: AccessTree::node_id(id),
                    target: Some(id),
                    identity: format!("{:?}", element.id),
                    role,
                    label,
                    value,
                    bounds: Some((point(x), point(y), point(x + w), point(y + h))),
                    children: Vec::new(),
                    actions: actions_of(id, element),
                    disabled: element.disabled,
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

/// Stable object identities, not row positions, guard delayed platform requests.
fn identity(model: &magistr_cockpit::cockpit::Model, target: WidgetId) -> String {
    use magistr_cockpit::cockpit::{Palette, Search};
    if let Some(index) = Widget::roadmap_row_index(target) {
        return format!("{:?}", model.roadmap().rows().get(index).map(|c| &c.id));
    }
    if let Some(index) = Widget::sky_star_index(target) {
        return format!("{:?}", model.roadmap().graph_chunk(index).map(|c| &c.id));
    }
    if let Some(index) = Widget::palette_row_index(target) {
        return format!("{:?}", Palette::rows(model).get(index));
    }
    if let Some(index) = Widget::search_row_index(target) {
        return format!("{:?}", Search::results(model).rows.get(index));
    }
    if let Some(index) = Widget::trace_row_index(target) {
        return format!("{:?}", model.trace().steps().get(index).map(|s| s.number));
    }
    if let Some(index) = Widget::change_row_index(target)
        .or_else(|| Widget::change_open_index(target))
        .or_else(|| Widget::change_undo_index(target))
        .or_else(|| Widget::change_approve_index(target))
        .or_else(|| Widget::review_file_index(target))
    {
        return format!("{:?}", model.changes().entries().get(index));
    }
    if let Some(index) = Widget::signal_row_index(target)
        .or_else(|| Widget::signal_promote_index(target))
        .or_else(|| Widget::signal_research_index(target))
        .or_else(|| Widget::signal_snooze_index(target))
        .or_else(|| Widget::signal_ignore_index(target))
    {
        return format!("{:?}", model.roadmap().signals().get(index));
    }
    if let Some(index) =
        Widget::proposal_accept_index(target).or_else(|| Widget::proposal_reject_index(target))
    {
        return format!("{:?}", model.roadmap().proposals().get(index));
    }
    if let Some(index) = Widget::roadmap_dependency_index(target) {
        let chunk = match model.selected() {
            Some(magistr_cockpit::cockpit::Selected::Chunk(id)) => model.roadmap().chunk(id),
            _ => None,
        };
        return format!(
            "{:?}",
            chunk.and_then(|c| c.deps.iter().chain(&c.cross_refs).nth(index))
        );
    }
    if Widget::spine_row_index(target).is_some()
        || Widget::spine_restore_index(target).is_some()
        || Widget::spine_fork_index(target).is_some()
    {
        return format!("{:?}", model.rail().spine());
    }
    if Widget::session_row_index(target).is_some() {
        return format!(
            "{:?}",
            magistr_cockpit::cockpit::session_item::SessionItem::collect(
                model.trace(),
                model.changes()
            )
        );
    }
    if let Some(index) = Widget::completion_index(target) {
        return format!(
            "{:?}",
            model.completion().and_then(|c| c.items().get(index))
        );
    }
    if let Some(index) = Widget::galaxy_cluster_index(target) {
        return format!("{:?}", model.galaxy().projects().get(index));
    }
    if Widget::from_id(target).is_some_and(|w| w.answers().is_some()) {
        return format!("{:?}", model.dock().front());
    }
    if matches!(
        Widget::from_id(target),
        Some(Widget::Composer | Widget::Prompt | Widget::OverlayQuery)
    ) {
        return format!("{:?}", model.tab());
    }
    // Context-sensitive buttons and indexed links include the selected object's identity.
    format!("{:?}:{:?}", model.place(), model.selected())
}

fn actions_of(id: WidgetId, element: &Element) -> Vec<Action> {
    if element.disabled {
        return Vec::new();
    }
    let mut actions = Vec::new();
    if element.focusable {
        actions.push(Action::Focus);
    }
    let widget = Widget::from_id(id);
    if matches!(
        widget,
        Some(Widget::Composer | Widget::Prompt | Widget::OverlayQuery)
    ) {
        actions.push(Action::SetValue);
    } else if element.focusable {
        if matches!(
            widget,
            Some(
                Widget::PanelColumns
                    | Widget::PanelRoadmap
                    | Widget::PanelReasoning
                    | Widget::PanelGraph
            )
        ) {
            actions.extend([Action::Increment, Action::Decrement]);
        } else {
            actions.push(Action::Click);
        }
    }
    if matches!(
        widget,
        Some(
            Widget::Stage
                | Widget::Inspector
                | Widget::InspectorDetail
                | Widget::RequestBody
                | Widget::RoadmapTable
                | Widget::TraceDetail
        )
    ) {
        actions.extend([Action::ScrollUp, Action::ScrollDown]);
    }
    actions
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
        Widget::RoadmapFilter => Role::Button,
        Widget::OverlayQuery => Role::TextInput,
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
    (Role::Button, "Open".to_owned())
}

#[cfg(test)]
#[path = "access.tests.rs"]
mod tests;
