use super::*;
use crate::element::Kind;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::medium::Medium;

fn trail() -> Vec<CrumbStop> {
    vec![
        CrumbStop::back("projects", Some(Mark::Icon(Icon::Projects)), ElementId(1)),
        CrumbStop::here("magistr", Some(Mark::Sigil("magistr"))),
    ]
}

/// A back stop answers; the here stop is emphasized and inert.
#[test]
fn back_stops_answer_and_the_here_stop_does_not() {
    let lex = Lexicon::of(Medium::Night);
    let crumb = Crumb::el(&trail(), &lex);
    let mut ids = Vec::new();
    crumb.each(&mut |_, element| ids.extend(element.id));
    assert_eq!(ids, vec![ElementId(1)], "exactly the back stop answers");
}

/// A separator lies between each pair of stops, and the marks ride at
/// the volume's own sizes — icons 13, sigils 14.
#[test]
fn separators_lie_between_stops_and_marks_keep_their_sizes() {
    let lex = Lexicon::of(Medium::Night);
    let crumb = Crumb::el(&trail(), &lex);
    let (mut seps, mut mark_px) = (0, Vec::new());
    crumb.each(&mut |_, element| match &element.kind {
        Kind::Text { content, .. } if content == "\u{25b8}" => seps += 1,
        Kind::Mark { px, .. } => mark_px.push(*px),
        _ => {}
    });
    assert_eq!(seps, 1);
    assert_eq!(mark_px, vec![13, 14]);
}
