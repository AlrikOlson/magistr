use magistr_cockpit::cockpit::{
    Density, Model, Motion, RuleMode, SettingsData, SettingsGroup, SettingsScope, ThemeChoice,
};

use crate::component::Component;
use crate::crumb::Crumb;
use crate::crumb_stop::CrumbStop;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::identity_row::IdentityRow;
use crate::key_hint::KeyHint;
use crate::led::Led;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::mode_badge::ModeBadge;
use crate::receipt_row::ReceiptRow;
use crate::segments::Segments;
use crate::setting_row::SettingRow;
use crate::settings_nav::SettingsNav;
use crate::switch::Switch;
use crate::theme_card::ThemeCard;
use crate::theme_preview::ThemePreview;
use crate::voice::Voice;

/// The ⌘, settings: the seventh edition's view 10 driven by the model — a real surface
/// over the whole window: its own header band (`settings` · `ESC · CLOSE`), the
/// searchable sidebar of scopes (This app · This project under its sigil · This team),
/// one pane per group, every pane ending with its source file and an `Open file` verb.
/// Controls match their content: theme cards for the theme, switches for booleans,
/// segments for density. Motion: none — settings hold still.
pub struct SettingsComposition;

impl SettingsComposition {
    /// `ESC · CLOSE` at the header's edge.
    pub const CLOSE: ElementId = ElementId(350);
    /// The open pane's `Open file`.
    pub const OPEN: ElementId = ElementId(351);
    /// The sidebar items: `NAV + index` for the group at `index` in
    /// [`SettingsGroup::ALL`], `NAV_END` exclusive.
    pub const NAV: u64 = 1 << 28;
    /// The end of the nav space.
    pub const NAV_END: u64 = Self::NAV + SettingsGroup::ALL.len() as u64;
    /// The theme cards: `THEME + index` for [`ThemeChoice::ALL`].
    pub const THEME: u64 = 1 << 29;
    /// The end of the card space.
    pub const THEME_END: u64 = Self::THEME + ThemeChoice::ALL.len() as u64;
    /// The switches: `SWITCH + index` down the panes (0 the heatmap, 1
    /// the grain, 2 reduced motion).
    pub const SWITCH: u64 = Self::THEME + 16;
    /// The end of the switch space.
    pub const SWITCH_END: u64 = Self::SWITCH + 3;
    /// The density segments: `DENSITY + index` for [`Density::ALL`].
    pub const DENSITY: u64 = Self::THEME + 32;
    /// The end of the segment space.
    pub const DENSITY_END: u64 = Self::DENSITY + Density::ALL.len() as u64;

    /// The header band: `padding:15px 20px 12px 92px` over its
    /// hairline — the lights' room before the crumb.
    const HEADER_PAD_TOP: f32 = 15.0;
    const HEADER_PAD_BOTTOM: f32 = 12.0;
    const HEADER_PAD_LEFT: f32 = 92.0;
    const HEADER_PAD_RIGHT: f32 = 20.0;
    /// The sidebar: 236 wide including its hairline, `padding:14px
    /// 10px; gap:2px`.
    const SIDEBAR_WIDTH: f32 = 236.0;
    const SIDEBAR_PAD_X: f32 = 10.0;
    const SIDEBAR_PAD_Y: f32 = 14.0;
    const SIDEBAR_GAP: f32 = 2.0;
    /// The search box: radius 8, `padding:6px 10px`, `gap:8px`, 10
    /// below; its placeholder in sans 11.5.
    const SEARCH_RADIUS: f32 = 8.0;
    const SEARCH_PAD_X: f32 = 10.0;
    const SEARCH_PAD_Y: f32 = 6.0;
    const SEARCH_GAP: f32 = 8.0;
    const SEARCH_BELOW: f32 = 10.0;
    const SEARCH_ICON_PX: u32 = 13;
    const SEARCH_VOICE: Voice = Voice {
        px: 11.5,
        ..Voice::SLOT
    };
    /// A scope cap's box: `padding:4px 10px 5px` for the first, `14px
    /// 10px 4px` / `14px 10px 5px` for the next two.
    const CAP_PAD_X: f32 = 10.0;
    /// The identity row's box under `This project`: `padding:2px 10px
    /// 7px` around [`IdentityRow`].
    const IDENTITY_PAD_TOP: f32 = 2.0;
    const IDENTITY_PAD_BOTTOM: f32 = 7.0;
    /// The pane: `padding:26px 34px`; its head sans 15/600 with 3
    /// below, its sub-line mono 10 with 18 below. The top is one
    /// pixel under the volume's: the browser rounds the head's line
    /// box a pixel up, measured against the capture.
    const PANE_PAD_X: f32 = 34.0;
    const PANE_PAD_TOP: f32 = 25.0;
    const HEAD_VOICE: Voice = Voice {
        px: 15.0,
        weight: 600,
        ..Voice::SLOT
    };
    const HEAD_BELOW: f32 = 3.0;
    const SUB_BELOW: f32 = 18.0;
    /// The theme block: the cap with 10 below, the cards 132×74 with
    /// 16 between and 24 below the row.
    const THEME_CAP_BELOW: f32 = 10.0;
    const CARD: (f32, f32) = ThemePreview::DRAWING;
    const CARDS_GAP: f32 = 16.0;
    const CARDS_BELOW: f32 = 24.0;
    /// The rows block: `max-width:480px`, its cap with 4 below, the
    /// source row 14 under the last row.
    const ROWS_WIDTH: f32 = 480.0;
    const ROWS_CAP_BELOW: f32 = 4.0;
    const SOURCE_ABOVE: f32 = 14.0;

    /// The settings surface for `model`, filling the window — `None`
    /// while ⌘, is down.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        let settings = model.settings()?;
        let lex = Lexicon::of(medium);
        let data = model.settings_data();
        let group = settings.group();
        Some(
            Element::surface()
                .grow(1.0)
                .min_height(0.0)
                .child(header(&lex))
                .child(
                    Element::row()
                        .grow(1.0)
                        .min_height(0.0)
                        .child(sidebar(model, &settings.visible(), settings.query(), &lex))
                        .child(
                            Element::surface()
                                .width(1.0)
                                .stretch()
                                .fill(lex.grounded(lex.hairc)),
                        )
                        .child(pane(model, data, group, medium, &lex)),
                ),
        )
    }
}

/// The header band: the room the lights take, the `settings` crumb at
/// its here stop, and `ESC · CLOSE` at the far edge — over the band's
/// hairline.
fn header(lex: &Lexicon) -> Element {
    let stops = [CrumbStop::here(
        "settings",
        Some(Mark::Icon(Icon::Settings)),
    )];
    Element::surface()
        .child(air(SettingsComposition::HEADER_PAD_TOP))
        .child(
            Element::row()
                .center()
                .child(air_x(SettingsComposition::HEADER_PAD_LEFT))
                .child(Crumb::el(&stops, lex))
                .child(Element::row().grow(1.0))
                .child(
                    Element::text(Voice::CAP, lex.ov0, "esc \u{b7} Close")
                        .claim(Component::KeyHint)
                        .nowrap()
                        .id(SettingsComposition::CLOSE)
                        .focusable()
                        .focus_ring(lex.mauve),
                )
                .child(air_x(SettingsComposition::HEADER_PAD_RIGHT)),
        )
        .child(air(SettingsComposition::HEADER_PAD_BOTTOM))
        .child(Element::surface().height(1.0).fill(lex.grounded(lex.hairc)))
}

/// The sidebar: the search box, then each scope's cap and its visible
/// groups — the project's under its identity row, the team's counted
/// and the read-only ones tagged.
fn sidebar(model: &Model, visible: &[SettingsGroup], query: &str, lex: &Lexicon) -> Element {
    let mut column = Element::surface()
        .width(SettingsComposition::SIDEBAR_WIDTH - 1.0)
        .no_shrink()
        .pad_x(SettingsComposition::SIDEBAR_PAD_X)
        .pad_y(SettingsComposition::SIDEBAR_PAD_Y)
        .gap(SettingsComposition::SIDEBAR_GAP)
        .child(search_box(query, lex));
    let data = model.settings_data();
    for (index, scope) in SettingsScope::ALL.into_iter().enumerate() {
        let groups: Vec<SettingsGroup> = visible
            .iter()
            .copied()
            .filter(|g| g.scope() == scope)
            .collect();
        if groups.is_empty() {
            continue;
        }
        let (above, below) = if index == 0 { (4.0, 5.0) } else { (14.0, 4.0) };
        let below = if scope == SettingsScope::Team {
            5.0
        } else {
            below
        };
        let cap_text = match (scope, data.team_size) {
            (SettingsScope::Team, Some(1)) => "This team \u{b7} 1 person".to_owned(),
            (SettingsScope::Team, Some(n)) => format!("This team \u{b7} {n} people"),
            _ => scope.label().to_owned(),
        };
        column = column.child(
            Element::surface()
                .pad_x(SettingsComposition::CAP_PAD_X)
                .pad_y(above)
                .pad_bottom(below)
                .child(
                    Element::text(Voice::CAP, lex.ov0, cap_text)
                        .claim(Component::Cap)
                        .nowrap(),
                ),
        );
        if scope == SettingsScope::Project {
            column = column.child(identity(model, lex));
        }
        let active = model.settings().map(|s| s.group());
        for group in groups {
            column = column.child(SettingsNav::tagged(
                icon_of(group),
                group.label(),
                active == Some(group),
                group.is_read_only().then_some("read-only"),
                lex.base,
                ElementId(SettingsComposition::NAV + group.index() as u64),
                lex,
            ));
        }
    }
    column
}

/// `Search settings` — the placeholder in the faint ink, or what has
/// been typed in the text ink.
fn search_box(query: &str, lex: &Lexicon) -> Element {
    let (ink, text) = if query.is_empty() {
        (lex.ov0, "Search settings")
    } else {
        (lex.tx, query)
    };
    Element::surface()
        .pad_bottom(SettingsComposition::SEARCH_BELOW)
        .child(
            Element::row()
                .gap(SettingsComposition::SEARCH_GAP)
                .center()
                .radius(SettingsComposition::SEARCH_RADIUS)
                .pad_x(SettingsComposition::SEARCH_PAD_X)
                .pad_y(SettingsComposition::SEARCH_PAD_Y)
                .stroke(lex.grounded(lex.hairc), 1.0)
                .child(Element::mark(
                    Mark::Icon(Icon::Search),
                    SettingsComposition::SEARCH_ICON_PX,
                    Lexicon::ink_bytes(lex.ov0),
                ))
                .child(
                    Element::text(SettingsComposition::SEARCH_VOICE, ink, text)
                        .claim(Component::Field)
                        .nowrap(),
                ),
        )
}

/// The project under its sigil: sigil · name · path. The project under its sigil: the
/// identity row in the sidebar's box. The sigil is a pure function of a 'static name
/// (`Mark`'s own contract), so only the brand's default wears one — the header
/// composition's rule.
fn identity(model: &Model, lex: &Lexicon) -> Element {
    let sigil = model.title().is_none().then_some(Mark::Sigil("magistr"));
    let path = &model.settings_data().project_path;
    Element::row()
        .pad_x(SettingsComposition::CAP_PAD_X)
        .pad_y(SettingsComposition::IDENTITY_PAD_TOP)
        .pad_bottom(SettingsComposition::IDENTITY_PAD_BOTTOM)
        .child(IdentityRow::el(
            sigil,
            project_name(model),
            Some(path.as_str()),
            lex,
        ))
}

/// The project's name as the launcher spells it, or the brand's.
fn project_name(model: &Model) -> &str {
    model
        .launcher()
        .map(|data| data.project.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("magistr")
}

/// The glyph each group wears in the sidebar — the volume's own
/// choices (`ic-image`, `ic-orbit`, `ic-time`, `ic-kbd`, `ic-gate`,
/// `ic-cost`, `ic-agent`, `ic-box`, `ic-galaxy`, `ic-checkpoint`).
fn icon_of(group: SettingsGroup) -> Icon {
    match group {
        SettingsGroup::Appearance => Icon::Image,
        SettingsGroup::Motion => Icon::Session,
        SettingsGroup::Rest => Icon::Time,
        SettingsGroup::Shortcuts => Icon::Keys,
        SettingsGroup::Gates => Icon::Gate,
        SettingsGroup::Budget => Icon::Cost,
        SettingsGroup::Charter => Icon::Agent,
        SettingsGroup::Backend => Icon::Stack,
        SettingsGroup::Members => Icon::Projects,
        SettingsGroup::Policy => Icon::Checkpoint,
    }
}

/// The open group's pane: its head repeating the scope, its content,
/// and the source row every pane ends with.
fn pane(
    model: &Model,
    data: &SettingsData,
    group: SettingsGroup,
    medium: Medium,
    lex: &Lexicon,
) -> Element {
    let scope = group.scope();
    let mut pane = Element::surface()
        .grow(1.0)
        .min_width(0.0)
        .align_start()
        .pad_x(SettingsComposition::PANE_PAD_X)
        .pad_y(SettingsComposition::PANE_PAD_TOP)
        .child(
            Element::text(SettingsComposition::HEAD_VOICE, lex.tx, group.label())
                .claim(Component::Title)
                .nowrap(),
        )
        .child(air(SettingsComposition::HEAD_BELOW))
        .child(
            Element::text(
                Voice::LEDGER_META,
                lex.ov0,
                format!(
                    "Scope: {} \u{b7} source: {}",
                    scope.reach(),
                    scope.source_name()
                ),
            )
            .claim(Component::Cap)
            .nowrap(),
        )
        .child(air(SettingsComposition::SUB_BELOW));
    if group == SettingsGroup::Appearance {
        pane = pane.child(theme_block(data, lex));
    }
    let rows = match group {
        SettingsGroup::Appearance => interface_rows(data, lex),
        SettingsGroup::Motion => vec![SettingRow::el(
            "Reduced motion",
            "Nothing decorative moves",
            Switch::el(
                !model.theme().motion().is_full(),
                ElementId(SettingsComposition::SWITCH + 2),
                lex,
            ),
            false,
            lex.base,
            lex,
        )],
        SettingsGroup::Rest => {
            let secs = Motion::REST_STEPS as f32 * Model::TICK_PERIOD.as_secs_f32();
            vec![SettingRow::el(
                "Rest after",
                "Idle before the chrome exhales",
                value(&format!("{} s", secs.round()), lex),
                false,
                lex.base,
                lex,
            )]
        }
        SettingsGroup::Shortcuts => {
            let chords = [
                ("Palette", "Jump to a place or a verb", "\u{2318}K", "Jump"),
                (
                    "Search",
                    "Find a chapter, a step, a receipt",
                    "\u{2318}\u{21e7}F",
                    "Find",
                ),
                ("Settings", "This surface", "\u{2318},", "Open"),
                ("Close", "Put a float away", "esc", "Close"),
            ];
            chords
                .iter()
                .enumerate()
                .map(|(i, (name, note, cap, verb))| {
                    SettingRow::el(
                        name,
                        note,
                        KeyHint::el(cap, verb, medium, lex.base, lex),
                        i + 1 < chords.len(),
                        lex.base,
                        lex,
                    )
                })
                .collect()
        }
        SettingsGroup::Gates => gate_rows(model, lex),
        SettingsGroup::Budget => vec![match model.rail().budget() {
            Some(budget) => SettingRow::el(
                "Budget",
                "The bound cap, spent to date",
                value(&budget.readout(), lex),
                false,
                lex.base,
                lex,
            ),
            None => SettingRow::el(
                "Budget",
                "No budget bound",
                value("\u{2014}", lex),
                false,
                lex.base,
                lex,
            ),
        }],
        SettingsGroup::Charter => charter_rows(model, lex),
        SettingsGroup::Backend => vec![SettingRow::el(
            "Agent backend",
            "Found on PATH at launch",
            value(
                model.launcher().map_or("ACP", |data| data.backend.as_str()),
                lex,
            ),
            false,
            lex.base,
            lex,
        )],
        SettingsGroup::Members => vec![SettingRow::el(
            "People",
            "The roster the team file lists",
            value(
                &data
                    .team_size
                    .map_or_else(|| "solo".to_owned(), |n| n.to_string()),
                lex,
            ),
            false,
            lex.base,
            lex,
        )],
        SettingsGroup::Policy => vec![SettingRow::el(
            "Ownership",
            "Guardrails the team publishes",
            value("read-only", lex),
            false,
            lex.base,
            lex,
        )],
    };
    let cap_word = match group {
        SettingsGroup::Appearance => "Interface",
        SettingsGroup::Motion => "Motion",
        SettingsGroup::Rest => "Rest",
        SettingsGroup::Shortcuts => "Chords",
        SettingsGroup::Gates => "Quality gates",
        SettingsGroup::Budget => "Budget & limits",
        SettingsGroup::Charter => "Rules",
        SettingsGroup::Backend => "Backend",
        SettingsGroup::Members => "Members",
        SettingsGroup::Policy => "Policy",
    };
    let mut block = Element::surface()
        .max_width(SettingsComposition::ROWS_WIDTH)
        .stretch()
        .child(
            Element::text(Voice::CAP, lex.ov0, cap_word)
                .claim(Component::Cap)
                .nowrap(),
        )
        .child(air(SettingsComposition::ROWS_CAP_BELOW));
    for row in rows {
        block = block.child(row);
    }
    block = block
        .child(air(SettingsComposition::SOURCE_ABOVE))
        .child(ReceiptRow::linked(
            (Icon::Report, "source"),
            group.source(),
            "Open file",
            SettingsComposition::OPEN,
            false,
            lex.base,
            lex,
        ));
    pane.child(block)
}

/// The theme cards under their cap: the sky, the chart, the OS split,
/// the chosen one ringed and checked.
fn theme_block(data: &SettingsData, lex: &Lexicon) -> Element {
    let (w, h) = SettingsComposition::CARD;
    let mut cards = Element::row().gap(SettingsComposition::CARDS_GAP);
    for (index, choice) in ThemeChoice::ALL.into_iter().enumerate() {
        let preview = match choice {
            ThemeChoice::Sky => ThemePreview::sky(w, h),
            ThemeChoice::Chart => ThemePreview::chart(w, h),
            ThemeChoice::FollowOs => ThemePreview::split(w, h),
        };
        cards = cards.child(ThemeCard::sized(
            choice.label(),
            data.theme == choice,
            preview,
            (w, h),
            ElementId(SettingsComposition::THEME + index as u64),
            lex,
        ));
    }
    Element::surface()
        .child(
            Element::text(Voice::CAP, lex.ov0, "Theme")
                .claim(Component::Cap)
                .nowrap(),
        )
        .child(air(SettingsComposition::THEME_CAP_BELOW))
        .child(cards)
        .child(air(SettingsComposition::CARDS_BELOW))
}

/// The Interface rows: two switches and the density segments.
fn interface_rows(data: &SettingsData, lex: &Lexicon) -> Vec<Element> {
    let density = Density::ALL
        .iter()
        .position(|d| *d == data.density)
        .unwrap_or(0);
    let labels: Vec<&str> = Density::ALL.iter().map(|d| d.label()).collect();
    vec![
        SettingRow::el(
            "Show activity heatmap",
            "On the project view",
            Switch::el(data.heatmap, ElementId(SettingsComposition::SWITCH), lex),
            true,
            lex.base,
            lex,
        ),
        SettingRow::el(
            "Texture",
            "Film grain, night only",
            Switch::el(data.grain, ElementId(SettingsComposition::SWITCH + 1), lex),
            true,
            lex.base,
            lex,
        ),
        SettingRow::el(
            "Density",
            "Row height and padding",
            Segments::el(
                &labels,
                density,
                ElementId(SettingsComposition::DENSITY),
                lex,
            ),
            false,
            lex.base,
            lex,
        ),
    ]
}

/// The gates the last loop run declared, each with its light — or the
/// honest absence.
fn gate_rows(model: &Model, lex: &Lexicon) -> Vec<Element> {
    let gates = model.loop_status().map(|s| s.gates()).unwrap_or_default();
    if gates.is_empty() {
        return vec![SettingRow::el(
            "Gates",
            "Declared in the project config; none has run this session",
            value("\u{2014}", lex),
            false,
            lex.base,
            lex,
        )];
    }
    gates
        .iter()
        .enumerate()
        .map(|(i, gate)| {
            SettingRow::el(
                gate.name(),
                match gate.passed() {
                    Some(true) => "Passed",
                    Some(false) => "Failed",
                    None => "Running",
                },
                Led::el(gate.passed() == Some(true), lex),
                i + 1 < gates.len(),
                lex.base,
                lex,
            )
        })
        .collect()
}

/// The charter's rules with their mode badges — the rail's own rows.
fn charter_rows(model: &Model, lex: &Lexicon) -> Vec<Element> {
    let charter = model.rail().charter();
    if charter.is_empty() {
        return vec![SettingRow::el(
            "Charter",
            "No rules bound",
            value("\u{2014}", lex),
            false,
            lex.base,
            lex,
        )];
    }
    charter
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            let badge = match rule.mode() {
                RuleMode::Ask => ModeBadge::ask(lex.base, lex),
                RuleMode::Auto => ModeBadge::auto(lex.base, lex),
                RuleMode::Off => ModeBadge::off(lex.base, lex),
            };
            ReceiptRow::rule(rule.subject(), badge, i + 1 < charter.len(), lex.base, lex)
        })
        .collect()
}

/// A read-only value at a row's edge, in the receipt value's voice.
fn value(text: &str, lex: &Lexicon) -> Element {
    Element::text(Voice::VALUE, lex.sb1, text)
        .claim(Component::Value)
        .nowrap()
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

/// A rigid run of horizontal air.
fn air_x(px: f32) -> Element {
    Element::row().width(px).no_shrink()
}

#[cfg(test)]
#[path = "comp_settings.tests.rs"]
mod tests;
