use magistr_cockpit::cockpit::{Copy, GalaxyProject, Model};
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::budget_meter::BudgetMeter;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::galaxy_layout::GalaxyLayout;
use crate::interned::interned;
use crate::key_hint::KeyHint;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::pill::DataPill;
use crate::placed_cluster::PlacedCluster;
use crate::voice::Voice;

/// The Galaxy, composed from the model: every project as a cluster wearing its sigil
/// and its status line, `＋ New project` floated over the field, and the two bands the
/// root borrows — the header's running / action-required / ⌘K chips and the projects /
/// meter / spend foot with its `↵ Open · scroll Zoom` hints.
///
/// The constellations themselves are not composed: the
/// [`Galaxy`](crate::galaxy::Galaxy) drawing owns the field's pixels and this
/// composition reserves the field ([`GalaxyComposition::FIELD`]) it draws into — but
/// the *labels* are composed, pinned inside that field at the anchors [`GalaxyLayout`]
/// answers, and the drawing places its stars from the same call, so a name and its
/// stars cannot come apart.
///
/// Every string here is derived from the project's numbers: the status line is the one
/// place the Galaxy's copy is written, so "1 Action required · 11 min" and "Idle · 3
/// days" cannot be spelled two ways.
pub struct GalaxyComposition;

impl GalaxyComposition {
    /// The field the clusters are drawn into — a reservation, not a
    /// drawing; the wheel over it zooms.
    pub const FIELD: ElementId = ElementId(330);
    /// The `＋ New project` verb.
    pub const NEW: ElementId = ElementId(331);
    /// Project links use a separate element range from the other compositions.
    pub const PROJECTS: u64 = 1 << 41;
    pub const PROJECTS_END: u64 = Self::PROJECTS + (1 << 32);

    /// The Galaxy's own chrome bands, measured off view 01: the header
    /// is `padding:15px 20px 12px 92px` over its hairline (51 high, its
    /// hairline on the capture's row 50), the foot is
    /// `padding:8px 20px` under its hairline, 36 high. Crate visible:
    /// the root lays the field between them and pads the header's right
    /// edge by the Galaxy's own 20.
    pub(crate) const HEADER_HEIGHT: f32 = 51.0;
    pub(crate) const HEADER_GAP: f32 = 20.0;
    /// The band's padding is 15 above and 12 below: the row sits three
    /// pixels lower than centred.
    pub(crate) const HEADER_LIFT: f32 = 3.0;
    pub(crate) const FOOT_HEIGHT: f32 = 36.0;
    /// Where the verb floats off the field's bottom-right corner
    /// (`right:110px; bottom:120px`).
    const NEW_RIGHT: f32 = 110.0;
    const NEW_BOTTOM: f32 = 120.0;
    /// The label's sigil box (`.sig` 14px) and the gap to the name.
    const SIGIL_PX: u32 = 14;
    const LABEL_GAP: f32 = 6.0;
    /// The header chips' pitch and the foot's (`gap:14px` / `gap:16px`).
    const CHIP_GAP: f32 = 14.0;
    const FOOT_GAP: f32 = 16.0;
    /// The foot's side margins and its meter's measure (`width:170px`).
    const FOOT_PAD: f32 = 20.0;
    const METER_W: f32 = 170.0;
    /// The project name's voice — the volume's sans 13/600.
    const NAME: Voice = Voice {
        family: "sans",
        px: 13.0,
        weight: 600,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };

    /// The projects the Galaxy shows, in the order their cluster ids
    /// count: the job's list, with what this window knows better laid
    /// over its own project — the rail's shipped count, the dock's
    /// pending questions, a running loop. Project identities come from the
    /// shared model, including the current project supplied by the host.
    #[must_use]
    pub fn projects(model: &Model) -> Vec<GalaxyProject> {
        let mut projects = model.galaxy().projects().to_vec();
        let (done, total) = model.rail().progress();
        let asking = u32::try_from(model.dock().len()).unwrap_or(u32::MAX);
        let running = u32::from(model.loop_status().is_some());
        for project in projects.iter_mut().filter(|project| project.here) {
            if total > 0 {
                project.done = Some(done);
            }
            project.asking = project.asking.max(asking);
            project.running = project.running.max(running);
        }
        projects
    }

    /// The field for `model`, `field` logical pixels big, in `medium`'s
    /// lexicon: the reservation, the labels pinned at their clusters,
    /// and the verb.
    #[must_use]
    pub fn el(model: &Model, medium: Medium, field: (f32, f32)) -> Element {
        let lex = Lexicon::of(medium);
        let projects = Self::projects(model);
        let placed = GalaxyLayout::place(&projects, field, model.galaxy().zoom());
        let mut room = Element::surface()
            .grow(1.0)
            .min_height(0.0)
            .fill(lex.base)
            .id(Self::FIELD);
        for (index, (project, cluster)) in projects.iter().zip(&placed).enumerate() {
            room = room.child(label(project, cluster, index, medium, &lex));
        }
        room.child(
            ActionRow::new(None, vec![Action::new("+ New project", Self::NEW)])
                .el(medium)
                .pin_right(Self::NEW_RIGHT)
                .pin_bottom(Self::NEW_BOTTOM),
        )
    }

    /// The header's right-hand chips: `N running` live, `N action
    /// required` quiet — each only while it counts — and `⌘K`.
    #[must_use]
    pub fn chips(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let projects = Self::projects(model);
        let running: u32 = projects.iter().map(|project| project.running).sum();
        let asking: u32 = projects.iter().map(|project| project.asking).sum();
        let mut row = Element::row().gap(Self::CHIP_GAP).center();
        if running > 0 {
            row = row.child(DataPill::live(&format!("{running} running"), &lex));
        }
        if asking > 0 {
            row = row.child(DataPill::quiet(
                &format!("{asking} action{} required", plural(u64::from(asking))),
                &lex,
            ));
        }
        row.child(
            ActionRow::new(
                None,
                vec![
                    Action::new(
                        "Commands",
                        crate::comp_workspace::WorkspaceComposition::id(
                            magistr_cockpit::cockpit::Widget::Commands.id(),
                        ),
                    )
                    .cap("\u{2318}K"),
                ],
            )
            .el(medium),
        )
    }

    /// The Galaxy's foot: the project count as a cap, the spend meter
    /// and its `spent / cap USD` value while a cap is known, and the two
    /// key hints at the right.
    #[must_use]
    pub fn foot(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let count = Self::projects(model).len();
        let mut bar = Element::row()
            .height(Self::FOOT_HEIGHT - 1.0)
            .pad_x(Self::FOOT_PAD)
            .gap(Self::FOOT_GAP)
            .center()
            .child(
                Element::text(
                    Voice::CAP,
                    lex.ov0,
                    format!(
                        "{count} project{}",
                        plural(u64::try_from(count).unwrap_or(u64::MAX))
                    ),
                )
                .claim(Component::Counter)
                .nowrap(),
            );
        if let Some(data) = model.galaxy().data() {
            let spent = data.spent_cents;
            match data.cap_cents.filter(|cap| *cap > 0) {
                Some(cap) => {
                    #[allow(clippy::cast_precision_loss)]
                    let share = |cents: u64| cents as f32 / cap as f32;
                    bar = bar
                        .child(BudgetMeter::track(
                            share(spent),
                            data.stop_cents.map(share),
                            Self::METER_W,
                            &lex,
                        ))
                        .child(
                            Element::text(
                                Voice::LEDGER_META,
                                lex.sb0,
                                format!("{} USD", Copy::pair(spent, cap)),
                            )
                            .claim(Component::Meter)
                            .nowrap(),
                        );
                }
                None => {
                    bar = bar.child(
                        Element::text(Voice::LEDGER_META, lex.sb0, Copy::usd(spent))
                            .claim(Component::Meter)
                            .nowrap(),
                    );
                }
            }
        }
        bar = bar
            .child(Element::row().grow(1.0))
            .child(KeyHint::el("\u{21b5}", "Open", medium, lex.base, &lex))
            .child(KeyHint::el("scroll", "Zoom", medium, lex.base, &lex));
        // The bar opens on its own hairline — the capture's row 604.
        Element::surface()
            .child(Element::surface().height(1.0).fill(lex.grounded(lex.hairc)))
            .child(bar)
    }
}

/// One cluster's label, pinned at the layout's anchor: the sigil and
/// the name on one row, the status line under them.
fn label(
    project: &GalaxyProject,
    cluster: &PlacedCluster,
    index: usize,
    medium: Medium,
    lex: &Lexicon,
) -> Element {
    let (name_ink, status, status_ink) = status_line(project, lex);
    let action = Action::new(
        "Open project",
        ElementId(GalaxyComposition::PROJECTS + index as u64),
    );
    let action = if project.here {
        action
    } else {
        action.disabled()
    };
    let mut open = ActionRow::new(None, vec![action]).el(medium);
    for button in &mut open.children {
        button.label = Some(format!("Open project {}", project.name));
    }
    Element::surface()
        .align_start()
        .pin_left(cluster.label.0)
        .pin_top(cluster.label.1)
        .child(
            Element::row()
                .gap(GalaxyComposition::LABEL_GAP)
                .center()
                .child(Element::mark(
                    Mark::Sigil(interned(&project.name)),
                    GalaxyComposition::SIGIL_PX,
                    Lexicon::ink_bytes(name_ink),
                ))
                .child(
                    Element::text(GalaxyComposition::NAME, name_ink, project.name.as_str())
                        .claim(Component::Label)
                        .nowrap(),
                ),
        )
        .child(
            Element::text(Voice::LEDGER_META, status_ink, status)
                .claim(Component::Cap)
                .nowrap(),
        )
        .child(Element::surface().height(8.0))
        .child(open)
}

/// The one place the Galaxy's copy is written: the name's ink, the
/// status line and its ink. A question outranks a session (amber, and
/// how long it has waited); a session says what shipped and what runs;
/// an idle project dims its name and says how long it has slept.
fn status_line(project: &GalaxyProject, lex: &Lexicon) -> (Srgba, String, Srgba) {
    if project.asking > 0 {
        let mut text = format!("{} Action required", project.asking);
        if let Some(age) = project.asked_secs {
            text.push_str(&format!(" \u{b7} {}", ago(age)));
        }
        return (lex.tx, text, lex.peach);
    }
    if project.running > 0 {
        let text = match project.done {
            Some(done) => format!("{done} done \u{b7} {} running", project.running),
            None => format!("{} running", project.running),
        };
        return (lex.tx, text, lex.ov0);
    }
    let text = match (project.idle_secs, project.done) {
        (Some(idle), _) => format!("Idle \u{b7} {}", ago(idle)),
        (None, Some(done)) => format!("{done} done \u{b7} idle"),
        (None, None) => "Idle".to_owned(),
    };
    (lex.sb1, text, lex.ov0)
}

/// A duration the way the copy law prints one — `11 min`, `3 d` —
/// [`Copy::duration`].
fn ago(secs: u64) -> String {
    Copy::duration(secs)
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
#[path = "comp_galaxy.tests.rs"]
mod tests;
