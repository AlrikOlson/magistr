use super::*;
use crate::agent::multi::conflict::ConflictResolution;
use crate::agent::multi::partition::PartitionStrategy;

#[test]
fn all_presets() {
    let presets = OrchestratorPreset::all();
    assert_eq!(presets.len(), 6);
}

#[test]
fn preset_display_and_parse() {
    let cases = [
        (
            OrchestratorPreset::ResearchThenImplement,
            "research-then-implement",
        ),
        (OrchestratorPreset::ParallelTdd, "parallel-tdd"),
        (OrchestratorPreset::ReviewPair, "review-pair"),
    ];

    for (preset, expected_str) in &cases {
        assert_eq!(preset.to_string(), *expected_str);
        let parsed: OrchestratorPreset = expected_str.parse().unwrap();
        assert_eq!(&parsed, preset);
    }
}

#[test]
fn preset_short_names_parse() {
    assert_eq!(
        "research".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::ResearchThenImplement
    );
    assert_eq!(
        "tdd".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::ParallelTdd
    );
    assert_eq!(
        "review".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::ReviewPair
    );
}

#[test]
fn preset_invalid_parse() {
    let err = "nonexistent".parse::<OrchestratorPreset>().unwrap_err();
    assert!(err.contains("unknown preset"));
}

#[test]
fn preset_partition_strategies() {
    assert_eq!(
        OrchestratorPreset::ResearchThenImplement.partition_strategy(),
        PartitionStrategy::SplitByPhase
    );
    assert_eq!(
        OrchestratorPreset::ParallelTdd.partition_strategy(),
        PartitionStrategy::SplitByComponent
    );
    assert_eq!(
        OrchestratorPreset::ReviewPair.partition_strategy(),
        PartitionStrategy::SplitByFile
    );
}

#[test]
fn preset_conflict_resolutions() {
    assert_eq!(
        OrchestratorPreset::ResearchThenImplement.conflict_resolution(),
        ConflictResolution::Serialize
    );
    assert_eq!(
        OrchestratorPreset::ParallelTdd.conflict_resolution(),
        ConflictResolution::Merge
    );
    assert_eq!(
        OrchestratorPreset::ReviewPair.conflict_resolution(),
        ConflictResolution::LastWriteWins
    );
}

#[test]
fn preset_budgets_are_finite() {
    for preset in OrchestratorPreset::all() {
        let budget = preset.default_budget();
        assert!(
            budget.max_total_cost_usd > 0.0,
            "preset {preset} should have a cost limit"
        );
    }
}

#[test]
fn preset_descriptions_not_empty() {
    for preset in OrchestratorPreset::all() {
        assert!(!preset.description().is_empty());
    }
}

#[test]
fn research_preset_is_sequential() {
    let budget = OrchestratorPreset::ResearchThenImplement.default_budget();
    assert_eq!(budget.max_concurrent_workers, 1);
}

#[test]
fn parallel_tdd_allows_concurrency() {
    let budget = OrchestratorPreset::ParallelTdd.default_budget();
    assert!(budget.max_concurrent_workers > 1);
}

#[test]
fn archetype_presets_use_split_by_archetype() {
    assert_eq!(
        OrchestratorPreset::FullstackFeature.partition_strategy(),
        PartitionStrategy::SplitByArchetype
    );
    assert_eq!(
        OrchestratorPreset::ApiFirst.partition_strategy(),
        PartitionStrategy::SplitByArchetype
    );
    assert_eq!(
        OrchestratorPreset::TestAcrossStacks.partition_strategy(),
        PartitionStrategy::SplitByArchetype
    );
}

#[test]
fn archetype_preset_display_and_parse() {
    let cases = [
        (OrchestratorPreset::FullstackFeature, "fullstack-feature"),
        (OrchestratorPreset::ApiFirst, "api-first"),
        (OrchestratorPreset::TestAcrossStacks, "test-across-stacks"),
    ];
    for (preset, expected_str) in &cases {
        assert_eq!(preset.to_string(), *expected_str);
        let parsed: OrchestratorPreset = expected_str.parse().unwrap();
        assert_eq!(&parsed, preset);
    }
}

#[test]
fn archetype_preset_short_names_parse() {
    assert_eq!(
        "fullstack".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::FullstackFeature
    );
    assert_eq!(
        "api".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::ApiFirst
    );
    assert_eq!(
        "test-stacks".parse::<OrchestratorPreset>().unwrap(),
        OrchestratorPreset::TestAcrossStacks
    );
}

#[test]
fn api_first_is_sequential() {
    let budget = OrchestratorPreset::ApiFirst.default_budget();
    assert_eq!(budget.max_concurrent_workers, 1);
}

#[test]
fn fullstack_feature_allows_concurrency() {
    let budget = OrchestratorPreset::FullstackFeature.default_budget();
    assert!(budget.max_concurrent_workers > 1);
}
