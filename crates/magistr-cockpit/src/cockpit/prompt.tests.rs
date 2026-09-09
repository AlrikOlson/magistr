use super::*;

fn typed(text: &str) -> Prompt {
    let mut p = Prompt::new();
    p.insert(text);
    p
}

/// The single visible row of a one-line prompt.
fn window(p: &Prompt, columns: u16) -> (String, String, String) {
    let mut rows = p.window_rows(columns, 1);
    rows.pop().expect("one row")
}

#[test]
fn typing_and_pasting_insert_at_the_cursor() {
    let mut p = Prompt::new();
    assert!(p.is_empty());
    p.insert("hllo");
    p.left();
    p.left();
    p.left();
    p.insert("e");
    assert_eq!(p.text(), "hello");
    assert_eq!(p.cursor(), 2);
    p.end();
    p.insert(" wor\nld");
    assert_eq!(p.text(), "hello wor\nld", "a paste keeps its newline");
    assert_eq!(p.cursor(), p.text().len());
}

#[test]
fn the_cursor_and_the_deletions_move_by_grapheme() {
    let mut p = typed("aé👨‍👩‍👧b");
    assert_eq!(p.cursor(), p.text().len());
    p.left();
    assert_eq!(&p.text()[p.cursor()..], "b");
    p.left();
    assert_eq!(&p.text()[p.cursor()..], "👨‍👩‍👧b");
    p.delete();
    assert_eq!(p.text(), "aéb");
    p.backspace();
    assert_eq!(p.text(), "ab");
    assert_eq!(p.cursor(), 1);
    p.home();
    p.backspace();
    assert_eq!(p.text(), "ab", "nothing before the start");
    p.right();
    p.right();
    p.right();
    p.delete();
    assert_eq!(p.text(), "ab", "nothing after the end");
    assert_eq!(p.cursor(), 2);
}

#[test]
fn the_kills_take_the_line_start_the_line_end_and_the_word() {
    let mut p = typed("one two  three");
    p.kill_word();
    assert_eq!(p.text(), "one two  ");
    p.kill_word();
    assert_eq!(p.text(), "one ");
    p.insert("four five");
    p.home();
    p.right();
    p.right();
    p.kill_to_start();
    assert_eq!(p.text(), "e four five");
    assert_eq!(p.cursor(), 0);
    p.right();
    p.right();
    p.kill_to_end();
    assert_eq!(p.text(), "e ");
    p.kill_word();
    assert_eq!(p.text(), "");
    p.kill_word();
    p.kill_to_start();
    p.kill_to_end();
    assert!(p.is_empty());
}

#[test]
fn newlines_make_rows_and_up_down_move_across_lines_by_column() {
    let mut p = typed("alpha\nlonger line\nz");
    assert_eq!(p.rows(), 3);
    assert!(!p.down(), "already the last line");
    assert!(p.up(), "from `z` to column 1 of the middle line");
    assert_eq!(&p.text()[p.cursor()..], "onger line\nz");
    p.end();
    assert!(p.up(), "column 11 pins to the shorter line's end");
    assert_eq!(&p.text()[p.cursor()..], "\nlonger line\nz");
    assert!(!p.up(), "the first line");
    assert!(p.down());
    assert_eq!(&p.text()[p.cursor()..], "r line\nz");
    assert!(p.down());
    assert_eq!(&p.text()[p.cursor()..], "", "`z` is shorter: its end");
    assert_eq!(typed("").rows(), 1, "an empty prompt is one row");
}

#[test]
fn home_end_and_the_kills_stay_on_the_cursors_line() {
    let mut p = typed("first line\nsecond");
    p.home();
    assert_eq!(&p.text()[p.cursor()..], "second");
    p.up();
    p.end();
    assert_eq!(&p.text()[p.cursor()..], "\nsecond");
    p.kill_to_start();
    assert_eq!(p.text(), "\nsecond", "only the first line dies");
    let mut p = typed("first\nsecond words");
    p.home();
    p.kill_to_end();
    assert_eq!(p.text(), "first\n", "only the second line dies");
    let mut p = typed("first\nsecond words");
    p.kill_word();
    assert_eq!(p.text(), "first\nsecond ");
    p.kill_word();
    assert_eq!(p.text(), "first\n", "the word kill stops at the newline");
}

#[test]
fn complete_replaces_from_the_anchor_and_moves_the_cursor_after() {
    let mut p = typed("read @pr please");
    p.left();
    p.left();
    p.left();
    p.left();
    p.left();
    p.left();
    p.left();
    assert_eq!(&p.text()[p.cursor()..], " please");
    p.complete(5, "@src/prompt.rs ");
    assert_eq!(p.text(), "read @src/prompt.rs  please");
    assert_eq!(&p.text()[p.cursor()..], " please");
}

#[test]
fn take_sends_the_line_into_history_without_repeats_and_empties_the_prompt() {
    let mut p = typed("first");
    assert_eq!(p.take(), "first");
    assert!(p.is_empty());
    assert_eq!(p.cursor(), 0);
    p.insert("first");
    p.take();
    p.insert("second");
    p.take();
    assert_eq!(p.history(), ["first", "second"]);
    for i in 0..HISTORY + 5 {
        p.insert(&format!("line {i}"));
        p.take();
    }
    assert_eq!(p.history().len(), HISTORY);
    assert_eq!(p.history()[0], "line 5", "the oldest entries fall off");
}

#[test]
fn up_and_down_walk_the_history_and_bring_the_draft_back() {
    let mut p = Prompt::new();
    assert!(!p.history_up(), "no history yet");
    assert!(!p.history_down());
    for line in ["alpha", "beta"] {
        p.insert(line);
        p.take();
    }
    p.insert("draft");
    assert!(p.history_up());
    assert_eq!(p.text(), "beta");
    assert_eq!(p.cursor(), 4);
    assert!(p.history_up());
    assert_eq!(p.text(), "alpha");
    assert!(!p.history_up(), "the oldest");
    assert_eq!(p.text(), "alpha");
    assert!(p.history_down());
    assert_eq!(p.text(), "beta");
    assert!(p.history_down());
    assert_eq!(p.text(), "draft", "below the newest is the draft");
    assert!(!p.history_down());
    // Editing a recalled entry makes it the draft; the history is untouched.
    p.history_up();
    p.insert("!");
    assert_eq!(p.text(), "beta!");
    assert!(!p.history_down(), "no longer browsing");
    assert_eq!(p.history(), ["alpha", "beta"]);
    assert_eq!(p.take(), "beta!");
    assert_eq!(p.history(), ["alpha", "beta", "beta!"]);
}

#[test]
fn the_window_shows_the_cursor_cell_and_scrolls_a_long_line() {
    let p = typed("hello");
    assert_eq!(
        window(&p, 20),
        ("hello".to_owned(), " ".to_owned(), String::new())
    );
    let mut p = typed("hello");
    p.home();
    assert_eq!(
        window(&p, 20),
        (String::new(), "h".to_owned(), "ello".to_owned())
    );
    p.right();
    assert_eq!(
        window(&p, 3),
        ("h".to_owned(), "e".to_owned(), "l".to_owned()),
        "after is cut to the width"
    );
    let mut long = typed("0123456789");
    assert_eq!(
        window(&long, 4),
        ("789".to_owned(), " ".to_owned(), String::new()),
        "the tail scrolls into view"
    );
    long.home();
    long.right();
    long.right();
    long.right();
    long.right();
    long.right();
    assert_eq!(
        window(&long, 4),
        ("234".to_owned(), "5".to_owned(), String::new())
    );
    assert_eq!(
        window(&long, 0),
        (String::new(), String::new(), String::new())
    );
    assert_eq!(
        window(&long, 1),
        (String::new(), "5".to_owned(), String::new()),
        "one column is the cursor"
    );
}

#[test]
fn the_window_shows_wide_graphemes_and_tabs_as_symbols() {
    let mut p = typed("a\tb\n漢字");
    assert_eq!(
        p.window_rows(10, 5),
        vec![
            ("a⇥b".to_owned(), String::new(), String::new()),
            ("漢字".to_owned(), " ".to_owned(), String::new()),
        ]
    );
    p.up();
    p.home();
    p.right();
    assert_eq!(
        p.window_rows(10, 5),
        vec![
            ("a".to_owned(), "⇥".to_owned(), "b".to_owned()),
            ("漢字".to_owned(), String::new(), String::new()),
        ]
    );
    p.down();
    p.end();
    assert_eq!(
        window(&p, 3),
        ("字".to_owned(), " ".to_owned(), String::new()),
        "a two-cell grapheme counts two"
    );
}

#[test]
fn window_rows_slide_to_keep_the_cursors_line_shown() {
    let mut p = typed("one\ntwo\nthree\nfour");
    assert_eq!(
        p.window_rows(10, 5),
        vec![
            ("one".to_owned(), String::new(), String::new()),
            ("two".to_owned(), String::new(), String::new()),
            ("three".to_owned(), String::new(), String::new()),
            ("four".to_owned(), " ".to_owned(), String::new()),
        ]
    );
    let capped = p.window_rows(10, 2);
    assert_eq!(
        capped,
        vec![
            ("three".to_owned(), String::new(), String::new()),
            ("four".to_owned(), " ".to_owned(), String::new()),
        ],
        "the window ends at the cursor's line"
    );
    p.up();
    p.up();
    p.up();
    assert_eq!(
        p.window_rows(10, 2),
        vec![
            ("one".to_owned(), " ".to_owned(), String::new()),
            ("two".to_owned(), String::new(), String::new()),
        ],
        "moving up slides the window with the cursor"
    );
    assert_eq!(p.window_rows(10, 0), Vec::new());
}
