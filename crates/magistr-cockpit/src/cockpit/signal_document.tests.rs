use super::*;
use crate::harness::fixtures;
use serde_json::json;
#[test]
fn structured_signal_content_and_all_research_survive_seed_and_change() {
    let mut source = fixtures::signal_from(json!({"id":"feedback","body":"Original",
        "content":{"version":1,"summary":"Feedback summary","facts":[{"label":"Origin","value":"Owner"}],
            "sections":[{"heading":"Requirements","list":[{"text":"Stored check","done":true}]}]},
        "enrichment":[{"summary":"First finding","confidence":0.8,"sources":["https://example.com/docs"]}]}));
    assert_eq!(SignalRow::from_signal(&source).content, source.content);
    let mut roadmap = crate::cockpit::Roadmap::default();
    roadmap.seed(&[], &[source.clone()]);
    assert_eq!(roadmap.signals()[0].content, source.content);
    assert_eq!(roadmap.signals()[0].enrichment, source.enrichment);
    source.content.as_mut().unwrap().summary = "Changed feedback".into();
    roadmap.apply(
        &fixtures::frame("signal", "signal_changed", json!({"signal":source})),
        0.0,
        false,
    );
    assert_eq!(
        roadmap.signals()[0].content.as_ref().unwrap().summary,
        "Changed feedback"
    );
}
