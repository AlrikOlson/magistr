use super::*;

#[test]
fn a_rust_line_gets_more_than_one_colour() {
    let mut block = Highlight::for_fence("```rust", ThemeMode::Dark);
    let spans = block.line("fn main() { println!(\"hi\"); }");
    assert!(spans.len() > 1, "one flat span means no highlighting");
    let colours: std::collections::HashSet<_> = spans
        .iter()
        .map(|s| match &s.style {
            SpanStyle::Fg(c) => *c,
            other => panic!("a code span is a colour, got {other:?}"),
        })
        .collect();
    assert!(colours.len() > 1, "the keyword and the string differ");
}

#[test]
fn spans_cover_the_line_in_order_without_the_terminator() {
    let mut block = Highlight::for_fence("```rust", ThemeMode::Dark);
    let text = "let x = 1;";
    let spans = block.line(text);
    let mut at = 0;
    for span in &spans {
        assert_eq!(span.start, at, "spans are contiguous");
        assert!(span.end <= text.len(), "the \\n never leaks into a span");
        at = span.end;
    }
    assert_eq!(at, text.len());
}

#[test]
fn state_carries_across_lines_of_one_block() {
    let mut block = Highlight::for_fence("```rust", ThemeMode::Dark);
    let first = block.line("let s = \"an open string");
    let string_colour = first
        .last()
        .map(|s| s.style.clone())
        .expect("the tail is styled");
    let second = block.line("still inside it\";");
    assert_eq!(
        second.first().map(|s| s.style.clone()),
        Some(string_colour),
        "the unterminated string's colour continues on the next line"
    );
}

#[test]
fn an_unknown_language_highlights_as_plain_text() {
    let mut block = Highlight::for_fence("```no-such-language", ThemeMode::Dark);
    let spans = block.line("anything at all");
    let colours: std::collections::HashSet<_> = spans.iter().map(|s| s.style.clone()).collect();
    assert!(colours.len() <= 1, "plain text is one colour at most");
}

#[test]
fn the_fence_language_token_is_taken_from_the_fence_line() {
    let dark = Highlight::for_fence("  ```rust extra words", ThemeMode::Dark);
    let same = Highlight::for_fence("```rust", ThemeMode::Dark);
    assert_eq!(dark, same, "lang and mode drive equality at zero lines");
}

#[test]
fn modes_pick_different_themes() {
    let mut dark = Highlight::for_fence("```rust", ThemeMode::Dark);
    let mut light = Highlight::for_fence("```rust", ThemeMode::Light);
    let d = dark.line("fn main() {}");
    let l = light.line("fn main() {}");
    assert_ne!(d, l, "the two themes colour the keyword differently");
}
