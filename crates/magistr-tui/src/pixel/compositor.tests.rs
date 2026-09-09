use tiny_skia::{Color, PixmapMut};

use super::*;
use crate::graphics::Passthrough;
use magistr_cockpit::pixel::CellSize;

/// The APC commands in `bytes`: control keys and payload of each.
fn commands(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8(bytes.to_vec()).expect("ascii");
    text.split("\x1b_G")
        .skip(1)
        .map(|rest| {
            let end = rest.find("\x1b\\").expect("APC terminator");
            let (keys, payload) = rest[..end].split_once(';').unwrap_or((&rest[..end], ""));
            (keys.to_owned(), payload.to_owned())
        })
        .collect()
}

fn has_key(keys: &str, key: &str) -> bool {
    keys.split(',').any(|k| k == key)
}

/// The keys of every command whose action is `action`.
fn with_action<'a>(cmds: &'a [(String, String)], action: &str) -> Vec<&'a str> {
    cmds.iter()
        .map(|(keys, _)| keys.as_str())
        .filter(|keys| has_key(keys, action))
        .collect()
}

fn compositor(compression: Compression) -> Compositor {
    Compositor::new(Kitty::new(Passthrough::Direct), compression, IdSpace::FIRST)
}

fn surface(rect: Rect) -> PixelSurface {
    let mut surface = PixelSurface::new(rect, CellSize::new(2, 2));
    surface.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::from_rgba8(1, 2, 3, 255)));
    surface
}

fn frame(
    compositor: &mut Compositor,
    encoder: &mut Encoder,
    shows: &mut [(u32, &mut PixelSurface)],
) -> Vec<u8> {
    for (id, surface) in shows.iter_mut() {
        compositor.show(WidgetId::new(*id), surface);
    }
    let mut out = Vec::new();
    compositor.finish(encoder, &mut out);
    out
}

#[test]
fn the_first_frame_transmits_moves_the_cursor_and_places_under_the_text() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(3, 4, 5, 2));
    let out = frame(&mut compositor, &mut encoder, &mut [(7, &mut s)]);
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 2, "{}", String::from_utf8_lossy(&out));
    for key in ["a=t", "f=24", "s=10", "v=4", "i=1", "m=0"] {
        assert!(has_key(&cmds[0].0, key), "{key} in {}", cmds[0].0);
    }
    assert!(!has_key(&cmds[0].0, "o=z"));
    assert_eq!(cmds[1].0, "a=p,q=2,i=1,p=1,c=5,r=2,z=-1,C=1");
    let text = String::from_utf8_lossy(&out);
    let cup = text
        .find("\x1b[5;4H")
        .expect("cursor moved to row 5, column 4");
    let place = text.find("a=p").expect("place");
    assert!(cup < place, "the cursor move precedes the placement");
    assert!(!s.is_dirty(), "shipped pixels are clean");
}

#[test]
fn an_unchanged_surface_costs_zero_bytes() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
}

#[test]
fn a_one_pixel_change_transmits_the_idle_id_places_it_and_deletes_the_old_placement() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    s.paint(&|pixmap: &mut PixmapMut<'_>| {
        pixmap.pixels_mut()[0] =
            tiny_skia::PremultipliedColorU8::from_rgba(9, 9, 9, 255).expect("premultiplied");
    });
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    let transmits = with_action(&cmds, "a=t");
    let places = with_action(&cmds, "a=p");
    let deletes = with_action(&cmds, "a=d");
    assert_eq!(transmits.len(), 1);
    assert!(has_key(transmits[0], "i=2"), "{}", transmits[0]);
    assert_eq!(places, ["a=p,q=2,i=2,p=1,c=2,r=2,z=-1,C=1"]);
    assert_eq!(deletes, ["a=d,q=2,d=i,i=1,p=1"]);
    let text = String::from_utf8_lossy(&out);
    assert!(text.find("a=p").unwrap() < text.find("a=d").unwrap());
    // A third dirty frame flips back to the first id.
    s.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::WHITE));
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    assert!(has_key(with_action(&cmds, "a=t")[0], "i=1"));
    assert_eq!(with_action(&cmds, "a=d"), ["a=d,q=2,d=i,i=2,p=1"]);
}

#[test]
fn a_move_re_places_without_transmitting() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    s.set_rect(Rect::new(10, 1, 2, 2));
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].0, "a=p,q=2,i=1,p=1,c=2,r=2,z=-1,C=1");
    assert!(String::from_utf8_lossy(&out).starts_with("\x1b[2;11H"));
}

#[test]
fn leaving_the_frame_deletes_the_placement_once_and_a_comeback_only_places() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let out = frame(&mut compositor, &mut encoder, &mut []);
    assert_eq!(commands(&out).len(), 1);
    assert_eq!(commands(&out)[0].0, "a=d,q=2,d=i,i=1,p=1");
    let out = frame(&mut compositor, &mut encoder, &mut []);
    assert!(out.is_empty(), "the delete is not repeated");
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 1);
    assert!(has_key(&cmds[0].0, "a=p"));
}

#[test]
fn an_empty_rect_is_not_shown() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    s.set_rect(Rect::new(0, 0, 0, 2));
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    assert_eq!(with_action(&commands(&out), "a=d").len(), 1);
    assert!(with_action(&commands(&out), "a=p").is_empty());
}

#[test]
fn every_surface_gets_its_own_ids_and_placement() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut a = surface(Rect::new(0, 0, 1, 1));
    let mut b = surface(Rect::new(5, 5, 1, 1));
    let out = frame(
        &mut compositor,
        &mut encoder,
        &mut [(1, &mut a), (2, &mut b)],
    );
    let cmds = commands(&out);
    let places = with_action(&cmds, "a=p");
    assert_eq!(
        places,
        [
            "a=p,q=2,i=1,p=1,c=1,r=1,z=-1,C=1",
            "a=p,q=2,i=3,p=2,c=1,r=1,z=-1,C=1"
        ]
    );
    a.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::WHITE));
    b.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::WHITE));
    let out = frame(
        &mut compositor,
        &mut encoder,
        &mut [(1, &mut a), (2, &mut b)],
    );
    let cmds = commands(&out);
    assert_eq!(
        with_action(&cmds, "a=d"),
        ["a=d,q=2,d=i,i=1,p=1", "a=d,q=2,d=i,i=3,p=2"]
    );
}

#[test]
fn an_alpha_surface_transmits_f32() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = PixelSurface::with_alpha(Rect::new(0, 0, 1, 1), CellSize::new(2, 2));
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    assert!(has_key(with_action(&commands(&out), "a=t")[0], "f=32"));
}

#[test]
fn zlib_marks_the_transmit_o_z() {
    let mut compositor = compositor(Compression::Zlib(6));
    assert_eq!(compositor.compression(), Compression::Zlib(6));
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 8, 8));
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    let transmit = with_action(&cmds, "a=t")[0];
    assert!(has_key(transmit, "o=z"), "{transmit}");
    assert!(has_key(transmit, "f=24"));
}

#[test]
fn after_forgetting_placements_the_next_frame_places_again_without_pixels() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut s = surface(Rect::new(0, 0, 2, 2));
    frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    compositor.forget_placements();
    let out = frame(&mut compositor, &mut encoder, &mut [(1, &mut s)]);
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].0, "a=p,q=2,i=1,p=1,c=2,r=2,z=-1,C=1");
    // Nothing was on screen, so nothing is deleted when it leaves either.
    compositor.forget_placements();
    let out = frame(&mut compositor, &mut encoder, &mut []);
    assert!(out.is_empty());
}

#[test]
fn release_frees_every_image_and_starts_over() {
    let mut compositor = compositor(Compression::None);
    let mut encoder = Encoder::new();
    let mut a = surface(Rect::new(0, 0, 1, 1));
    let mut b = surface(Rect::new(1, 1, 1, 1));
    frame(
        &mut compositor,
        &mut encoder,
        &mut [(1, &mut a), (2, &mut b)],
    );
    let mut out = Vec::new();
    compositor.release(&mut out);
    let cmds = commands(&out);
    let keys: Vec<&str> = cmds.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            "a=d,q=2,d=I,i=1",
            "a=d,q=2,d=I,i=2",
            "a=d,q=2,d=I,i=3",
            "a=d,q=2,d=I,i=4"
        ]
    );
    // The surfaces are clean but the terminal forgot them: showing one
    // again transmits under a fresh slot.
    let out = frame(&mut compositor, &mut encoder, &mut [(2, &mut b)]);
    let cmds = commands(&out);
    assert!(
        b.is_dirty()
            || !with_action(&cmds, "a=t").is_empty()
            || !with_action(&cmds, "a=p").is_empty()
    );
}

/// The image ids every command in `out` names.
fn image_ids(out: &[u8]) -> Vec<u32> {
    commands(out)
        .iter()
        .flat_map(|(keys, _)| {
            keys.split(',')
                .filter_map(|k| k.strip_prefix("i=").and_then(|n| n.parse().ok()))
                .collect::<Vec<u32>>()
        })
        .collect()
}

#[test]
fn two_sessions_in_their_own_spaces_never_name_the_same_image_id() {
    // Two cockpits behind one terminal image store — tmux panes under
    // passthrough — each allocate from slot 0 in their own first-show
    // order: the sparkline first in one, the tech tree first in the
    // other. In the same space the second's transmit would replace the
    // first's pixels under its placement; in their own spaces the ids
    // are disjoint.
    let mut a = Compositor::new(
        Kitty::new(Passthrough::Direct),
        Compression::None,
        IdSpace::new(1),
    );
    let mut b = Compositor::new(
        Kitty::new(Passthrough::Direct),
        Compression::None,
        IdSpace::new(2),
    );
    let mut encoder = Encoder::new();
    let (mut spark, mut tree) = (
        surface(Rect::new(0, 0, 3, 1)),
        surface(Rect::new(0, 2, 8, 6)),
    );
    let out_a = frame(&mut a, &mut encoder, &mut [(1, &mut spark), (2, &mut tree)]);
    let (mut spark, mut tree) = (
        surface(Rect::new(0, 0, 3, 1)),
        surface(Rect::new(0, 2, 8, 6)),
    );
    let out_b = frame(&mut b, &mut encoder, &mut [(2, &mut tree), (1, &mut spark)]);
    let (ids_a, ids_b) = (image_ids(&out_a), image_ids(&out_b));
    assert_eq!(
        ids_a,
        [4097, 4099, 4097, 4099],
        "transmit, transmit, place, place"
    );
    assert_eq!(ids_b, [8193, 8195, 8193, 8195]);
    assert!(ids_a.iter().all(|id| !ids_b.contains(id)));
    // Repaints flip to the idle ids, still inside each space.
    spark.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Color::WHITE));
    let out_b = frame(&mut b, &mut encoder, &mut [(2, &mut tree), (1, &mut spark)]);
    assert!(
        image_ids(&out_b)
            .iter()
            .all(|id| (8193..8193 + 4).contains(id))
    );
    assert!(image_ids(&out_b).iter().all(|id| !ids_a.contains(id)));
}
