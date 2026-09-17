//! The atom-scale parity gate.
//!
//! Every control atom is rendered by [`atom_still`] at its specimen
//! capture's own pixel size, in both media, and diffed against the capture under
//! [`Parity`]'s channel tolerance. The thresholds are per specimen, pinned to what the
//! atom actually measures with honest headroom — an atom that drifts past its pinned
//! distance fails by name.
//!
//! `ATOM_PARITY_DUMP=1` writes each still and diff beside the target dir for review;
//! the numbers, not the pictures, decide.

use magistr_cockpit::theme::Severity;
use magistr_gui2::{
    Action, ActionRow, BudgetMeter, Card, CmdkRow, Composer, Crumb, CrumbStop, DataPill, DiffBlock,
    DiffRow, Edge, Element, ElementId, ElementState, Icon, KeyHint, Led, Ledger, Lexicon, Mark,
    Mask, Medium, ModeBadge, NavChip, Parity, PhaseRail, ReceiptRow, SearchField, Segments,
    SettingsNav, SeverityCap, SeverityInk, Slot, Star, Stat, StatusRow, Switch, ThemeCard, Voice,
    atom_still, emit,
};
use qs_gpu::color::Srgba;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

/// One gated atom: its specimen id, the share of pixels it may differ
/// by (per medium — day type renders a hair differently), and its
/// fixture.
struct Gated {
    id: &'static str,
    threshold: [f32; 2],
    fixture: fn(&Lexicon) -> Element,
}

/// The volume's own fixtures — the copies and states the specimen sheet shows, the
/// design class by class.
const GATED: &[Gated] = &[
    Gated {
        id: "control-data-chip-live",
        threshold: [0.155, 0.14],
        fixture: |lex| DataPill::live("Executing · 04:12", lex),
    },
    Gated {
        id: "control-data-chip-gates",
        threshold: [0.115, 0.105],
        fixture: |lex| DataPill::ok("gates ✓✓✓", lex),
    },
    Gated {
        id: "control-data-chip-num",
        threshold: [0.095, 0.1],
        fixture: |lex| DataPill::num("1.84 / 12.00", lex),
    },
    Gated {
        id: "control-nav-chip",
        threshold: [0.15, 0.14],
        fixture: |lex| NavChip::el("gui2-2a · execute", ElementId(1), lex),
    },
    Gated {
        id: "control-nav-crumb",
        threshold: [0.155, 0.135],
        fixture: |lex| {
            Crumb::el(
                &[
                    CrumbStop::back("projects", Some(Mark::Icon(Icon::Projects)), ElementId(1)),
                    CrumbStop::here("magistr", Some(Mark::Sigil("magistr"))),
                ],
                lex,
            )
        },
    },
    Gated {
        id: "control-action-primary",
        threshold: [0.115, 0.115],
        fixture: |lex| {
            ActionRow::new(
                Some(Action::new("Approve & merge", ElementId(1)).cap("A")),
                Vec::new(),
            )
            .el(medium_of(lex))
        },
    },
    Gated {
        id: "control-action-secondary",
        threshold: [0.09, 0.07],
        fixture: |lex| {
            ActionRow::new(None, vec![Action::new("Request changes…", ElementId(2))])
                .el(medium_of(lex))
        },
    },
    Gated {
        id: "control-action-negative",
        threshold: [0.09, 0.04],
        fixture: |lex| {
            ActionRow::new(
                None,
                vec![Action::new("Deny", ElementId(3)).cap("D").negative()],
            )
            .el(medium_of(lex))
        },
    },
    Gated {
        id: "control-segment",
        threshold: [0.125, 0.11],
        fixture: |lex| {
            Segments::el(
                &["The sky", "The chart", "Follow OS"],
                2,
                ElementId(10),
                lex,
            )
        },
    },
    Gated {
        id: "control-switch-on",
        threshold: [0.17, 0.1],
        fixture: |lex| Switch::el(true, ElementId(20), lex),
    },
    Gated {
        id: "control-switch-off",
        threshold: [0.035, 0.06],
        fixture: |lex| Switch::el(false, ElementId(21), lex),
    },
    Gated {
        id: "control-theme-card",
        threshold: [0.025, 0.065],
        fixture: |lex| {
            ThemeCard::el(
                "The sky",
                false,
                ThemeCard::sky_preview(lex),
                ElementId(30),
                lex,
            )
        },
    },
    Gated {
        id: "control-theme-card-chosen",
        threshold: [0.105, 0.1],
        fixture: |lex| ThemeCard::el("Follow OS", true, split_preview(), ElementId(31), lex),
    },
    Gated {
        id: "control-slot",
        threshold: [0.095, 0.065],
        fixture: |lex| Slot::el("Drop a folder here", lex),
    },
    // The specimen was cut with its track one row above where view 05's capture puts it
    // under the same cap (16 rows cap-to-track against the view's 17 — the specimen
    // page's own sub-pixel offset, snapped the other way). Painted boxes snap to the
    // grid the way the browser paints them and the view is the fixed point, so the
    // meter now matches the view row for row and sits one row off this specimen: the
    // pin carries that row.
    Gated {
        id: "meter",
        threshold: [0.13, 0.125],
        fixture: |lex| {
            BudgetMeter::el(
                "Budget",
                "1.84 / 12.00",
                0.15,
                Some(0.83),
                None,
                None,
                220.0,
                lex,
            )
        },
    },
    // ── the a3 surface and copy-law vocabulary ───────────────────────
    Gated {
        id: "led-gate-on",
        threshold: [0.075, 0.15],
        fixture: |lex| Led::el(true, lex),
    },
    Gated {
        id: "led-gate-off",
        threshold: [0.01, 0.01],
        fixture: |lex| Led::el(false, lex),
    },
    Gated {
        id: "badge-mode-off",
        threshold: [0.245, 0.23],
        fixture: |lex| ModeBadge::off(lex.base, lex),
    },
    Gated {
        id: "badge-mode-auto",
        threshold: [0.125, 0.13],
        fixture: |lex| ModeBadge::auto(lex.base, lex),
    },
    Gated {
        id: "key-hint",
        threshold: [0.18, 0.15],
        fixture: |lex| KeyHint::el("\u{23ce}", "Open", medium_of(lex), lex.base, lex),
    },
    Gated {
        id: "indicator-counter",
        threshold: [0.105, 0.1],
        fixture: |lex| centered(Stat::el("211", "306", lex)),
    },
    Gated {
        id: "indicator-phases",
        threshold: [0.215, 0.19],
        fixture: |lex| {
            centered(PhaseRail::el(
                &["research", "plan", "execute", "verify"],
                2,
                lex,
            ))
        },
    },
    Gated {
        id: "bar-budget",
        threshold: [0.02, 0.025],
        fixture: |lex| magistr_gui2::ThinBar::el(0.15, 150.0, lex),
    },
    Gated {
        id: "row-status-executing",
        threshold: [0.045, 0.04],
        fixture: |lex| {
            StatusRow::el(
                lex.mauve,
                "gui2-2a",
                ("Executing", None),
                "04:12",
                true,
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-status-blocked",
        threshold: [0.07, 0.07],
        fixture: |lex| {
            StatusRow::el(
                lex.red,
                "e6b",
                ("Blocked", Some(lex.red)),
                "Requires agent",
                false,
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-receipt-commit",
        threshold: [0.05, 0.045],
        fixture: |lex| {
            ReceiptRow::value(
                (Icon::Commit, "commit"),
                "a3f9c21",
                Some("View"),
                true,
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-receipt-gates",
        threshold: [0.025, 0.02],
        fixture: |lex| {
            ReceiptRow::leds(
                (Icon::Gate, "gates"),
                &[true, true, false],
                Some("Log"),
                false,
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-rule-off",
        threshold: [0.08, 0.075],
        fixture: |lex| {
            ReceiptRow::rule(
                "git push",
                ModeBadge::off(lex.base, lex),
                true,
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "cap-severity-attention",
        threshold: [0.09, 0.1],
        fixture: |lex| {
            on_ground(
                lex.mantle,
                SeverityCap::el(
                    Severity::Attention,
                    None,
                    "plan \u{b7} two-phase \u{b7} cp named",
                    lex,
                ),
            )
        },
    },
    Gated {
        id: "cap-severity-positive",
        threshold: [0.05, 0.055],
        fixture: |lex| {
            on_ground(
                lex.mantle,
                SeverityCap::el(Severity::Good, None, "shipped \u{b7} toast", lex),
            )
        },
    },
    Gated {
        id: "cap-severity-danger",
        threshold: [0.04, 0.05],
        fixture: |lex| {
            on_ground(
                SeverityInk::of(Severity::Alert, lex).ground,
                SeverityCap::el(Severity::Alert, None, "Blocking", lex),
            )
        },
    },
    Gated {
        id: "ground-severity-danger",
        threshold: [0.08, 0.08],
        fixture: |lex| {
            rise_card(
                Severity::Alert,
                Icon::Alert,
                "Stuck \u{b7} 4 h",
                "e6b requires a decision: OTLP endpoint scope.",
                Action::new("Answer", ElementId(90)).accent(),
                lex,
            )
        },
    },
    Gated {
        id: "ground-severity-attention",
        threshold: [0.075, 0.075],
        fixture: |lex| {
            rise_card(
                Severity::Attention,
                Icon::Report,
                "Review",
                "theme-sweep is ready for review. 3 files. Gates passed.",
                Action::new("Review", ElementId(91)),
                lex,
            )
        },
    },
    Gated {
        id: "ground-severity-positive",
        threshold: [0.09, 0.09],
        fixture: |lex| {
            rise_card(
                Severity::Good,
                Icon::Done,
                "Done \u{b7} 2",
                "e10-fleet, tui2-sheet-overflow. Merged. 41 tests added.",
                Action::new("View reports", ElementId(92)),
                lex,
            )
        },
    },
    Gated {
        id: "card-plan",
        threshold: [0.085, 0.085],
        fixture: plan_card,
    },
    Gated {
        id: "card-ground-attention",
        threshold: [0.135, 0.12],
        fixture: |lex| {
            Card::severity(Severity::Attention, 15.0, 12.0, lex).child(
                Element::row()
                    .gap(10.0)
                    .center()
                    .child(SeverityCap::el(
                        Severity::Attention,
                        Some(Icon::Report),
                        "Review",
                        lex,
                    ))
                    .child(
                        Element::row().grow(1.0).child(
                            Element::text(body_voice(), lex.tx, "theme sweep waits on your eye")
                                .nowrap(),
                        ),
                    )
                    .child(
                        ActionRow::new(None, vec![Action::new("Review", ElementId(93))])
                            .el(medium_of(lex)),
                    ),
            )
        },
    },
    Gated {
        id: "card-toast-shipped",
        threshold: [0.06, 0.065],
        fixture: |lex| {
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
                    .nowrap(),
                )
                .child(Element::surface().height(8.0))
                .child(
                    ActionRow::new(
                        Some(Action::new("Claim gui2-2b", ElementId(94))),
                        vec![Action::new("Later", ElementId(95))],
                    )
                    .gap(8.0)
                    .el(medium_of(lex)),
                )
        },
    },
    Gated {
        id: "card-think-pin",
        threshold: [0.07, 0.07],
        fixture: |lex| {
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
                        .child(Element::text(Voice::KBD, lex.ov0, "task:design-canvas")),
                )
                .child(Element::surface().height(3.0))
                .child(Element::text(
                    body_voice(),
                    lex.sb1,
                    "Open gui2-2a: one view, two arrangements; the ids never change.",
                ))
        },
    },
    Gated {
        id: "card-signal",
        threshold: [0.055, 0.055],
        fixture: |lex| {
            Card::el(14.0, 11.0, lex)
                .child(SeverityCap::el(
                    Severity::Quiet,
                    None,
                    "Signal \u{b7} developer \u{b7} 2 d",
                    lex,
                ))
                .child(Element::surface().height(3.0))
                .child(
                    Element::text(
                        body_voice(),
                        lex.sb1,
                        "Retina window renders text at half size",
                    )
                    .nowrap(),
                )
                .child(Element::surface().height(8.0))
                .child(
                    ActionRow::new(
                        None,
                        vec![
                            Action::new("Promote", ElementId(96)).accent(),
                            Action::new("Research", ElementId(97)),
                            Action::new("Snooze", ElementId(98)),
                        ],
                    )
                    .gap(6.0)
                    .el(medium_of(lex)),
                )
        },
    },
    Gated {
        id: "card-ground-danger",
        threshold: [0.045, 0.05],
        fixture: |lex| {
            Card::severity(Severity::Alert, 14.0, 11.0, lex)
                .child(SeverityCap::el(Severity::Alert, None, "Blocking", lex))
                .child(Element::surface().height(3.0))
                .child(Element::text(body_voice(), lex.sb1, "Agent binary not found").nowrap())
                .child(Element::surface().height(8.0))
                .child(
                    ActionRow::new(
                        None,
                        vec![
                            Action::new("Install agent\u{2026}", ElementId(99)).accent(),
                            Action::new("How?", ElementId(100)),
                        ],
                    )
                    .gap(6.0)
                    .el(medium_of(lex)),
                )
        },
    },
    Gated {
        id: "block-diff",
        threshold: [0.16, 0.18],
        fixture: |lex| {
            DiffBlock::el(
                "diff \u{b7} theme/ink.rs",
                vec![
                    DiffRow::del(
                        "- pub fn ink(field: PaletteField) -> Color {",
                        lex.base,
                        lex,
                    ),
                    DiffRow::add(
                        "+ pub fn ink(field: PaletteField, palette: &Palette)",
                        lex.base,
                        lex,
                    ),
                    DiffRow::ctx("  receipt #241 \u{b7} rollback available", lex),
                ],
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-diff-del",
        threshold: [0.095, 0.065],
        fixture: |lex| {
            DiffRow::del(
                "- pub fn ink(field: PaletteField) -> Color {",
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-diff-add",
        threshold: [0.145, 0.115],
        fixture: |lex| {
            DiffRow::add(
                "+ pub fn ink(field: PaletteField, palette: &Palette)",
                lex.base,
                lex,
            )
        },
    },
    Gated {
        id: "row-diff-ctx",
        threshold: [0.06, 0.055],
        fixture: |lex| DiffRow::ctx("  receipt #241 \u{b7} rollback available", lex),
    },
    Gated {
        id: "block-receipts-ledger",
        threshold: [0.04, 0.035],
        fixture: |lex| {
            Ledger::el(
                "receipts",
                &[
                    ("commit", "a3f9c21", lex.sb1),
                    ("gates", "\u{2713}\u{2713}\u{2713}", lex.green),
                    ("cost", "2.07 USD", lex.sb0),
                ],
                lex,
            )
        },
    },
    Gated {
        id: "control-composer",
        threshold: [0.04, 0.04],
        fixture: |lex| {
            Composer::el(
                "\u{203a}",
                "allow, but stop if\u{2026}",
                ElementId(101),
                lex,
            )
        },
    },
    Gated {
        id: "control-search-field",
        threshold: [0.07, 0.06],
        fixture: |lex| {
            SearchField::el(
                "\u{2318}\u{21e7}F",
                "contrast floor",
                "6 results",
                ElementId(102),
                lex,
            )
        },
    },
    Gated {
        id: "row-cmdk-result",
        threshold: [0.115, 0.11],
        fixture: |lex| {
            CmdkRow::el(
                vec![
                    Element::row()
                        .center()
                        .child(Element::text(body_voice(), lex.tx, "Open\u{a0}").nowrap())
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
                            .nowrap(),
                        ),
                    Element::text(Voice::LEDGER_META, lex.peach, "1 action required").nowrap(),
                ],
                true,
                lex.base,
                ElementId(103),
                lex,
            )
        },
    },
    Gated {
        id: "settings-nav-active",
        threshold: [0.05, 0.045],
        fixture: |lex| {
            SettingsNav::el(
                Icon::Image,
                "Appearance",
                true,
                lex.base,
                ElementId(104),
                lex,
            )
        },
    },
    Gated {
        id: "settings-nav-idle",
        threshold: [0.03, 0.025],
        fixture: |lex| {
            SettingsNav::el(
                Icon::Session,
                "Motion",
                false,
                lex.base,
                ElementId(105),
                lex,
            )
        },
    },
    Gated {
        id: "star-pending",
        threshold: [0.155, 0.145],
        fixture: |lex| centered(Star::pending(lex)),
    },
    Gated {
        id: "star-ready",
        threshold: [0.16, 0.055],
        fixture: |lex| centered(Star::ready(lex)),
    },
    Gated {
        id: "star-executing",
        threshold: [0.085, 0.045],
        fixture: |lex| centered(Star::executing(lex)),
    },
    Gated {
        id: "star-stuck",
        threshold: [0.14, 0.16],
        fixture: |lex| centered(Star::stuck(lex)),
    },
    Gated {
        id: "star-blocked",
        threshold: [0.175, 0.17],
        fixture: |lex| centered(Star::blocked(lex)),
    },
    Gated {
        id: "star-done",
        threshold: [0.015, 0.11],
        fixture: |lex| centered(Star::done(lex)),
    },
    Gated {
        id: "star-hue-core",
        threshold: [0.01, 0.145],
        fixture: |lex| centered(Star::hue(lex.mauve)),
    },
    Gated {
        id: "star-hue-api",
        threshold: [0.01, 0.155],
        fixture: |lex| centered(Star::hue(lex.blue)),
    },
    Gated {
        id: "star-hue-ops",
        threshold: [0.01, 0.155],
        fixture: |lex| centered(Star::hue(lex.teal)),
    },
    Gated {
        id: "edge-dependency-done",
        threshold: [0.01, 0.01],
        fixture: |lex| centered(Edge::done(120.0, lex)),
    },
    Gated {
        id: "edge-dependency-ahead",
        threshold: [0.01, 0.01],
        fixture: |lex| centered(Edge::ahead(120.0, lex)),
    },
    Gated {
        id: "elevation-overlay",
        threshold: [0.06, 0.055],
        fixture: overlay_card,
    },
    Gated {
        id: "elevation-popover",
        threshold: [0.065, 0.055],
        fixture: popover_card,
    },
];

/// The volume's inline `font-size:12px` body run, worn by several
/// cards.
fn body_voice() -> Voice {
    Voice {
        px: 12.0,
        ..Voice::SLOT
    }
}

/// A fixture centered in its capture — the stars and edges are cut
/// with a 3px pad, the counter with 10.
fn centered(child: Element) -> Element {
    Element::surface().center().justify_center().child(child)
}

/// A fixture over the ground its capture was cut on — the caps live
/// inside cards, not on the page.
fn on_ground(ground: Srgba, child: Element) -> Element {
    Element::surface().fill(ground).child(child)
}

/// One of the Brief's rise cards (figure 17): severity ground, the
/// coloured cap with its kind mark, the body, one action.
fn rise_card(
    severity: Severity,
    icon: Icon,
    cap: &str,
    body: &str,
    action: Action,
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
                    .child(Element::text(text, lex.tx, body)),
            )
            .child(ActionRow::new(None, vec![action]).el(medium_of(lex))),
    )
}

/// The plan card (figure 2): peach cap, the moment title, the ordered
/// steps, Allow/Deny.
fn plan_card(lex: &Lexicon) -> Element {
    let title = Voice {
        px: 13.5,
        weight: 600,
        ..Voice::SLOT
    };
    let step = Voice {
        px: 11.5,
        line_height: 1.6,
        ..Voice::SLOT
    };
    let mut list = Element::surface();
    for (n, item) in [
        "Run the shots",
        "Diff \u{2014} strip rows only",
        "Bless matches, refuse the rest",
    ]
    .iter()
    .enumerate()
    {
        list = list.child(
            Element::row()
                .child(
                    Element::row()
                        .width(16.0)
                        .justify_end()
                        .child(Element::text(step, lex.sb1, format!("{}. ", n + 1)).nowrap()),
                )
                .child(Element::text(step, lex.sb1, *item).nowrap()),
        );
    }
    Card::el(17.0, 14.0, lex)
        .child(SeverityCap::el(
            Severity::Attention,
            None,
            "plan \u{b7} two-phase \u{b7} cp named",
            lex,
        ))
        .child(Element::surface().height(6.0))
        .child(Element::text(title, lex.tx, "Re-bless 12 goldens").nowrap())
        .child(Element::surface().height(6.0))
        .child(list)
        .child(Element::surface().height(10.0))
        .child(
            ActionRow::new(
                Some(Action::new("Allow", ElementId(110)).cap("A")),
                vec![Action::new("Deny", ElementId(111)).cap("D").negative()],
            )
            .gap(8.0)
            .el(medium_of(lex)),
        )
}

/// F1's "New project" overlay (figure 11's capture): the glass card at
/// the overlay lift, the moment title, the big empty-state slot, the
/// two neutral exits and the dismiss cap.
fn overlay_card(lex: &Lexicon) -> Element {
    let title = Voice {
        px: 16.0,
        weight: 600,
        ..Voice::SLOT
    };
    Card::overlay(lex.base, 26.0, 24.0, lex)
        .child(Element::text(title, lex.tx, "New project").nowrap())
        .child(Element::surface().height(4.0))
        .child(
            Element::text(
                Voice::CHIP,
                lex.ov0,
                "Select a folder, a repository, or start empty",
            )
            .nowrap(),
        )
        .child(Element::surface().height(18.0))
        .child(Slot::empty_state(
            Icon::Folder,
            "Drop a folder here",
            "Or click to browse",
            lex,
        ))
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
                    .el(medium_of(lex)),
                )
                .child(Element::row().grow(1.0))
                .child(SeverityCap::tinted(lex.ov0, None, "esc \u{b7} Cancel")),
        )
}

/// The ⌘K popover (figure 16's capture): the glass card at the top of
/// the scale, the chord header over its hairline, the chosen result
/// with its live badge, and the pending decision under it.
fn popover_card(lex: &Lexicon) -> Element {
    let glass = Lexicon::composite(
        lex.base,
        Srgba::new(lex.mantle.r, lex.mantle.g, lex.mantle.b, 0.84),
    );
    let chord = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    let header = Element::surface()
        .child(
            Element::row()
                .pad_x(10.0)
                .pad_y(7.0)
                .gap(10.0)
                .center()
                .child(Element::text(chord, lex.mauve, "\u{2318}K").nowrap())
                .child(
                    Element::row()
                        .center()
                        .child(Element::text(body_voice(), lex.tx, "thi").nowrap())
                        .child(Element::surface().width(1.5).height(14.0).fill(lex.mauve)),
                ),
        )
        .child(
            Element::surface()
                .height(1.0)
                .fill(Lexicon::composite(glass, lex.hairc)),
        );
    let chosen = CmdkRow::el(
        vec![
            Element::row()
                .center()
                .child(Element::text(body_voice(), lex.tx, "Open\u{a0}").nowrap())
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
                    .nowrap(),
                ),
            Element::text(Voice::LEDGER_META, lex.peach, "1 action required").nowrap(),
            Element::row().grow(1.0),
            Element::text(Voice::KBD, lex.ov0, "\u{21b5}").nowrap(),
        ],
        true,
        glass,
        ElementId(122),
        lex,
    );
    let pending = Element::row()
        .pad_x(12.0)
        .pad_y(7.0)
        .gap(10.0)
        .center()
        .child(
            Element::row()
                .gap(6.0)
                .center()
                .child(Element::mark(
                    Mark::Icon(Icon::Alert),
                    13,
                    Lexicon::ink_bytes(lex.sb0),
                ))
                .child(Element::text(body_voice(), lex.sb0, "Answer pending decision").nowrap()),
        )
        .child(Element::row().grow(1.0))
        .child(Element::text(Voice::KBD, lex.ov0, "\u{2318}\u{21b5}").nowrap());
    Card::popover(lex.base, 6.0, 4.0, lex)
        .child(header)
        .child(chosen)
        .child(pending)
}

/// Which medium a lexicon was resolved for — the fixtures that branch
/// on medium (the action faces) recover it from the ground they were
/// handed, so the table's fixture signature stays one function.
fn medium_of(lex: &Lexicon) -> Medium {
    if lex.base.r < 0.5 {
        Medium::Night
    } else {
        Medium::Day
    }
}

/// The volume's "Follow OS" preview: the sky and the chart split at
/// the diagonal seam (`linear-gradient(105deg, #14141f 49.8%, #f1f0f5
/// 50.2%)`) — drawn as its two halves, the seam's 15° lean conceded to
/// the tolerance band.
fn split_preview() -> Element {
    let sky = Srgba::new(
        0x14 as f32 / 255.0,
        0x14 as f32 / 255.0,
        0x1f as f32 / 255.0,
        1.0,
    );
    let chart = Srgba::new(
        0xf1 as f32 / 255.0,
        0xf0 as f32 / 255.0,
        0xf5 as f32 / 255.0,
        1.0,
    );
    Element::surface().fill(sky).child(
        Element::surface()
            .fill(chart)
            .radius(9.0)
            .pin_left(48.0)
            .pin_top(0.0)
            .pin_right(0.0)
            .pin_bottom(0.0),
    )
}

fn specimens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mocks/specimens")
}

fn capture(id: &str, medium: Medium) -> Pixmap {
    let path = specimens_dir().join(format!("{id}-{medium}.png"));
    Pixmap::load_png(&path).unwrap_or_else(|e| panic!("no capture at {} ({e})", path.display()))
}

fn dump(name: &str, pixmap: &Pixmap) {
    if std::env::var_os("ATOM_PARITY_DUMP").is_none() {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/atom-parity");
    std::fs::create_dir_all(&dir).expect("the dump dir");
    pixmap
        .save_png(dir.join(format!("{name}.png")))
        .expect("the dump writes");
}

/// Every gated atom, both media: the still at the capture's own pixels
/// is within its pinned share of the capture.
#[test]
fn every_atom_is_within_its_pinned_distance_of_its_capture() {
    let mask = Mask::none();
    let mut failures = Vec::new();
    for gated in GATED {
        for (at, medium) in Medium::ALL.into_iter().enumerate() {
            let lex = Lexicon::of(medium);
            let root = (gated.fixture)(&lex);
            let capture = capture(gated.id, medium);
            let still = atom_still(
                &root,
                &ElementState::default(),
                medium,
                capture.width(),
                capture.height(),
            )
            .unwrap_or_else(|e| panic!("{}-{medium}: the still refused: {e}", gated.id));
            let verdict = Parity::compare(&capture, &still, &mask, medium);
            dump(&format!("{}-{medium}", gated.id), &still);
            dump(
                &format!("{}-{medium}-diff", gated.id),
                &Parity::diff(&capture, &still, &mask, medium),
            );
            let share = verdict.share();
            if verdict.size_mismatch.is_some() || share > gated.threshold[at] {
                failures.push(format!(
                    "{}-{medium}: {:.2}% differ (pinned {:.2}%)",
                    gated.id,
                    share * 100.0,
                    gated.threshold[at] * 100.0
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "atoms out of parity:\n  {}",
        failures.join("\n  ")
    );
}

/// Every card fixture, at its capture's size — the surfaces the design prohibitions are
/// probed over.
fn card_fixtures(lex: &Lexicon) -> Vec<(&'static str, Element)> {
    GATED
        .iter()
        .filter(|gated| gated.id.starts_with("card-") || gated.id.starts_with("ground-severity"))
        .map(|gated| (gated.id, (gated.fixture)(lex)))
        .collect()
}

/// Design rules as structural probes over the emitted DrawList: no card draws a ~1px
/// stroke around its own perimeter (elevation is shadows and a rim, never a border),
/// and no card puts a narrow full-height fill at its left edge (severity is the whole
/// ground plus the cap, never a strip).
#[test]
fn no_card_wears_a_border_or_a_left_strip() {
    use magistr_gui2::{ElementLayout, TextEngine};
    use qs_gpu::frame::{DrawList, PrimKind};

    let lex = Lexicon::of(Medium::Night);
    for (id, card) in card_fixtures(&lex) {
        let capture = capture(id, Medium::Night);
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (capture.width() as f32, capture.height() as f32);
        let mut text = TextEngine::embedded();
        let layout = ElementLayout::compute(&card, w, h, &mut text).expect("layout");
        let mut list = DrawList::default();
        text.begin_frame();
        list.reset([capture.width(), capture.height()], lex.base, 1);
        emit(
            &card,
            &layout,
            &ElementState::default(),
            &mut text,
            &mut list,
            0..card.count(),
        );
        list.end_batch(None, true);
        list.finish();
        for instance in &list.instances {
            let [x, y, iw, ih] = instance.rect;
            let covers_root = x <= 1.0 && y <= 1.0 && iw >= w - 2.0 && ih >= h - 2.0;
            if PrimKind::from_raw(instance.kind) == Some(PrimKind::Stroke) {
                assert!(
                    !covers_root,
                    "{id}: a stroke rides the card's own perimeter — elevation is never a border"
                );
            }
            if PrimKind::from_raw(instance.kind) == Some(PrimKind::Rect) {
                let strip = x <= 1.0 && iw <= 6.0 && ih >= h - 2.0;
                assert!(
                    !strip,
                    "{id}: a narrow full-height fill sits at the left edge — severity is never a strip"
                );
            }
        }
    }
}

/// A state transition is a pure function of the folded state: the same
/// fold gives byte-identical pixels, a different fold different ones —
/// no clock is read anywhere on the way, so reduced motion is
/// structurally the end frame.
#[test]
fn interaction_states_are_pure_functions_of_the_fold() {
    use magistr_cockpit::cockpit::Msg;
    use magistr_cockpit::input::{MouseEvent, MouseKind};
    use magistr_gui2::{ElementLayout, TextEngine};

    let lex = Lexicon::of(Medium::Night);
    let chip = || NavChip::el("gui2-2a · execute", ElementId(1), &lex);
    let hovered = || {
        let mut text = TextEngine::embedded();
        let tree = chip();
        let layout = ElementLayout::compute(&tree, 143.0, 24.0, &mut text).expect("layout");
        let mut state = ElementState::default();
        let over = Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 0, 0).at_px(10, 10),
            target: None,
        };
        assert!(state.fold(&over, &tree, &layout));
        state
    };
    let rest = atom_still(&chip(), &ElementState::default(), Medium::Night, 143, 24)
        .expect("the rest still");
    let once = atom_still(&chip(), &hovered(), Medium::Night, 143, 24).expect("the hover still");
    let twice = atom_still(&chip(), &hovered(), Medium::Night, 143, 24).expect("the hover still");
    assert_eq!(once.data(), twice.data(), "same fold, same pixels");
    assert_ne!(once.data(), rest.data(), "the hover face is a real face");
}
