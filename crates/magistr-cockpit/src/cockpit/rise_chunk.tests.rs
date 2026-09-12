use super::*;
use crate::cockpit::{Msg, Size, Surface, update};
use crate::engine::EngineEvent;

/// A window on `here` — the chunk the rail names, executing — with a
/// pending `next` and a done `shipped` beside it.
fn window() -> Model {
    let mut m = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    m.place = Place::Session;
    let chunk = |id: &str, status: &str| serde_json::json!({"id": id, "name": id, "title": id, "status": status, "priority": 200});
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
            chunks: crate::harness::fixtures::chunks_from(vec![
                chunk("here", "in_progress"),
                chunk("next", "pending"),
                chunk("shipped", "done"),
            ]),
            ..magistr_engine::NativeSnapshot::default()
        })),
    );
    m
}

fn selected(model: &Model) -> Option<&str> {
    model.roadmap().selected().map(|chunk| chunk.id.as_str())
}

/// The rise is the same for every status — the executing chunk this
/// session is on, which the open-chunk fold would route back to the
/// Session, rises to the Sky like a pending or a done one: the chip's
/// point is the star, framed.
#[test]
fn every_chunk_rises_to_the_sky_framed() {
    for id in ["here", "next", "shipped"] {
        let mut w = window();
        assert!(rise_to_chunk(&mut w, id), "{id}");
        assert_eq!(w.place(), Some(Place::Sky), "{id} rises to the Sky");
        assert_eq!(selected(&w), Some(id), "{id} is framed");
    }
}

/// A chunk the roadmap does not hold moves nothing: the camera stays at
/// the Session and nothing is selected.
#[test]
fn an_unknown_chunk_moves_nothing() {
    let mut w = window();
    assert!(!rise_to_chunk(&mut w, "nowhere"));
    assert_eq!(w.place(), Some(Place::Session));
    assert_eq!(selected(&w), None);
}
