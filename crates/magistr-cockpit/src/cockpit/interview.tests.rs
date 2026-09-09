use super::*;

fn report() -> SurveyReport {
    let mut report = SurveyReport::named("tidewatch");
    report.never_break = vec![
        "prediction accuracy".to_owned(),
        "the public API".to_owned(),
        "offline operation".to_owned(),
    ];
    report
}

/// Three questions in declaration order, the project named in the
/// first, the survey's chips on the second, the charter notches on the
/// third.
#[test]
fn three_questions_with_the_surveys_chips_pre_offered() {
    let interview = Interview::of(&report());
    assert_eq!(interview.len(), 3);
    assert!(!interview.is_empty());
    assert_eq!(interview.current(), 0);
    assert_eq!(
        interview.asking().map(Question::prompt),
        Some("What should tidewatch become?")
    );
    assert_eq!(interview.next_short(), Some("what must never break"));
    assert_eq!(interview.remaining(), 2);
    let two = interview.question(1).expect("question two");
    assert_eq!(
        two.chips(),
        vec![
            ("prediction accuracy", false),
            ("the public API", false),
            ("offline operation", false)
        ]
    );
    let three = interview.question(2).expect("question three");
    assert_eq!(three.chips().len(), Interview::FREEDOM.len());
}

/// Nothing picked or typed refuses to advance; own words win over the
/// picks; the picks alone are joined as the answer.
#[test]
fn own_words_win_and_an_empty_answer_holds() {
    let mut interview = Interview::of(&report());
    assert!(!interview.advance(), "nothing to answer with");
    for c in "A tide service".chars() {
        interview.type_char(c);
    }
    interview.backspace();
    assert_eq!(interview.asking().map(Question::own), Some("A tide servic"));
    assert!(interview.advance());
    assert_eq!(interview.current(), 1);
    assert_eq!(interview.answered()[0].answer(), Some("A tide servic"));
    assert!(interview.toggle(0));
    assert!(interview.toggle(1));
    assert!(!interview.toggle(9), "no such chip");
    for c in "the harbor feed".chars() {
        interview.type_char(c);
    }
    assert!(interview.advance());
    assert_eq!(
        interview.question(1).and_then(Question::answer),
        Some("the harbor feed"),
        "the typed words win over the two picks"
    );
    assert_eq!(
        interview.never_break(),
        vec!["prediction accuracy", "the public API"],
        "and the picks still name the gates"
    );
    assert_eq!(interview.next_short(), None, "the last question");
    assert_eq!(interview.remaining(), 0);
}

/// The charter notch is single-select, and the picks alone are the
/// answer when nothing is typed; the three answers come out together.
#[test]
fn the_notch_is_single_select_and_the_answers_come_out_together() {
    let mut interview = Interview::of(&report());
    interview.type_char('x');
    interview.advance();
    interview.toggle(2);
    interview.advance();
    assert!(interview.toggle(0));
    assert!(interview.toggle(2));
    assert_eq!(interview.freedom(), Some(2), "one pick at a time");
    assert_eq!(interview.answers(), None, "not done yet");
    assert!(interview.advance());
    assert!(interview.is_done());
    assert_eq!(
        interview.answers(),
        Some([
            "x".to_owned(),
            "offline operation".to_owned(),
            "free rein with checkpoints".to_owned()
        ])
    );
    assert_eq!(interview.asking(), None);
    assert!(!interview.advance(), "nothing left to answer");
}
