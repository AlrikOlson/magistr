use magistr_application::sessions::PermissionKind;
use magistr_cockpit::cockpit::{BriefBody, Copy, FrameClock, Model, Place, Widget};
use magistr_cockpit::widget_id::WidgetId;
use qs_gpu::color::Srgba;

use crate::budget_meter::BudgetMeter;
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
use crate::comp_roadmap::RoadmapComposition;
use crate::comp_search::SearchComposition;
use crate::comp_settings::SettingsComposition;
use crate::comp_shelf::ShelfComposition;
use crate::comp_signals::SignalsComposition;
use crate::comp_sky::SkyComposition;
use crate::comp_term_dock::TermDockComposition;
use crate::comp_trace::TraceComposition;
use crate::comp_workspace::WorkspaceComposition;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::ledger_verbs::LedgerVerbs;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::pill::DataPill;
use crate::spine_ledger::SpineLedger;
use crate::voice::Voice;

/// Map the shared model into the native element tree.
///
/// The composition chooses the active view, header, footer, navigation controls, and
/// overlays. Pixel layout and drawing remain the frame's responsibility.
pub struct RootComposition;

impl RootComposition {
    /// The rail's drag handle — one cell wide, because
    /// [`Rail::drag_width`](magistr_cockpit::cockpit::Rail) measures
    /// the drag in columns.
    pub const DIVIDER: ElementId = ElementId(320);
    /// The feed's viewport — the scroll wheel's target.
    pub const STAGE: ElementId = ElementId(321);

    /// Whether the feed anchors to its bottom edge: it follows the
    /// tail and holds more rows than the window shows — the
    /// transcript's own behaviour, the column packed to its end. A
    /// pinned (scrolled) window anchors at the top. The frame reads the
    /// same answer to carry the rows the right way while a scroll
    /// glides between them.
    #[must_use]
    pub(crate) fn feed_at_its_tail(model: &Model) -> bool {
        model.viewport().is_following() && model.turn().len() > model.stage_rows()
    }

    /// The air between the cap's live dot and the cap — the volume's `left:-20px` less
    /// the disc's own 9 px, on the working thing's row.
    const CAP_DOT_GAP: f32 = 11.0;

    /// The header band — the volume's `STAGE_HEADER_HEIGHT`. Crate
    /// visible: the frame's scissor tests measure the stage against it.
    pub(crate) const HEADER_HEIGHT: f32 = 51.75;
    /// The room the window's own lights take before the path
    /// (`STAGE_HEADER_PAD_LEFT` 92, less the 18 the header composition
    /// already pads).
    const LIGHTS_ROOM: f32 = 74.0;
    /// The air between the clock chip and the window's edge
    /// (`STAGE_HEADER_GAP`).
    const HEADER_GAP: f32 = 14.0;
    /// The turn column's margins — `STAGE_PAD_TOP` / `STAGE_PAD_SIDE`.
    const STAGE_PAD_TOP: f32 = 20.0;
    const STAGE_PAD_SIDE: f32 = 30.0;
    /// The air under the session cap (`STAGE_CAP_GAP`).
    const STAGE_CAP_GAP: f32 = 8.0;
    /// The air under the composer (`COMPOSER_PAD_BOTTOM`).
    const COMPOSER_PAD_BOTTOM: f32 = 18.0;
    /// The volume's measure on the turn column (`TURN_MAX_WIDTH`).
    const TURN_MAX_WIDTH: f32 = 600.0;
    /// The scrim an overlay floats on: the ground at this alpha, so the session dims
    /// and stays legible — the design forbids blanking it.
    const SCRIM: f32 = 0.55;
    /// The deeper scrim under a search (view 13's `opacity:.15` on the
    /// room): a decision screen, and the chrome has stood down with it.
    const SEARCH_SCRIM: f32 = 0.85;

    /// The window for `model`, in `medium`'s lexicon. `cell` is the
    /// surface's cell advance and row height in pixels — the feed's
    /// image footprints and the rail's drag width are measured in the
    /// model's cells, and only the renderer knows their pixel size.
    /// `viewport` is the window's logical size — the Galaxy lays its
    /// clusters on the field the chrome leaves, and only the frame
    /// knows how big that is. `clock` is the session clock the header
    /// chip prints.
    #[must_use]
    pub fn el(
        model: &Model,
        medium: Medium,
        cell: (f32, f32),
        viewport: (f32, f32),
        clock: &FrameClock,
    ) -> Element {
        let lex = Lexicon::of(medium);
        // Settings (view 10) is a real surface, not a float: it takes
        // the whole window — its own header, sidebar and pane — and the
        // room behind it is not drawn at all, the way the volume shows
        // it. Only the window's edge is kept.
        if let Some(surface) = SettingsComposition::el(model, medium) {
            return Element::surface()
                .fill(lex.base)
                .child(surface)
                .child(edge(&lex));
        }
        // Search (view 13) is a decision screen: the header, the rail
        // and the foot stand down entirely, the room itself stays at
        // 15% under the scrim — never blanked — and the float sits on
        // it. The Galaxy's room is its field; every other place's is
        // the stage.
        if let Some(float) = SearchComposition::el(model, medium) {
            let room = if model.place() == Some(Place::Galaxy) {
                GalaxyComposition::el(model, medium, viewport)
            } else {
                stage(model, medium, cell, &lex)
            };
            let scrim = Srgba::new(lex.base.r, lex.base.g, lex.base.b, Self::SEARCH_SCRIM);
            return Element::surface()
                .fill(lex.base)
                .child(Element::surface().grow(1.0).min_height(0.0).child(room))
                .child(
                    Element::surface()
                        .pin_left(0.0)
                        .pin_top(0.0)
                        .pin_right(0.0)
                        .pin_bottom(0.0)
                        .fill(scrim),
                )
                .child(edge(&lex))
                .child(float);
        }
        // The review replaces the session: the thing to look at is the change, under
        // the same path.
        if model.review().is_some() {
            let review = MomentComposition::el(model, medium).expect("Model::review is Some");
            return Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(review.grow(1.0))
                .child(edge(&lex));
        }
        // Rest: the chrome has exhaled and the window is the constellation and its
        // murmur — nothing else stands.
        let mut root = if RoadmapComposition::active(model) {
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(RoadmapComposition::el(
                    model,
                    medium,
                    cell,
                    (
                        viewport.0,
                        (viewport.1 - header_height(model) - FootComposition::HEIGHT).max(0.0),
                    ),
                ))
                .child(FootComposition::el(model, medium))
        } else if model.sky_at_rest() {
            Element::surface()
                .fill(lex.base)
                .child(SkyComposition::el(model, medium))
        } else if model.place() == Some(Place::Birth) && !BirthComposition::has_header(model) {
            // Birth's ground (F1): the card floats over the room with
            // no header and no foot — the Galaxy waits underneath.
            Element::surface()
                .fill(lex.base)
                .child(BirthComposition::el(model, medium, viewport, cell))
        } else if model.place() == Some(Place::Birth) {
            // Birth's later moments (F2–F5): the record band with the
            // moment's own chips, the moment's body under it — no
            // supervision rail, no foot; a project not yet born has no
            // session to supervise.
            let room = (viewport.0, (viewport.1 - header_height(model)).max(0.0));
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(BirthComposition::el(model, medium, room, cell))
        } else if model.place() == Some(Place::Galaxy) {
            // Altitude 0: the header with the Galaxy's chips, the field full-bleed — no
            // supervision rail, there is no one session to supervise from the top of
            // the world — and the Galaxy's own foot.
            let field = (
                viewport.0,
                (viewport.1 - GalaxyComposition::HEADER_HEIGHT - GalaxyComposition::FOOT_HEIGHT)
                    .max(0.0),
            );
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(GalaxyComposition::el(model, medium, field))
                .child(GalaxyComposition::foot(model, medium))
        } else if model.place() == Some(Place::Trace) {
            // Altitude 3, the Trace (view 11): the header with the
            // trace's chips, the step feed and the BRANCHES rail — no
            // supervision rail and no foot, which the volume draws
            // neither of on a record.
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(TraceComposition::el(model, medium))
        } else if model.place() == Some(Place::Report) {
            // Altitude 3, the Report (view 09): the header with the two
            // verbs, the chapter beside the deliverables rail — no
            // supervision rail and no foot; an archive page holds
            // still.
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(ReportComposition::el(model, medium))
        } else if model.bad_day().is_some() {
            // A bad day (view 14): the header with the blocking count,
            // the room — the faded constellation at the Sky, the stage
            // elsewhere — and nothing else; the cards and the murmur
            // foot float over it below.
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(
                    Element::surface()
                        .grow(1.0)
                        .min_height(0.0)
                        .child(stage(model, medium, cell, &lex)),
                )
        } else if model.place() == Some(Place::Signals) {
            // Altitude 1's second room, the Signals inbox (view 12):
            // the header with the open count, the cards — no rail and
            // no foot; there is no session to supervise from the
            // project's inbox.
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(SignalsComposition::el(model, medium, clock))
        } else if WorkspaceComposition::active(model) {
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(WorkspaceComposition::el(
                    model,
                    medium,
                    cell,
                    (
                        viewport.0,
                        (viewport.1 - header_height(model) - FootComposition::HEIGHT).max(0.0),
                    ),
                ))
                .child(FootComposition::el(model, medium))
        } else {
            let deciding = model.moment().is_some();
            // min-height 0 all the way down the feed chain: flex's automatic minimum
            // would otherwise let a tall turn inflate body → root and push the composer
            // and foot below the window.
            let mut body = Element::row()
                .grow(1.0)
                .min_height(0.0)
                .child(stage(model, medium, cell, &lex));
            // The supervision rail is altitude 2's; behind a Moment it
            // is gone rather than collapsed — there is no session to
            // supervise while the session is waiting on you — and the
            // Sky (views 03/04) never draws it: the constellation is
            // the field full-bleed, and a session is supervised from
            // its own altitude.
            let sky = model.place() == Some(Place::Sky);
            if !deciding && !sky && !model.rail().is_collapsed() {
                // The grab is one cell wide, flush against the rail's
                // own seam (the volume's `.hair-l` is inside the
                // rail's 248, and the rail draws it); the column
                // itself is invisible.
                body = body
                    .child(Element::row().width(cell.0).stretch().id(Self::DIVIDER))
                    .child(RailComposition::el(model, medium, cell.0));
            }
            Element::surface()
                .fill(lex.base)
                .child(header(model, medium, clock, &lex))
                .child(body)
                .child(FootComposition::el(model, medium))
        };
        // The window's edge — the volume's `.app7` inset hairline on
        // every view — drawn over the bands and under the overlays.
        root = root.child(edge(&lex));
        // The bad day's cards float over the room with no scrim — the
        // room is already the faded constellation — pinned under the
        // header band, and they are never dismissed.
        let band = header_height(model);
        if let Some(float) = BadDayComposition::el(model, medium, (viewport.1 - band).max(0.0)) {
            root = root.child(
                Element::surface()
                    .pin_left(0.0)
                    .pin_top(band)
                    .pin_right(0.0)
                    .pin_bottom(0.0)
                    .child(float),
            );
        }
        // The Moment: one decision on a scrim over the dimmed session. The Brief floats
        // the same way, and never at the same time — `Model::brief` answers `None`
        // while anything is asking, because the card comes to you.
        if let Some(card) = MomentComposition::el(model, medium) {
            root = root.child(overlay(card, &lex));
        }
        if let Some(card) = BriefComposition::el(model, medium) {
            root = root.child(overlay(card, &lex));
        }
        // The ⌘K palette floats over everything with no scrim — a jump
        // leaves the room fully lit; the composition carries its own
        // pin and seat.
        if let Some(palette) = PaletteComposition::el(model, medium) {
            root = root.child(palette);
        }
        if let Some(completion) = ComposerComposition::completion(model, medium, viewport) {
            root = root.child(completion);
        }
        root
    }

    /// The cockpit widget behind an element id, for the ids the
    /// compositions mint — the one seam between the element tree's
    /// interaction identities and the shared `update`'s vocabulary.
    /// `None` for an id that is visual state only (hover, focus) and
    /// for anything unminted.
    #[must_use]
    pub fn target_of(model: &Model, id: ElementId) -> Option<WidgetId> {
        if id == RoadmapComposition::LIST {
            return Some(Widget::RoadmapTable.id());
        }
        if id == RoadmapComposition::REQUESTS {
            return Some(Widget::RequestBody.id());
        }
        if (RoadmapComposition::BODY.0..=RoadmapComposition::INPUT.0).contains(&id.0) {
            return None;
        }
        if id == WorkspaceComposition::DETAILS {
            return Some(Widget::Inspector.id());
        }
        if (WorkspaceComposition::BODY.0..=WorkspaceComposition::RIGHT.0).contains(&id.0) {
            return None;
        }
        if let Some(widget) = WorkspaceComposition::target(id) {
            return Some(widget);
        }
        if let Some(widget) = birth_target(id) {
            return Some(widget);
        }
        match id {
            PaletteComposition::QUERY | SearchComposition::FIELD | SettingsComposition::QUERY => {
                Some(Widget::OverlayQuery.id())
            }
            HeaderComposition::PROJECTS => Some(Widget::CrumbProjects.id()),
            HeaderComposition::PROJECT => Some(Widget::CrumbProject.id()),
            HeaderComposition::SESSION => Some(Widget::CrumbSession.id()),
            ComposerComposition::PILL => Some(Widget::Composer.id()),
            SkyComposition::FIELD => Some(Widget::RoadmapGraph.id()),
            GalaxyComposition::FIELD => Some(Widget::GalaxyField.id()),
            GalaxyComposition::NEW => Some(Widget::GalaxyNew.id()),
            SkyComposition::START => Some(Widget::RoadmapStart.id()),
            SkyComposition::COMPLETE => Some(Widget::RoadmapComplete.id()),
            ShelfComposition::EXPORT => Some(Widget::ShelfExport.id()),
            ReportComposition::SHARE => Some(Widget::ShelfShare.id()),
            ReportComposition::COMMIT => Some(Widget::ReportCommit.id()),
            ReportComposition::GATES => Some(Widget::ReportGates.id()),
            ReportComposition::COST => Some(Widget::ReportCost.id()),
            ReportComposition::TRACE => Some(Widget::ReportTrace.id()),
            BriefComposition::DISMISS => Some(Widget::BriefDismiss.id()),
            Self::DIVIDER => Some(Widget::RailDivider.id()),
            Self::STAGE => Some(Widget::Stage.id()),
            TraceComposition::GRAPH => Some(Widget::TraceGraph.id()),
            SettingsComposition::CLOSE => Some(Widget::SettingsClose.id()),
            SettingsComposition::OPEN => Some(Widget::SettingsOpenFile.id()),
            // The ledger head's verbs: the rail's key-hint pairs are the cockpit's own
            // loop widgets.
            LedgerVerbs::PAUSE => Some(Widget::LoopPause.id()),
            LedgerVerbs::STEP => Some(Widget::LoopStep.id()),
            LedgerVerbs::CHECKPOINT => Some(Widget::LoopCheckpoint.id()),
            LedgerVerbs::ABORT => Some(Widget::LoopAbort.id()),
            // The meter's Raise on the rail's foot line.
            BudgetMeter::RAISE => Some(Widget::BudgetRaise.id()),
            // The identity band's chunk chip.
            BandComposition::CHUNK => Some(Widget::SessionChunk.id()),
            // The terminal dock's grip and screen.
            TermDockComposition::GRIP => Some(Widget::TermDockGrip.id()),
            TermDockComposition::PANE => Some(Widget::TermDock.id()),
            // The spine's rows and their revealed verbs, by spine index, and the
            // charter's rows.
            ElementId(n) if (SpineLedger::ROWS..SpineLedger::RESTORE).contains(&n) => Some(
                Widget::spine_row(usize::try_from(n - SpineLedger::ROWS).ok()?),
            ),
            ElementId(n) if (SpineLedger::RESTORE..SpineLedger::FORK).contains(&n) => Some(
                Widget::spine_restore(usize::try_from(n - SpineLedger::RESTORE).ok()?),
            ),
            ElementId(n) if (SpineLedger::FORK..SpineLedger::FORK_END).contains(&n) => Some(
                Widget::spine_fork(usize::try_from(n - SpineLedger::FORK).ok()?),
            ),
            ElementId(n)
                if (RailComposition::CHARTER..RailComposition::CHARTER_END).contains(&n) =>
            {
                Some(Widget::charter_row(
                    usize::try_from(n - RailComposition::CHARTER).ok()?,
                ))
            }
            // A charter offer's Delegate.
            ElementId(n) if (RailComposition::OFFER..RailComposition::OFFER_END).contains(&n) => {
                Some(Widget::charter_offer(
                    usize::try_from(n - RailComposition::OFFER).ok()?,
                ))
            }
            // The Trace view's spaces — a card by feed index, its verbs
            // by step number — ahead of the feed's open-ended button
            // range, which they sit inside numerically.
            ElementId(n) if (TraceComposition::CARDS..TraceComposition::PINS).contains(&n) => Some(
                Widget::trace_row(usize::try_from(n - TraceComposition::CARDS).ok()?),
            ),
            ElementId(n) if (TraceComposition::PINS..TraceComposition::RECEIPTS).contains(&n) => {
                Some(Widget::trace_pin(
                    u32::try_from(n - TraceComposition::PINS).ok()?,
                ))
            }
            ElementId(n)
                if (TraceComposition::RECEIPTS..TraceComposition::RECEIPTS_END).contains(&n) =>
            {
                Some(Widget::trace_chunk(
                    u32::try_from(n - TraceComposition::RECEIPTS).ok()?,
                ))
            }
            // The Signals view's verbs, by the signal's index among the
            // open signals — the list the cockpit's signal buttons
            // index — in the three spaces after the Trace's.
            ElementId(n)
                if (SignalsComposition::PROMOTE..SignalsComposition::RESEARCH).contains(&n) =>
            {
                Some(Widget::signal_promote(
                    usize::try_from(n - SignalsComposition::PROMOTE).ok()?,
                ))
            }
            ElementId(n)
                if (SignalsComposition::RESEARCH..SignalsComposition::SNOOZE).contains(&n) =>
            {
                Some(Widget::signal_research(
                    usize::try_from(n - SignalsComposition::RESEARCH).ok()?,
                ))
            }
            ElementId(n)
                if (SignalsComposition::SNOOZE..SignalsComposition::SNOOZE_END).contains(&n) =>
            {
                Some(Widget::signal_snooze(
                    usize::try_from(n - SignalsComposition::SNOOZE).ok()?,
                ))
            }
            // The bad day's verbs, across every card in deal order.
            ElementId(n)
                if (BadDayComposition::VERBS..BadDayComposition::VERBS_END).contains(&n) =>
            {
                let day = model.bad_day()?;
                Some(
                    day.verb(usize::try_from(n - BadDayComposition::VERBS).ok()?)?
                        .widget()
                        .id(),
                )
            }
            // The search float's rows and scope chips, by index.
            ElementId(n) if (SearchComposition::ROWS..SearchComposition::ROWS_END).contains(&n) => {
                Some(Widget::search_row(
                    usize::try_from(n - SearchComposition::ROWS).ok()?,
                ))
            }
            ElementId(n)
                if (SearchComposition::SCOPES..SearchComposition::SCOPES_END).contains(&n) =>
            {
                Some(Widget::search_scope(
                    usize::try_from(n - SearchComposition::SCOPES).ok()?,
                ))
            }
            // The settings surface's spaces: the sidebar items, the
            // theme cards, the switches and the density segments.
            ElementId(n)
                if (SettingsComposition::NAV..SettingsComposition::DENSITY_END).contains(&n) =>
            {
                settings_target(n)
            }
            // A feed card's head, minted by absolute row: the click
            // (or ↵) the cockpit's own stage-row toggle answers, so
            // the card that opens on the window is the card that
            // opens on a terminal. The image space above it is
            // reservations, not controls.
            ElementId(n) if (FeedComposition::HEADS..FeedComposition::IMAGES).contains(&n) => {
                let visible = usize::try_from(n - FeedComposition::HEADS)
                    .ok()?
                    .checked_sub(model.top_row())
                    .filter(|row| *row < model.visible_rows().len())?;
                Some(Widget::stage_row(visible))
            }
            // An edit card's [Open]/[Undo] pair, minted by absolute
            // row; the cockpit's ids count visible rows.
            ElementId(n) if (FeedComposition::BUTTONS..FeedComposition::HEADS).contains(&n) => {
                let at = (n - FeedComposition::BUTTONS) / 2;
                let visible = usize::try_from(at)
                    .ok()?
                    .checked_sub(model.top_row())
                    .filter(|row| *row < model.visible_rows().len())?;
                Some(if (n - FeedComposition::BUTTONS).is_multiple_of(2) {
                    Widget::edit_open(visible)
                } else {
                    Widget::edit_undo(visible)
                })
            }
            // A Moment's answer verbs, slotted in PermissionKind's own
            // order.
            ElementId(n)
                if (MomentComposition::ANSWERS
                    ..MomentComposition::ANSWERS + PermissionKind::ALL.len() as u64)
                    .contains(&n) =>
            {
                let kind =
                    PermissionKind::ALL[usize::try_from(n - MomentComposition::ANSWERS).ok()?];
                Some(
                    match kind {
                        PermissionKind::AllowOnce => Widget::Allow,
                        PermissionKind::AllowAlways => Widget::Always,
                        PermissionKind::RejectOnce => Widget::Deny,
                        PermissionKind::RejectAlways => Widget::Never,
                    }
                    .id(),
                )
            }
            // A ⌘K palette row, by its index in the result list.
            ElementId(n)
                if (PaletteComposition::ROWS..PaletteComposition::ROWS_END).contains(&n) =>
            {
                Some(Widget::palette_row(
                    usize::try_from(n - PaletteComposition::ROWS).ok()?,
                ))
            }
            // A Brief card's verb: the card carries its own widget.
            ElementId(n) if n >= BriefComposition::VERBS => {
                let index = usize::try_from(n - BriefComposition::VERBS).ok()?;
                let brief = model.brief()?;
                let card = brief.cards().get(index)?;
                match card.body() {
                    BriefBody::Act { widget, .. } => Some(widget.id()),
                    BriefBody::Measure { .. } => None,
                }
            }
            _ => None,
        }
    }
}

/// Birth's controls: the nine verbs by name, then the interview chips and the proposal
/// rows by index — ahead of the feed's open-ended button range, which the two spaces
/// sit inside numerically. `None` for an id that is not Birth's.
fn birth_target(id: ElementId) -> Option<WidgetId> {
    let named = match id {
        BirthComposition::SLOT => Some(Widget::BirthSlot),
        BirthComposition::CLONE => Some(Widget::BirthClone),
        BirthComposition::EMPTY => Some(Widget::BirthEmpty),
        BirthComposition::CONTINUE => Some(Widget::BirthContinue),
        BirthComposition::ACCEPT => Some(Widget::BirthAccept),
        BirthComposition::REDRAW => Some(Widget::BirthRedraw),
        BirthComposition::EDIT => Some(Widget::BirthEdit),
        BirthComposition::CLAIM => Some(Widget::BirthClaim),
        BirthComposition::LATER => Some(Widget::BirthLater),
        _ => None,
    };
    if let Some(widget) = named {
        return Some(widget.id());
    }
    let ElementId(n) = id;
    if (BirthComposition::CHIPS..BirthComposition::ROWS).contains(&n) {
        return Some(Widget::birth_chip(
            usize::try_from(n - BirthComposition::CHIPS).ok()?,
        ));
    }
    if (BirthComposition::ROWS..BirthComposition::ROWS_END).contains(&n) {
        return Some(Widget::birth_row(
            usize::try_from(n - BirthComposition::ROWS).ok()?,
        ));
    }
    None
}

/// The settings surface's indexed spaces, by the id's range: the
/// sidebar item, the theme card, the switch or the density segment at
/// that index. `None` for a number between the spaces.
fn settings_target(n: u64) -> Option<WidgetId> {
    let index = |base: u64| usize::try_from(n - base).ok();
    if (SettingsComposition::NAV..SettingsComposition::NAV_END).contains(&n) {
        Some(Widget::settings_nav(index(SettingsComposition::NAV)?))
    } else if (SettingsComposition::THEME..SettingsComposition::THEME_END).contains(&n) {
        Some(Widget::settings_theme(index(SettingsComposition::THEME)?))
    } else if (SettingsComposition::SWITCH..SettingsComposition::SWITCH_END).contains(&n) {
        Some(Widget::settings_switch(index(SettingsComposition::SWITCH)?))
    } else if (SettingsComposition::DENSITY..SettingsComposition::DENSITY_END).contains(&n) {
        Some(Widget::settings_density(index(
            SettingsComposition::DENSITY,
        )?))
    } else {
        None
    }
}

/// The header band: the room the lights take, the altitude path, and
/// at the right edge the live session clock — or, at the Galaxy, the
/// running / action-required / ⌘K chips (view 01: there is no one
/// session to clock at the top of the world).
fn header(model: &Model, medium: Medium, clock: &FrameClock, lex: &Lexicon) -> Element {
    let secs = clock.elapsed().as_secs();
    // The bad day's count leads (view 14): the failure is the header's
    // news wherever the camera is.
    let right = match model.place() {
        _ if model.bad_day().is_some() => {
            BadDayComposition::chips(model, medium).expect("Model::bad_day is Some")
        }
        Some(Place::Galaxy) => GalaxyComposition::chips(model, medium),
        Some(Place::Trace) => TraceComposition::chips(model, medium),
        Some(Place::Signals) => SignalsComposition::chips(model, medium),
        Some(Place::Report) => ReportComposition::chips(medium),
        Some(Place::Birth) => BirthComposition::chips(model, medium),
        _ => DataPill::live(&Copy::clock(secs), lex),
    };
    // The Galaxy's band is its own measure (view 01): a pixel shorter,
    // and padded to the window's edge by 20 rather than the clock's 14.
    // The Trace (view 11), the Signals inbox (view 12) and the Report
    // (view 09, taller by its buttons) are drawn on the same band.
    let height = header_height(model);
    // A bad day (view 14) wears the record band too.
    let (gap, lift) = match model.place() {
        Some(Place::Galaxy | Place::Trace | Place::Signals | Place::Report | Place::Birth) => (
            GalaxyComposition::HEADER_GAP,
            GalaxyComposition::HEADER_LIFT,
        ),
        _ if model.bad_day().is_some() => (
            GalaxyComposition::HEADER_GAP,
            GalaxyComposition::HEADER_LIFT,
        ),
        _ => (RootComposition::HEADER_GAP, 0.0),
    };
    // The band closes on its own hairline (the volume's border-bottom,
    // inside `STAGE_HEADER_HEIGHT` the way border-box counts it).
    Element::surface()
        .child(air(lift))
        .child(
            Element::row()
                .height(height - 1.0 - lift)
                .center()
                .child(Element::row().width(RootComposition::LIGHTS_ROOM))
                .child(HeaderComposition::el(model, medium))
                .child(Element::row().grow(1.0))
                .child(right)
                .child(Element::row().width(gap)),
        )
        .child(Element::surface().height(1.0).fill(lex.grounded(lex.hairc)))
}

/// The header band's height where the camera is — the Galaxy's, the
/// Trace's and the Signals' own measures, the session's everywhere
/// else. What the bad day's float pins under.
fn header_height(model: &Model) -> f32 {
    match model.place() {
        Some(Place::Galaxy) => GalaxyComposition::HEADER_HEIGHT,
        Some(Place::Trace) => TraceComposition::HEADER_HEIGHT,
        Some(Place::Signals) => SignalsComposition::HEADER_HEIGHT,
        Some(Place::Report) => ReportComposition::HEADER_HEIGHT,
        Some(Place::Birth) => BirthComposition::header_height(model),
        // The bad day's band is the record band's (view 14's
        // `padding:15px 20px 12px 92px` is view 12's).
        _ if model.bad_day().is_some() => SignalsComposition::HEADER_HEIGHT,
        _ => RootComposition::HEADER_HEIGHT,
    }
}

/// The stage, routed by where the camera is: the sky at the Sky (the Galaxy never
/// reaches here — the root lays its field itself) and view 05's turn column otherwise:
/// the caption, the feed at the volume's measure, and the composer at its foot at the
/// Session; the feed alone at a record. Behind a Moment the column's own chrome stands
/// down (third altitude), exactly as the cell view decided it.
fn stage(model: &Model, medium: Medium, cell: (f32, f32), lex: &Lexicon) -> Element {
    // The frame declares every window native before mapping the root.
    let place = model.place().expect("the window is a native surface");
    // The Sky — also under a Brief or a question, when `Model::is_sky`
    // (which those override) is false but the room behind the overlay
    // is still the constellation.
    if place == Place::Sky {
        return SkyComposition::el(model, medium);
    }
    let session = place == Place::Session
        && !model.is_sheet_open()
        && model.moment().is_none()
        && model.search().is_none();
    let mut column = Element::surface().grow(1.0).min_height(0.0).align_start();
    if model.search().is_some() {
        // The dimmed room under a search keeps the stage's own top
        // margin (view 13's `padding:20px 30px`) with no cap over it.
        column = column.child(air(RootComposition::STAGE_PAD_TOP));
    }
    if session {
        // The identity band is the cap: one row of essentials, composed from the model.
        // The band's live dot: the volume's 9 px mauve disc beside the working thing,
        // pinned in the margin to the band's left so the band itself stays where the
        // capture puts it, coming up out of the ground with the wave and gone at the
        // foot — so a settled still is the volume's.
        let mut row = Element::row()
            .center()
            .child(BandComposition::el(model, medium));
        if let Some(breath) = crate::breath::Breath::of(model) {
            let line = Voice::CAP.px * Voice::CAP.leading();
            row = row.child(
                breath
                    .dot(medium, lex)
                    .pin_left(-(crate::breath::Breath::DOT_PX + RootComposition::CAP_DOT_GAP))
                    .pin_top(((line - crate::breath::Breath::DOT_PX) / 2.0).max(0.0)),
            );
        }
        column = column
            .child(air(RootComposition::STAGE_PAD_TOP))
            .child(row)
            .child(air(RootComposition::STAGE_CAP_GAP));
    }
    // While the feed follows the tail and holds more rows than the
    // window shows, the column anchors to its bottom edge — the
    // transcript's own behaviour; a pinned (scrolled) window anchors
    // at the top.
    // CSS's scrollable-middle pattern, whole: basis 0 (`height(0.0)` +
    // grow) so the cap, composer and their air never enter flex shrink,
    // and min-height 0 so the automatic minimum cannot hold the feed at
    // its content height — the box is exactly the room the chrome
    // leaves, whatever the turn holds.
    let mut feed = FeedComposition::el(model, medium, cell)
        .grow(1.0)
        .height(0.0)
        .min_height(0.0)
        .stretch()
        .max_width(RootComposition::TURN_MAX_WIDTH)
        .id(RootComposition::STAGE);
    if session && RootComposition::feed_at_its_tail(model) {
        feed = feed.justify_end();
    }
    column = column.child(feed);
    if session {
        // The terminal dock rises from the composer's edge over the feed: the feed
        // above it gives up the rows it takes, the composer under it stays where it
        // stands.
        if let Some(dock) = TermDockComposition::el(model, medium) {
            column = column.child(dock);
        }
        // The pill spans the column (the capture's composer runs the
        // stage's width, not the verb's).
        column = column
            .child(ComposerComposition::el(model, medium).stretch())
            .child(air(RootComposition::COMPOSER_PAD_BOTTOM));
    }
    Element::row()
        .grow(1.0)
        .child(Element::row().width(RootComposition::STAGE_PAD_SIDE))
        .child(column)
        .child(Element::row().width(RootComposition::STAGE_PAD_SIDE))
}

/// A card floated on the scrim: the ground at the overlay alpha over
/// the whole window, the card centred on it at full ink.
fn overlay(card: Element, lex: &Lexicon) -> Element {
    let scrim = Srgba::new(lex.base.r, lex.base.g, lex.base.b, RootComposition::SCRIM);
    Element::surface()
        .pin_left(0.0)
        .pin_top(0.0)
        .pin_right(0.0)
        .pin_bottom(0.0)
        .fill(scrim)
        .center()
        .justify_center()
        .child(card)
}

/// The window's edge: a one-pixel hairline ring inside the whole
/// window (`.app7 { box-shadow: inset 0 0 0 1px var(--hairc) }`), out of
/// flow so it costs the bands nothing and takes no click.
fn edge(lex: &Lexicon) -> Element {
    Element::surface()
        .pin_left(0.0)
        .pin_top(0.0)
        .pin_right(0.0)
        .pin_bottom(0.0)
        .stroke(lex.grounded(lex.hairc), 1.0)
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_root.tests.rs"]
mod tests;
