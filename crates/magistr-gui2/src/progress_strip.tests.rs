use super::*;
use crate::medium::Medium;

fn bytes(c: Srgba) -> (u8, u8, u8) {
    let b = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    (b(c.r), b(c.g), b(c.b))
}

/// A colour's bytes.
type Rgb = (u8, u8, u8);

/// The strip's label with its ink, and the filled share's hue.
fn read(strip: &Element) -> (String, Rgb, Option<Rgb>) {
    let mut label = String::new();
    let mut ink = None;
    let mut fills = Vec::new();
    strip.each(&mut |_, element| match &element.kind {
        crate::element::Kind::Text { content, .. } => {
            label.push_str(content);
            ink = element.fill.map(bytes);
        }
        _ => {
            if let Some(fill) = element.fill {
                fills.push(bytes(fill));
            }
        }
    });
    // The track's fill first, then the filled share's.
    (
        label,
        ink.expect("the label carries ink"),
        fills.get(1).copied(),
    )
}

#[test]
fn a_running_report_is_a_green_share_with_its_percent() {
    let lex = Lexicon::of(Medium::Night);
    let strip = ProgressStrip::el(Progress::Running(70), &lex).expect("a strip");
    let (label, ink, fill) = read(&strip);
    assert_eq!(label, "70%");
    assert_eq!(ink, bytes(lex.sb1));
    assert_eq!(fill, Some(bytes(lex.green)));
}

#[test]
fn a_failed_report_wears_the_failure_hue_and_ink() {
    let lex = Lexicon::of(Medium::Day);
    let strip = ProgressStrip::el(Progress::Failed(40), &lex).expect("a strip");
    let (label, ink, fill) = read(&strip);
    assert_eq!(label, "40%");
    assert_eq!(fill, Some(bytes(lex.red)));
    assert_eq!(ink, bytes(lex.red));
}

#[test]
fn no_fraction_reads_working_and_nothing_reported_draws_nothing() {
    let lex = Lexicon::of(Medium::Night);
    let strip = ProgressStrip::el(Progress::Indeterminate, &lex).expect("a strip");
    assert_eq!(read(&strip).0, "working");
    assert!(ProgressStrip::el(Progress::Cleared, &lex).is_none());
}
