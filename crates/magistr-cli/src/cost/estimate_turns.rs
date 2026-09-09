//! Turn estimation from prompt length.

/// Estimate turns from prompt length (heuristic).
///
/// Longer prompts tend to trigger more complex tasks with more turns.
/// This is a rough heuristic — callers should prefer historical data.
pub fn estimate_turns_from_prompt(prompt: &str) -> u32 {
    let words = prompt.split_whitespace().count();
    match words {
        0..=10 => 1,
        11..=30 => 2,
        31..=80 => 3,
        81..=200 => 5,
        _ => 8,
    }
}

#[cfg(test)]
#[path = "estimate_turns.tests.rs"]
mod tests;
