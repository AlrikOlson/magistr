use super::*;
use crate::layout::{Frame, LayoutTree, Length};
use crate::pixel::Icon;
use crate::render::{Color, Style};

fn icon(col: u16) -> Mark {
    Mark::Icon {
        icon: Icon::Done,
        col,
        cols: 1,
        ink: Color::Rgb(1, 2, 3),
    }
}

#[test]
fn marks_come_back_with_the_rect_of_the_node_that_carries_them() {
    let root = Node::column()
        .child(Node::text("head").height(Length::Cells(1)))
        .child(
            Node::row()
                .height(Length::Cells(1))
                .child(Node::text("  a").mark(icon(0)).shrink(0.0))
                .child(Node::text(" b").mark(icon(0)).mark(icon(1)).shrink(0.0)),
        );
    let resolved = LayoutTree::new().compute(&root, 10, 3).expect("layout");
    let found = marks(&root, &resolved);
    assert_eq!(
        found,
        vec![
            (Rect::new(0, 1, 3, 1), icon(0)),
            (Rect::new(3, 1, 2, 1), icon(0)),
            (Rect::new(3, 1, 2, 1), icon(1)),
        ]
    );
}

#[test]
fn a_drawn_frame_is_a_panel_mark_with_its_title_span_and_a_spelled_one_is_not() {
    let border = Style::new().fg(Color::Rgb(9, 9, 9));
    let drawn = Node::column()
        .frame(Frame::new(border.clone()).title("plan").drawn(true))
        .child(Node::text("x").height(Length::Cells(1)));
    let resolved = LayoutTree::new().compute(&drawn, 12, 3).expect("layout");
    assert_eq!(
        marks(&drawn, &resolved),
        vec![(
            Rect::new(0, 0, 12, 3),
            Mark::Panel {
                border: Color::Rgb(9, 9, 9),
                title: Some((2, 6)),
            }
        )]
    );
    let spelled = Node::column()
        .frame(Frame::new(border))
        .child(Node::text("x").height(Length::Cells(1)));
    let resolved = LayoutTree::new().compute(&spelled, 12, 3).expect("layout");
    assert!(marks(&spelled, &resolved).is_empty());
}

#[test]
fn a_node_that_landed_nowhere_carries_no_mark() {
    let root = Node::column()
        .height(Length::Cells(1))
        .viewport()
        .child(Node::text("shown").mark(icon(0)).height(Length::Cells(1)))
        .child(Node::text("").mark(icon(0)).height(Length::Cells(0)));
    let resolved = LayoutTree::new().compute(&root, 8, 1).expect("layout");
    assert_eq!(marks(&root, &resolved).len(), 1);
}
