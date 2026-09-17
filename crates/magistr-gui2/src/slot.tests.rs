use super::*;
use crate::action::Action;
use crate::action_row::ActionRow;
use crate::budget_meter::BudgetMeter;
use crate::crumb::Crumb;
use crate::crumb_stop::CrumbStop;
use crate::element_id::ElementId;
use crate::medium::Medium;
use crate::nav_chip::NavChip;
use crate::pill::DataPill;
use crate::segments::Segments;
use crate::switch::Switch;
use crate::theme_card::ThemeCard;

fn dashes(root: &Element) -> usize {
    let mut n = 0;
    root.each(&mut |_, element| {
        if element.dashed {
            n += 1;
        }
    });
    n
}

/// The slot is inert like the data pill: dashed marks a place, never a
/// control.
#[test]
fn a_slot_is_dashed_and_structurally_inert() {
    let lex = Lexicon::of(Medium::Night);
    let slot = Slot::el("Drop a folder here", &lex);
    assert_eq!(dashes(&slot), 1);
    slot.each(&mut |_, element| {
        assert!(element.id.is_none());
        assert!(!element.focusable);
    });
}

/// exclusivity, swept over every atom this layer ships: only the slot draws dashed. A
/// dashed anything-else would read as a drop target, which is exactly the confusion the
/// law exists to prevent.
#[test]
fn only_the_slot_draws_dashed() {
    let lex = Lexicon::of(Medium::Night);
    let atoms: Vec<Element> = vec![
        DataPill::live("Executing · 04:12", &lex),
        DataPill::ok("gates ✓✓✓", &lex),
        DataPill::num("1.84 / 12.00", &lex),
        NavChip::el("gui2-2a · execute", ElementId(1), &lex),
        Crumb::el(&[CrumbStop::here("projects", None)], &lex),
        ActionRow::new(
            Some(Action::new("Approve & merge", ElementId(2)).cap("A")),
            vec![Action::new("Deny", ElementId(3)).negative()],
        )
        .el(Medium::Night),
        Segments::el(&["The sky", "The chart"], 0, ElementId(10), &lex),
        Switch::el(true, ElementId(20), &lex),
        ThemeCard::el(
            "The sky",
            true,
            ThemeCard::sky_preview(&lex),
            ElementId(30),
            &lex,
        ),
        BudgetMeter::el(
            "Budget",
            "1.84 / 12.00",
            0.15,
            Some(0.83),
            None,
            None,
            220.0,
            &lex,
        ),
    ];
    for atom in &atoms {
        assert_eq!(dashes(atom), 0, "no atom but the slot dashes");
    }
    assert_eq!(dashes(&Slot::el("Drop a folder here", &lex)), 1);
}
