use super::*;

#[test]
fn the_default_tab_is_the_turn() {
    assert_eq!(StageTab::default(), StageTab::Turn);
}

#[test]
fn next_cycles_through_every_tab_and_wraps() {
    let mut tab = StageTab::Home;
    let mut seen = Vec::new();
    for _ in 0..StageTab::ALL.len() {
        seen.push(tab);
        tab = tab.next();
    }
    assert_eq!(seen, StageTab::ALL);
    assert_eq!(tab, StageTab::Home, "wraps back to the start");
}

#[test]
fn prev_undoes_next_for_every_tab() {
    for tab in StageTab::ALL {
        assert_eq!(tab.next().prev(), tab);
        assert_eq!(tab.prev().next(), tab);
    }
}

#[test]
fn labels_are_distinct() {
    let labels: std::collections::HashSet<_> = StageTab::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(labels.len(), StageTab::ALL.len());
}
