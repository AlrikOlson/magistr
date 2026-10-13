//! Cost estimation from historical session data.

use super::cost_entry::CostEntry;
use super::estimate::CostEstimate;

/// Estimate the cost of a prompt based on historical data.
///
/// Uses past sessions (optionally filtered by project) to compute
/// average cost-per-turn, then estimates total cost based on the
/// expected number of turns for the prompt complexity.
///
/// Returns `None` if there's insufficient historical data (< 3 sessions).
pub fn estimate_cost(history: &[CostEntry], estimated_turns: Option<u32>) -> Option<CostEstimate> {
    let samples: Vec<_> = history
        .iter()
        .filter(|entry| entry.turns > 0)
        .filter_map(|entry| entry.cost_usd.map(|cost| (cost, entry.turns)))
        .collect();
    if samples.len() < 3 {
        return None;
    }

    let mut costs_per_turn: Vec<f64> = samples
        .iter()
        .map(|(cost, turns)| cost / f64::from(*turns))
        .collect();

    costs_per_turn.sort_by(|a, b| a.total_cmp(b));

    let avg_cost_per_turn = costs_per_turn.iter().sum::<f64>() / costs_per_turn.len() as f64;

    // Use p25 and p75 for the range
    let p25 = percentile(&costs_per_turn, 25);
    let p75 = percentile(&costs_per_turn, 75);

    // Estimate turns from prompt or use historical average
    let turns = estimated_turns.unwrap_or_else(|| {
        let avg_turns: f64 = samples
            .iter()
            .map(|(_, turns)| f64::from(*turns))
            .sum::<f64>()
            / samples.len() as f64;
        avg_turns.round() as u32
    });

    let turns_f64 = f64::from(turns);

    Some(CostEstimate {
        low: p25 * turns_f64,
        high: p75 * turns_f64,
        estimated_turns: turns,
        avg_cost_per_turn,
        sample_size: samples.len(),
    })
}

/// Compute the p-th percentile of a sorted slice.
fn percentile(sorted: &[f64], p: u32) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let rank = (f64::from(p) / 100.0) * (sorted.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let fraction = rank - rank.floor();

    if lower == upper {
        sorted[lower]
    } else {
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

#[cfg(test)]
#[path = "estimate_cost.tests.rs"]
mod tests;
