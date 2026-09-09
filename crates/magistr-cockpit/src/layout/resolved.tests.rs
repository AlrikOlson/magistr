use super::*;

fn placed(id: Option<u32>, depth: u16, rect: Rect) -> Placed {
    Placed {
        id: id.map(WidgetId::new),
        depth,
        layer: 0,
        rect,
        px: PxRect::new(
            f32::from(rect.x),
            f32::from(rect.y),
            f32::from(rect.width),
            f32::from(rect.height),
        ),
        content: Extent {
            width: rect.width,
            height: rect.height,
        },
    }
}

#[test]
fn starts_empty() {
    let resolved = Resolved::default();
    assert!(resolved.is_empty());
    assert_eq!(resolved.len(), 0);
    assert_eq!(resolved.rect(WidgetId::new(1)), None);
}

#[test]
fn keeps_insertion_order_and_finds_rects_by_id() {
    let mut resolved = Resolved::default();
    resolved.push(placed(None, 0, Rect::new(0, 0, 10, 10)));
    resolved.push(placed(Some(1), 1, Rect::new(0, 0, 5, 10)));
    resolved.push(placed(Some(2), 1, Rect::new(5, 0, 5, 10)));
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved.placed()[0].depth, 0);
    assert_eq!(
        resolved.rect(WidgetId::new(2)),
        Some(Rect::new(5, 0, 5, 10))
    );
    assert_eq!(resolved.rect(WidgetId::new(3)), None);
    assert_eq!(
        resolved.content_extent(WidgetId::new(2)),
        Some(Extent {
            width: 5,
            height: 10
        })
    );
    assert_eq!(resolved.content_extent(WidgetId::new(3)), None);
}

#[test]
fn a_duplicate_id_resolves_to_the_first_in_tree_order() {
    let mut resolved = Resolved::default();
    resolved.push(placed(Some(1), 0, Rect::new(0, 0, 3, 3)));
    resolved.push(placed(Some(1), 1, Rect::new(1, 1, 1, 1)));
    assert_eq!(resolved.rect(WidgetId::new(1)), Some(Rect::new(0, 0, 3, 3)));
}

#[test]
fn the_max_depth_is_the_stride_a_stacking_order_needs() {
    let mut resolved = Resolved::default();
    assert_eq!(resolved.max_depth(), 0, "nothing placed");
    resolved.push(placed(Some(1), 0, Rect::new(0, 0, 10, 10)));
    resolved.push(placed(Some(2), 3, Rect::new(0, 0, 5, 5)));
    resolved.push(placed(Some(3), 1, Rect::new(5, 0, 5, 5)));
    assert_eq!(resolved.max_depth(), 3);
}
