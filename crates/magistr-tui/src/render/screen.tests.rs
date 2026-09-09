use tiny_skia::{Color as Rgba, PixmapMut};

use super::*;
use crate::graphics::{Passthrough, Protocol};
use crate::pixel::IdSpace;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::CellSize;
use magistr_cockpit::render::{Color, Style};

const BSU: &[u8] = b"\x1b[?2026h";
const ESU: &[u8] = b"\x1b[?2026l";

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

fn draw_page(screen: &mut Screen, marker: &str) {
    let (w, h) = screen.size();
    let buffer = screen.buffer();
    for row in 0..h {
        let line: String = (0..w)
            .map(|c| if (c + row) % 7 == 0 { '#' } else { '.' })
            .collect();
        buffer.put_str(0, row, &line, &Style::new());
    }
    buffer.put_str(2, 1, marker, &Style::new().fg(Color::Indexed(4)));
}

#[test]
fn the_first_frame_clears_and_draws_everything() {
    let mut screen = Screen::new(10, 3);
    draw_page(&mut screen, "hi");
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, BSU), 1);
    assert_eq!(count(&out, ESU), 1);
    assert_eq!(count(&out, b"\x1b[2J"), 1);
    assert_eq!(count(&out, b"hi"), 1);
}

#[test]
fn an_unchanged_frame_emits_only_the_bracket() {
    let mut screen = Screen::new(40, 12);
    draw_page(&mut screen, "same");
    screen.present(&mut Vec::new());
    draw_page(&mut screen, "same");
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn a_one_cell_change_on_a_settled_screen_costs_under_64_bytes() {
    let mut screen = Screen::new(120, 40);
    draw_page(&mut screen, "a");
    screen.present(&mut Vec::new());
    draw_page(&mut screen, "b");
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, BSU), 1);
    assert_eq!(count(&out, ESU), 1);
    assert!(
        out.len() < 64,
        "{} bytes: {:?}",
        out.len(),
        String::from_utf8_lossy(&out)
    );
    assert_eq!(count(&out, b"b"), 1);
}

#[test]
fn the_back_buffer_starts_blank_every_frame() {
    let mut screen = Screen::new(5, 1);
    screen.buffer().put_str(0, 0, "abc", &Style::new());
    screen.present(&mut Vec::new());
    assert!(screen.buffer().cells().iter().all(|c| c.symbol == " "));
    // Not drawing again erases the text on the terminal.
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"   "), 1);
}

#[test]
fn resize_forces_a_clear_and_full_redraw() {
    let mut screen = Screen::new(5, 1);
    screen.buffer().put_str(0, 0, "abc", &Style::new());
    screen.present(&mut Vec::new());
    screen.resize(6, 2);
    assert_eq!(screen.size(), (6, 2));
    screen.buffer().put_str(0, 1, "abc", &Style::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 1);
    assert_eq!(count(&out, b"\x1b[2;1Habc"), 1);
}

#[test]
fn invalidate_redraws_without_resizing() {
    let mut screen = Screen::new(5, 1);
    screen.buffer().put_str(0, 0, "abc", &Style::new());
    screen.present(&mut Vec::new());
    screen.invalidate();
    screen.buffer().put_str(0, 0, "abc", &Style::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 1);
    assert_eq!(count(&out, b"abc"), 1);
}

#[test]
fn a_zero_sized_screen_presents_the_bracket_only() {
    let mut screen = Screen::new(0, 0);
    screen.buffer().put_str(0, 0, "abc", &Style::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(out, [BSU, b"\x1b[2J".as_slice(), ESU].concat());
}

fn kitty_screen() -> Screen {
    let mut screen = Screen::new(20, 5);
    screen.enable_graphics(
        Protocol::Kitty(Passthrough::Direct),
        Compression::None,
        IdSpace::FIRST,
    );
    screen
}

fn painted(rect: Rect) -> PixelSurface {
    let mut surface = PixelSurface::new(rect, CellSize::new(2, 2));
    surface.paint(&|pixmap: &mut PixmapMut<'_>| pixmap.fill(Rgba::from_rgba8(1, 2, 3, 255)));
    surface
}

#[test]
fn without_graphics_a_surface_is_a_no_op() {
    let mut screen = Screen::new(20, 5);
    assert!(!screen.has_graphics());
    let mut surface = painted(Rect::new(1, 1, 3, 2));
    screen.present(&mut Vec::new());
    screen.surface(WidgetId::new(1), &mut surface);
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
    assert!(surface.is_dirty(), "nothing shipped the pixels");
    screen.release(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn a_surface_rides_inside_the_frames_bracket_after_the_text() {
    let mut screen = kitty_screen();
    assert!(screen.has_graphics());
    screen.buffer().put_str(0, 0, "title", &Style::new());
    let mut surface = painted(Rect::new(2, 2, 4, 2));
    screen.surface(WidgetId::new(1), &mut surface);
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, BSU), 1);
    assert_eq!(count(&out, ESU), 1);
    let text = String::from_utf8_lossy(&out);
    let title = text.find("title").expect("text");
    let transmit = text.find("a=t").expect("transmit");
    let place = text.find("a=p").expect("place");
    let esu = text.find("\x1b[?2026l").expect("ESU");
    assert!(
        title < transmit && transmit < place && place < esu,
        "{text:?}"
    );
    assert!(text.contains("\x1b[3;3H\x1b_Ga=p,q=2,i=1,p=1,c=4,r=2,z=-1,C=1\x1b\\"));
    assert!(!surface.is_dirty());
    // Steady state: the bare bracket.
    screen.buffer().put_str(0, 0, "title", &Style::new());
    screen.surface(WidgetId::new(1), &mut surface);
    out.clear();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn a_resize_clears_and_places_every_surface_again() {
    let mut screen = kitty_screen();
    let mut surface = painted(Rect::new(0, 0, 2, 1));
    screen.surface(WidgetId::new(1), &mut surface);
    screen.present(&mut Vec::new());
    screen.resize(30, 6);
    screen.surface(WidgetId::new(1), &mut surface);
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 1);
    assert_eq!(count(&out, b"a=t"), 0, "the image data survived the clear");
    assert_eq!(count(&out, b"a=p"), 1);
    // The same after an invalidate.
    screen.invalidate();
    screen.surface(WidgetId::new(1), &mut surface);
    out.clear();
    screen.present(&mut out);
    assert_eq!(count(&out, b"a=p"), 1);
}

#[test]
fn a_surface_left_out_of_a_frame_is_deleted_once() {
    let mut screen = kitty_screen();
    let mut surface = painted(Rect::new(0, 0, 2, 1));
    screen.surface(WidgetId::new(1), &mut surface);
    screen.present(&mut Vec::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(
        out,
        [BSU, b"\x1b_Ga=d,q=2,d=i,i=1,p=1\x1b\\".as_slice(), ESU].concat()
    );
    out.clear();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn release_frees_the_images() {
    let mut screen = kitty_screen();
    let mut surface = painted(Rect::new(0, 0, 2, 1));
    screen.surface(WidgetId::new(1), &mut surface);
    screen.present(&mut Vec::new());
    let mut out = Vec::new();
    screen.release(&mut out);
    assert_eq!(count(&out, b"d=I"), 2);
}

fn sixel_screen() -> Screen {
    let mut screen = Screen::new(20, 5);
    screen.enable_graphics(Protocol::Sixel, Compression::None, IdSpace::FIRST);
    screen
}

#[test]
fn a_sixel_surface_rides_inside_the_frames_bracket_after_the_text() {
    let mut screen = sixel_screen();
    assert!(screen.has_graphics());
    screen.buffer().put_str(0, 0, "title", &Style::new());
    let mut surface = painted(Rect::new(2, 2, 4, 2));
    screen.surface(WidgetId::new(1), &mut surface);
    let mut out = Vec::new();
    screen.present(&mut out);

    assert_eq!(count(&out, BSU), 1);
    assert_eq!(count(&out, ESU), 1);
    assert_eq!(count(&out, b"\x1b_G"), 0, "no kitty command on this path");
    let text = String::from_utf8_lossy(&out);
    let title = text.find("title").expect("text");
    let image = text.find("\x1bP0;1;0q").expect("a sixel image");
    let esu = text.find("\x1b[?2026l").expect("ESU");
    assert!(title < image && image < esu, "{text:?}");
    assert!(
        text.contains("\x1b[3;3H\x1b7\x1bP0;1;0q\"1;1;8;4"),
        "{text:?}"
    );
    assert!(!surface.is_dirty());

    // Steady state: the bare bracket.
    screen.buffer().put_str(0, 0, "title", &Style::new());
    screen.surface(WidgetId::new(1), &mut surface);
    out.clear();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat());
}

#[test]
fn a_sixel_surface_leaving_the_frame_forces_the_next_frame_to_redraw() {
    let mut screen = sixel_screen();
    let mut surface = painted(Rect::new(2, 2, 4, 2));
    screen.surface(WidgetId::new(1), &mut surface);
    screen.buffer().put_str(0, 0, "title", &Style::new());
    screen.present(&mut Vec::new());

    // The surface is not shown: its cells still hold its pixels.
    screen.buffer().put_str(0, 0, "title", &Style::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 0, "the frame itself is unchanged");

    screen.buffer().put_str(0, 0, "title", &Style::new());
    out.clear();
    screen.present(&mut out);
    assert_eq!(
        count(&out, b"\x1b[2J"),
        1,
        "the frame after clears and redraws"
    );
    assert!(
        String::from_utf8_lossy(&out).contains("title"),
        "the text under the image is painted again"
    );
}

#[test]
fn a_kitty_surface_leaving_the_frame_never_forces_a_redraw() {
    let mut screen = kitty_screen();
    let mut surface = painted(Rect::new(2, 2, 4, 2));
    screen.surface(WidgetId::new(1), &mut surface);
    screen.present(&mut Vec::new());

    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"a=d"), 1, "one placement delete");
    out.clear();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 0, "the cells were never covered");
}

#[test]
fn enclosed_bytes_ride_inside_the_bracket_and_go_out_once() {
    let mut screen = Screen::new(4, 1);
    screen.present(&mut Vec::new());
    screen.enclose(b"\x1b]9;4;1;50\x1b\\");
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(
        out,
        [BSU, b"\x1b]9;4;1;50\x1b\\".as_slice(), ESU].concat(),
        "after the cells, before ESU"
    );
    out.clear();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat(), "queued once, sent once");
}

/// A headline over cells 0..20 of rows 1 and 2.
fn a_headline(text: &str) -> Headline {
    Headline::fill(
        text,
        Rect::new(0, 1, 20, 2),
        Style::new().fg(Color::Indexed(4)),
    )
    .expect("the rect fits a headline")
}

#[test]
fn a_headline_rides_inside_the_bracket_after_the_cells() {
    let mut screen = Screen::new(20, 4);
    screen.present(&mut Vec::new());
    screen.headline(Some(a_headline("magistr")));
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(
        out,
        [
            BSU,
            b"\x1b[2;1H".as_slice(),
            b"\x1b[38;5;4m".as_slice(),
            b"\x1b]66;s=2;magistr   \x1b\\".as_slice(),
            b"\x1b[0m".as_slice(),
            ESU
        ]
        .concat()
    );
}

#[test]
fn the_pen_is_reset_before_esu_so_nothing_after_the_run_inherits_it() {
    let mut screen = Screen::new(20, 4);
    screen.present(&mut Vec::new());
    screen.headline(Some(a_headline("magistr")));
    let mut out = Vec::new();
    screen.present(&mut out);
    let esu = out
        .windows(ESU.len())
        .position(|w| w == ESU)
        .expect("the frame closes");
    let reset = out
        .windows(4)
        .rposition(|w| w == b"\x1b[0m")
        .expect("the pen is reset");
    assert!(reset < esu, "the reset comes before ESU");
}

#[test]
fn an_unchanged_headline_costs_nothing() {
    let mut screen = Screen::new(20, 4);
    screen.headline(Some(a_headline("magistr")));
    screen.present(&mut Vec::new());
    screen.headline(Some(a_headline("magistr")));
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(out, [BSU, ESU].concat(), "a settled headline is silent");
}

#[test]
fn a_changed_headline_is_emitted_again() {
    let mut screen = Screen::new(20, 4);
    screen.headline(Some(a_headline("magistr")));
    screen.present(&mut Vec::new());
    screen.headline(Some(a_headline("other")));
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b]66;s=2;other     \x1b\\"), 1);
}

#[test]
fn a_redraw_puts_the_headline_back() {
    let mut screen = Screen::new(20, 4);
    screen.headline(Some(a_headline("magistr")));
    screen.present(&mut Vec::new());
    screen.invalidate();
    screen.headline(Some(a_headline("magistr")));
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 1, "the display was cleared");
    assert_eq!(count(&out, b"\x1b]66;s=2;"), 1, "and the run went again");
}

#[test]
fn a_headline_leaving_the_frame_forces_a_redraw() {
    // Our buffer holds blanks where the terminal holds scaled glyphs, so
    // the diff alone would leave them on screen.
    let mut screen = Screen::new(20, 4);
    screen.headline(Some(a_headline("magistr")));
    screen.present(&mut Vec::new());
    screen.headline(None);
    screen.present(&mut Vec::new());
    let mut out = Vec::new();
    screen.present(&mut out);
    assert_eq!(count(&out, b"\x1b[2J"), 1);
}

#[test]
fn a_screen_nobody_gave_a_headline_is_byte_identical() {
    let mut screen = Screen::new(20, 4);
    draw_page(&mut screen, "hi");
    let mut with = Vec::new();
    screen.present(&mut with);

    let mut plain = Screen::new(20, 4);
    draw_page(&mut plain, "hi");
    plain.headline(None);
    let mut without = Vec::new();
    plain.present(&mut without);
    assert_eq!(with, without);
}
