use super::*;

#[test]
fn the_agent_stage_is_the_default_and_the_only_one_with_an_agent() {
    assert_eq!(LoopStage::default(), LoopStage::Agent);
    assert!(!LoopStage::Agent.is_between_agents());
    for stage in [LoopStage::Connecting, LoopStage::Gates, LoopStage::Judge] {
        assert!(
            stage.is_between_agents(),
            "{stage:?} has no agent to stream"
        );
    }
}

#[test]
fn every_stage_has_a_word_and_displays_it() {
    assert_eq!(LoopStage::Connecting.label(), "starting the agent");
    assert_eq!(LoopStage::Agent.label(), "agent working");
    assert_eq!(LoopStage::Gates.label(), "gates running");
    assert_eq!(LoopStage::Judge.label(), "judge deciding");
    assert_eq!(LoopStage::Judge.to_string(), "judge deciding");
}
