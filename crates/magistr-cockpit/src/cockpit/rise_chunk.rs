//! The rise to a chunk's star — the identity
//! band's chip goes up, card framed.

use super::model::Model;
use super::place::Place;

/// Navigate from a session's chunk chip to its roadmap node.
/// The roadmap camera focuses the node and opens its card.
pub fn rise_to_chunk(model: &mut Model, id: &str) -> bool {
    let (now, animate) = (model.now(), model.theme.motion().is_full());
    if !model.roadmap.select(id, now, animate) {
        return false;
    }
    model.place = Place::Sky;
    true
}

#[cfg(test)]
#[path = "rise_chunk.tests.rs"]
mod tests;
