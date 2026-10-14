//! Producers and current adapters share canonical records and observation contracts.
use magistr_workflow_contracts as contracts;
use std::any::TypeId;

fn same_type<A: 'static, B: 'static>() {
    assert_eq!(TypeId::of::<A>(), TypeId::of::<B>());
}

#[test]
fn producers_and_surfaces_share_the_same_record_definitions() {
    same_type::<contracts::Chunk, magistr_workflow::roadmap::domain::Chunk>();
    same_type::<contracts::Signal, magistr_workflow::signal::domain::Signal>();
    same_type::<contracts::ThinkStep, magistr_workflow::think::domain::ThinkStep>();
    same_type::<contracts::think::Branch, magistr_workflow::think::domain::Branch>();
    same_type::<contracts::Objective, magistr_workflow::ship::domain::objective::Objective>();
    same_type::<contracts::Task, magistr_workflow::ship::domain::task::Task>();
    same_type::<contracts::Action, magistr_workflow::ship::domain::action::Action>();
    same_type::<contracts::Check, magistr_workflow::ship::domain::check::Check>();
    same_type::<contracts::ship::ReportRecord, magistr_workflow::ship::report::ReportRecord>();
    same_type::<contracts::StructuredContent, magistr_workflow::content::StructuredContent>();
}

#[test]
fn projections_observations_and_broadcasts_share_canonical_type_identity() {
    same_type::<contracts::NativeSnapshot, magistr_engine::NativeSnapshot>();
    same_type::<
        contracts::ReadState<contracts::ShipData>,
        magistr_engine::native::ReadState<magistr_engine::native::ShipData>,
    >();
    same_type::<contracts::ReadSource, magistr_engine::native::ReadSource>();
    same_type::<contracts::StoreStamp, magistr_workflow::infra::StoreStamp>();
    same_type::<contracts::NativeEvent, magistr_engine::NativeEvent>();
    same_type::<contracts::NativeFrame, magistr_engine::NativeFrame>();
    same_type::<contracts::Delivery, magistr_engine::Delivery>();
    same_type::<contracts::OwnerGeneration, magistr_engine::OwnerGeneration>();
    same_type::<contracts::Family, magistr_workflow::infra::Family>();
    same_type::<contracts::ThinkFrame, magistr_workflow::think::broadcast::BroadcastFrame>();
    same_type::<contracts::ShipFrame, magistr_workflow::ship::broadcast::BroadcastFrame>();
    same_type::<contracts::RoadmapFrame, magistr_workflow::roadmap::broadcast::RoadmapFrame>();
    same_type::<contracts::SignalFrame, magistr_workflow::signal::broadcast::SignalFrame>();
    same_type::<magistr_domain::workflow::CheckpointReason, magistr_engine::CheckpointReason>();
}
