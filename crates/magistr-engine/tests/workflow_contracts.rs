//! Producers and current adapters share canonical records and observation contracts.
use magistr_workflow_contracts as contracts;
use std::any::TypeId;

fn same_type<A: 'static, B: 'static>() {
    assert_eq!(TypeId::of::<A>(), TypeId::of::<B>());
}

#[test]
fn producers_and_surfaces_share_the_same_record_definitions() {
    same_type::<contracts::Task, magistr_workflow::plan::domain::Task>();
    same_type::<contracts::Request, magistr_workflow::request::domain::Request>();
    same_type::<contracts::Note, magistr_workflow::decision::domain::Note>();
    same_type::<contracts::decision::Thread, magistr_workflow::decision::domain::Thread>();
    same_type::<contracts::Goal, magistr_workflow::run::domain::goal::Goal>();
    same_type::<contracts::Step, magistr_workflow::run::domain::step::Step>();
    same_type::<contracts::Progress, magistr_workflow::run::domain::progress::Progress>();
    same_type::<contracts::Check, magistr_workflow::run::domain::check::Check>();
    same_type::<contracts::run::ReportRecord, magistr_workflow::run::report::ReportRecord>();
    same_type::<contracts::StructuredContent, magistr_workflow::content::StructuredContent>();
}

#[test]
fn projections_observations_and_broadcasts_share_canonical_type_identity() {
    same_type::<contracts::NativeSnapshot, magistr_engine::NativeSnapshot>();
    same_type::<
        contracts::ReadState<contracts::RunData>,
        magistr_engine::native::ReadState<magistr_engine::native::RunData>,
    >();
    same_type::<contracts::ReadSource, magistr_engine::native::ReadSource>();
    same_type::<contracts::StoreStamp, magistr_workflow::infra::StoreStamp>();
    same_type::<contracts::NativeEvent, magistr_engine::NativeEvent>();
    same_type::<contracts::NativeFrame, magistr_engine::NativeFrame>();
    same_type::<contracts::Delivery, magistr_engine::Delivery>();
    same_type::<contracts::OwnerGeneration, magistr_engine::OwnerGeneration>();
    same_type::<contracts::Family, magistr_workflow::infra::Family>();
    same_type::<contracts::DecisionFrame, magistr_workflow::decision::broadcast::BroadcastFrame>();
    same_type::<contracts::RunFrame, magistr_workflow::run::broadcast::BroadcastFrame>();
    same_type::<contracts::PlanFrame, magistr_workflow::plan::broadcast::PlanFrame>();
    same_type::<contracts::RequestFrame, magistr_workflow::request::broadcast::RequestFrame>();
    same_type::<magistr_domain::workflow::CheckpointReason, magistr_engine::CheckpointReason>();
}
