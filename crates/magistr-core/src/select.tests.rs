use super::*;

#[test]
fn select_item_constructors() {
    let item = SelectItem::new("Label", "value");
    assert_eq!(item.label, "Label");
    assert_eq!(item.value, "value");
    assert!(item.description.is_none());

    let desc = SelectItem::with_description("Label", "value", "A description");
    assert_eq!(desc.description.as_deref(), Some("A description"));

    let simple = SelectItem::simple("both");
    assert_eq!(simple.label, "both");
    assert_eq!(simple.value, "both");
}

#[test]
fn select_list_navigation_wraps() {
    let items = vec![
        SelectItem::simple("a"),
        SelectItem::simple("b"),
        SelectItem::simple("c"),
    ];
    let mut list = SelectList::new("Pick one", items);

    assert_eq!(list.selected_index(), 0);
    assert_eq!(list.selected_value(), Some("a"));

    list.move_down();
    assert_eq!(list.selected_index(), 1);
    assert_eq!(list.previous_selected(), Some(0));
    assert!(list.selection_changed);

    list.clear_selection_changed();
    assert!(!list.selection_changed);

    list.move_down();
    assert_eq!(list.selected_index(), 2);

    // Wraps to first item
    list.move_down();
    assert_eq!(list.selected_index(), 0);

    // Wraps to last item from top
    list.move_up();
    assert_eq!(list.selected_index(), 2);

    list.move_up();
    assert_eq!(list.selected_index(), 1);
}

#[test]
fn select_list_by_value() {
    let items = vec![
        SelectItem::new("First", "1"),
        SelectItem::new("Second", "2"),
        SelectItem::new("Third", "3"),
    ];
    let mut list = SelectList::new("test", items);

    assert!(list.set_selected_value("2"));
    assert_eq!(list.selected_index(), 1);

    assert!(!list.set_selected_value("nonexistent"));
    assert_eq!(list.selected_index(), 1); // unchanged
}

#[test]
fn select_list_by_index() {
    let items = vec![SelectItem::simple("a"), SelectItem::simple("b")];
    let mut list = SelectList::new("test", items);

    assert!(list.select_by_index(1));
    assert_eq!(list.selected_index(), 1);

    assert!(!list.select_by_index(5)); // out of bounds
    assert_eq!(list.selected_index(), 1); // unchanged
}

#[test]
fn empty_list() {
    let list = SelectList::new("empty", vec![]);
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
    assert!(list.selected_item().is_none());
    assert!(list.selected_value().is_none());
}
