use super::*;
use crate::cockpit::proposed_chunk::ProposedChunk;
use crate::cockpit::scaffold_kind::ScaffoldKind;
use crate::cockpit::survey_kind::SurveyKind;

fn report() -> SurveyReport {
    let mut report = SurveyReport::named("tidewatch");
    report.detected = vec![
        (SurveyKind::Stack, "Rust".to_owned()),
        (SurveyKind::Tests, "nextest".to_owned()),
        (SurveyKind::Git, "main".to_owned()),
        (SurveyKind::Agent, "claude on PATH".to_owned()),
    ];
    report.written = vec![
        (
            ScaffoldKind::Config,
            ".magistr/project.json".to_owned(),
            true,
        ),
        (ScaffoldKind::Store, "think-and-ship".to_owned(), true),
    ];
    report.never_break = vec!["the public API".to_owned()];
    report
}

fn proposal() -> Proposal {
    Proposal {
        chunks: vec![
            ProposedChunk::new("harmonic-engine", "core", 100),
            ProposedChunk::new("forecast-endpoint", "api", 110).standing_on(&["harmonic-engine"]),
        ],
        sources: Vec::new(),
    }
}

/// The flow opens on the ground; a typed folder chooses it and asks
/// for the survey; the same folder chosen again with a survey kept
/// asks nothing.
#[test]
fn choosing_the_ground_asks_for_the_survey_once() {
    let mut birth = Birth::default();
    assert_eq!(birth.moment(), BirthMoment::Ground);
    birth.open(true);
    assert!(birth.is_armed());
    birth.type_into(GroundField::Browse);
    for c in "~/Code/tidewatch".chars() {
        birth.field_char(c);
    }
    assert_eq!(
        birth.field(),
        Some((GroundField::Browse, "~/Code/tidewatch"))
    );
    let ground = birth.field_ground("~/Code").expect("a folder");
    assert_eq!(ground, Ground::Folder("~/Code/tidewatch".to_owned()));
    let cmd = birth.choose(ground.clone(), 0);
    assert_eq!(
        cmd,
        Some(BirthCmd::Survey {
            ground: ground.clone()
        })
    );
    assert_eq!(birth.moment(), BirthMoment::Survey);
    assert!(!birth.is_armed(), "a chosen ground disarms the slot");
    assert_eq!(birth.field(), None);
    assert_eq!(birth.name(), "tidewatch");
    assert!(birth.is_scanning(0.0, true));
    birth.surveyed(report(), 10);
    assert!(birth.stand_down());
    assert_eq!(birth.moment(), BirthMoment::Ground);
    assert_eq!(birth.choose(ground, 20), None, "the survey is kept");
    assert_eq!(birth.moment(), BirthMoment::Survey);
    assert!(!birth.stand_down() || birth.moment() == BirthMoment::Ground);
    assert!(!Birth::default().stand_down(), "the ground itself rises");
}

/// A clone's field names the folder it lands in; an empty ground is
/// the folder to create; an empty field names nothing.
#[test]
fn the_fields_name_their_grounds() {
    let mut birth = Birth::default();
    birth.type_into(GroundField::Clone);
    assert_eq!(birth.field_ground("~/Code"), None);
    for c in "https://x/y/harbor.git".chars() {
        birth.field_char(c);
    }
    assert_eq!(
        birth.field_ground("~/Code"),
        Some(Ground::Clone {
            url: "https://x/y/harbor.git".to_owned(),
            into: "~/Code/harbor".to_owned()
        })
    );
    birth.field_backspace();
    assert_eq!(birth.field().map(|(_, t)| t), Some("https://x/y/harbor.gi"));
    assert!(birth.close_field());
    assert!(!birth.close_field());
    birth.type_into(GroundField::Empty);
    birth.field_char('a');
    assert_eq!(birth.field_ground(""), Some(Ground::Empty("a".to_owned())));
}

/// The survey's rows land one every rise stagger from the tick the
/// report arrived, the scan meter counts the facts, and once the last
/// row has held its beat the interview opens — at once when the
/// motion is not running.
#[test]
fn the_survey_lands_its_rows_by_tick_and_then_opens_the_interview() {
    let mut birth = Birth::default();
    birth.choose(Ground::Folder("/p/tidewatch".to_owned()), 0);
    assert_eq!(birth.rows_landed(0.0, true), 0);
    assert_eq!(birth.scan(0.0, true), (0, 0));
    birth.surveyed(report(), 100);
    assert_eq!(birth.rows_landed(90.0, true), 0);
    assert_eq!(birth.rows_landed(100.0, true), 1);
    let stagger = Motion::RISE_STAGGER as f32;
    assert_eq!(birth.rows_landed(100.0 + stagger * 2.0, true), 3);
    assert_eq!(
        birth.rows_landed(100.0 + stagger * 40.0, true),
        6,
        "capped at the rows"
    );
    assert_eq!(birth.scan(100.0 + stagger * 2.0, true), (3, 4));
    assert_eq!(birth.scan(100.0 + stagger * 40.0, true), (4, 4));
    assert_eq!(birth.row_rise(0, 100.0, true), Some(0.0));
    assert_eq!(birth.row_rise(0, 99.0, true), None);
    assert_eq!(
        birth.row_rise(0, 100.0 + Motion::ARRIVAL_STEPS as f32, true),
        None
    );
    assert!(birth.is_scanning(100.0, true));
    assert!(!birth.settle(100.0, true));
    let done = 100.0 + (6 * Motion::RISE_STAGGER + Birth::SURVEY_BEAT) as f32;
    assert!(!birth.settle(done - 1.0, true));
    assert!(birth.settle(done, true));
    assert_eq!(birth.moment(), BirthMoment::Interview);
    assert!(!birth.settle(done, true), "settled once");
    let interview = birth.interview().expect("the interview opened");
    assert_eq!(interview.question(1).map(|q| q.chips().len()), Some(1));
    // Reduced motion: everything is landed and the interview opens on
    // the report's arrival.
    let mut still = Birth::default();
    still.choose(Ground::Folder("/p/x".to_owned()), 5);
    still.surveyed(report(), 5);
    assert_eq!(still.rows_landed(5.0, false), 6);
    assert_eq!(still.row_rise(3, 5.0, false), None);
    assert!(!still.is_scanning(5.0, false));
    assert!(still.settle(5.0, false));
    // The radar is a pure function of the tick, from the top at the
    // tick the ground was chosen.
    assert!((still.radar_phase(5.0)).abs() < f32::EPSILON);
    assert!((still.radar_phase(5.0 + Birth::RADAR_STEPS as f32 / 2.0) - 0.5).abs() < 1e-6);
    assert!((still.radar_phase(5.0 + Birth::RADAR_STEPS as f32)).abs() < 1e-6);
}

/// A failed survey stands the flow down with nothing kept.
#[test]
fn a_failed_survey_stands_down_to_the_ground() {
    let mut birth = Birth::default();
    birth.choose(Ground::Empty("/p/new".to_owned()), 0);
    birth.survey_failed();
    assert_eq!(birth.moment(), BirthMoment::Ground);
    assert_eq!(birth.ground(), None);
    assert_eq!(birth.name(), "");
    assert_eq!(birth.dir(), "");
}

/// Answering the three questions asks for the chart; the proposal is
/// born star by star from its arrival; a drag reorders, a rename pins,
/// a redraw keeps the pins, and Accept carries the proposal, the picks
/// and the notch.
#[test]
fn the_interview_charts_and_the_charting_accepts() {
    let mut birth = Birth::default();
    birth.choose(Ground::Folder("/p/tidewatch".to_owned()), 0);
    birth.surveyed(report(), 0);
    assert!(birth.settle(0.0, false));
    assert_eq!(birth.advance(), None, "nothing answered yet");
    for c in "a tide service".chars() {
        assert!(birth.answer_char(c));
    }
    assert!(birth.answer_backspace());
    assert_eq!(birth.advance(), None, "two questions left");
    assert!(birth.toggle_chip(0));
    assert_eq!(birth.advance(), None);
    assert!(birth.toggle_chip(1));
    let cmd = birth.advance().expect("the last answer charts");
    match cmd {
        BirthCmd::Chart {
            dir,
            name,
            answers,
            pinned,
        } => {
            assert_eq!(dir, "/p/tidewatch");
            assert_eq!(name, "tidewatch");
            assert_eq!(answers[0], "a tide servic");
            assert_eq!(answers[1], "the public API");
            assert_eq!(answers[2], Interview::FREEDOM[1]);
            assert!(pinned.is_empty());
        }
        other => panic!("not a chart: {other:?}"),
    }
    assert_eq!(birth.moment(), BirthMoment::Charting);
    assert!(birth.is_charting());
    assert!(!birth.answer_char('x'), "the interview is over");
    assert_eq!(birth.accept(), None, "nothing to accept while drafting");
    birth.charted(proposal(), 200);
    assert!(!birth.is_charting());
    assert!((birth.born_alpha(0, 199.0, true)).abs() < f32::EPSILON);
    assert!(
        (birth.born_alpha(0, 200.0 + Motion::ARRIVAL_STEPS as f32, true) - 1.0).abs()
            < f32::EPSILON
    );
    assert!(
        (birth.born_alpha(1, 200.0, true)).abs() < f32::EPSILON,
        "the second waits its turn"
    );
    assert!(
        (birth.born_alpha(1, 0.0, false) - 1.0).abs() < f32::EPSILON,
        "reduced motion: born"
    );
    assert!(birth.is_being_born(200.0));
    assert!(birth.is_moving(200.0));
    let settled = 200.0 + (Birth::BORN_STAGGER + Motion::ARRIVAL_STEPS) as f32;
    assert!(!birth.is_being_born(settled));
    assert!(!birth.is_moving(settled));
    // Drag the second row onto the first.
    birth.drag_from(1);
    assert_eq!(birth.dragging(), Some(1));
    assert!(birth.drop_on(0));
    assert_eq!(birth.dragging(), None);
    assert!(!birth.drop_on(0), "nothing dragged");
    assert_eq!(
        birth.proposal().map(|p| p.chunks[0].id.as_str()),
        Some("forecast-endpoint")
    );
    // Rename the second row — the one the drag did not pin.
    assert!(birth.edit(1));
    assert!(!birth.edit(9));
    birth.edit_char('!');
    assert_eq!(birth.editing(), Some((1, "harmonic engine!")));
    assert!(birth.edit_backspace());
    assert!(birth.commit_edit());
    assert!(!birth.commit_edit());
    assert_eq!(
        birth.proposal().map(|p| p.chunks[1].name.as_str()),
        Some("harmonic engine")
    );
    assert!(birth.edit(0));
    assert!(birth.cancel_edit());
    assert!(!birth.cancel_edit());
    // Redraw keeps the pins.
    match birth.redraw().expect("a redraw asks again") {
        BirthCmd::Chart { pinned, .. } => assert_eq!(pinned.len(), 2),
        other => panic!("not a chart: {other:?}"),
    }
    birth.charted(proposal(), 300);
    match birth.accept().expect("the proposal is accepted") {
        BirthCmd::Accept {
            dir,
            name,
            proposal,
            never_break,
            freedom,
        } => {
            assert_eq!(dir, "/p/tidewatch");
            assert_eq!(name, "tidewatch");
            assert_eq!(proposal.chunks.len(), 2);
            assert_eq!(never_break, vec!["the public API"]);
            assert_eq!(freedom, Some(1));
        }
        other => panic!("not an accept: {other:?}"),
    }
    let born = BornData {
        name: "tidewatch".to_owned(),
        dir: "/p/tidewatch".to_owned(),
        chunks: proposal().records(),
        gates: vec!["cargo test".to_owned()],
        protected: Vec::new(),
    };
    birth.born(born.clone());
    assert_eq!(birth.moment(), BirthMoment::FirstLight);
    assert_eq!(birth.born_data(), Some(&born));
    assert!(!birth.is_moving(0.0));
    assert_eq!(birth.redraw(), None, "no redraw after first light");
    assert!(birth.stand_down());
    assert_eq!(birth.moment(), BirthMoment::Ground);
}
