use taffy::NodeId;

use super::*;

#[test]
fn wraps_the_engine_message() {
    let error: LayoutError = TaffyError::InvalidParentNode(NodeId::from(3usize)).into();
    assert!(error.message().contains("3"), "{}", error.message());
    assert!(error.to_string().starts_with("layout failed: "));
    let boxed: Box<dyn std::error::Error> = Box::new(error);
    assert!(boxed.source().is_none());
}
