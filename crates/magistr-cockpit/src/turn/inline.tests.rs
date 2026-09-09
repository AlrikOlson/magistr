use super::*;

fn spans(text: &str) -> (String, Vec<(usize, usize, SpanStyle)>) {
    let (display, spans) = parse_inline(text);
    let flat = spans
        .into_iter()
        .map(|s| (s.start, s.end, s.style))
        .collect();
    (display, flat)
}

#[test]
fn plain_text_passes_through_without_spans() {
    let (display, spans) = spans("just words, nothing more");
    assert_eq!(display, "just words, nothing more");
    assert!(spans.is_empty());
}

#[test]
fn strong_emphasis_strips_its_markers() {
    let (display, spans) = spans("a **bold** word");
    assert_eq!(display, "a bold word");
    assert_eq!(spans, vec![(2, 6, SpanStyle::Bold)]);
}

#[test]
fn underscore_strong_matches_too() {
    let (display, spans) = spans("__all bold__");
    assert_eq!(display, "all bold");
    assert_eq!(spans, vec![(0, 8, SpanStyle::Bold)]);
}

#[test]
fn emphasis_with_star_and_underscore() {
    let (display, spans) = spans("*em* and _also_");
    assert_eq!(display, "em and also");
    assert_eq!(
        spans,
        vec![(0, 2, SpanStyle::Italic), (7, 11, SpanStyle::Italic)]
    );
}

#[test]
fn code_span_keeps_its_content_verbatim() {
    let (display, spans) = spans("run `cargo *test*` now");
    assert_eq!(display, "run cargo *test* now");
    assert_eq!(spans, vec![(4, 16, SpanStyle::Code)]);
}

#[test]
fn link_shows_the_label_and_carries_the_url() {
    let (display, spans) = spans("see [the docs](https://example.com) here");
    assert_eq!(display, "see the docs here");
    assert_eq!(
        spans,
        vec![(4, 12, SpanStyle::Link("https://example.com".to_owned()))]
    );
}

#[test]
fn an_unclosed_marker_and_an_intra_word_underscore_stay_literal() {
    let (display, spans) = spans("2 * 3 = 6 and a_variable_name");
    assert_eq!(display, "2 * 3 = 6 and a_variable_name");
    assert!(spans.is_empty());
}

#[test]
fn an_empty_pair_stays_literal() {
    let (display, spans) = spans("**** and ``");
    assert_eq!(display, "**** and ``");
    assert!(spans.is_empty());
}

#[test]
fn a_bare_bracket_stays_literal() {
    let (display, spans) = spans("array[0] and [label] alone");
    assert_eq!(display, "array[0] and [label] alone");
    assert!(spans.is_empty());
}

#[test]
fn several_spans_keep_their_order_and_offsets() {
    let (display, spans) = spans("**a** `b` *c*");
    assert_eq!(display, "a b c");
    assert_eq!(
        spans,
        vec![
            (0, 1, SpanStyle::Bold),
            (2, 3, SpanStyle::Code),
            (4, 5, SpanStyle::Italic),
        ]
    );
}

#[test]
fn multibyte_text_around_markers_keeps_byte_offsets_honest() {
    let (display, spans) = spans("héllo **wörld**");
    assert_eq!(display, "héllo wörld");
    let start = "héllo ".len();
    assert_eq!(spans, vec![(start, start + "wörld".len(), SpanStyle::Bold)]);
}

#[test]
fn a_window_slices_and_rebases_a_span() {
    let span = Span::new(4, 10, SpanStyle::Bold);
    assert_eq!(span.window(0, 4), None);
    assert_eq!(span.window(10, 20), None);
    assert_eq!(span.window(0, 7), Some(Span::new(4, 7, SpanStyle::Bold)));
    assert_eq!(span.window(6, 20), Some(Span::new(0, 4, SpanStyle::Bold)));
    assert_eq!(
        span.window(5, 8).map(|s| s.shifted(2)),
        Some(Span::new(2, 5, SpanStyle::Bold))
    );
}
