use super::*;

fn iteration_state(
    iteration: u32,
    max_iterations: u32,
    checked: u32,
    total: u32,
) -> IterationState {
    IterationState {
        iteration,
        max_iterations,
        last_status: IterationStatus::None,
        spec_checked: checked,
        spec_total: total,
    }
}

#[test]
fn spec_completion_requires_a_nonempty_spec_and_all_items_checked() {
    for (checked, total, complete) in [
        (0, 0, false),
        (1, 0, false),
        (0, 1, false),
        (1, 2, false),
        (2, 2, true),
        (3, 2, true),
        (u32::MAX, u32::MAX, true),
    ] {
        assert_eq!(
            iteration_state(0, 10, checked, total).is_spec_complete(),
            complete,
            "spec progress {checked}/{total}"
        );
    }
}

#[test]
fn iteration_ceiling_is_reached_at_equality_and_when_restored_past_it() {
    for (iteration, limit, reached) in [
        (0, 0, true),
        (0, 1, false),
        (1, 2, false),
        (2, 2, true),
        (3, 2, true),
        (u32::MAX, u32::MAX, true),
    ] {
        assert_eq!(
            iteration_state(iteration, limit, 0, 0).is_iteration_limit_reached(),
            reached,
            "iteration {iteration}, limit {limit}"
        );
    }
}
