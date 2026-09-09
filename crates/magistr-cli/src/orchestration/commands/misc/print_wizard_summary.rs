use crate::detect;
use crate::types;
use crate::utils::format as fmt;

/// Print wizard intelligence summary: common setup detection + skippable categories.
pub fn print_wizard_summary(smart: &types::SmartStack) {
    let summary = detect::wizard::wizard_summary(smart);

    // Show common setup archetype if detected
    if let Some(setup) = &summary.common_setup {
        eprintln!(
            "{} Detected archetype: {}",
            fmt::cyan("→"),
            fmt::bold(setup.name)
        );
        eprintln!("  {}", setup.description);
        eprintln!();
    }

    // Show what's auto-confirmed vs needs review
    if !summary.confirmed.is_empty() {
        let confirmed_labels: Vec<&str> = summary.confirmed.iter().map(|c| c.category).collect();
        eprintln!(
            "{}",
            fmt::success(&format!(
                "Auto-confirmed (high confidence): {}",
                confirmed_labels.join(", ")
            ))
        );
    }
    if !summary.needs_review.is_empty() {
        let review_labels: Vec<String> = summary
            .needs_review
            .iter()
            .map(|r| format!("{} ({})", r.category, r.confidence))
            .collect();
        eprintln!(
            "{} Needs review: {}",
            fmt::yellow("?"),
            review_labels.join(", ")
        );
    }
    eprintln!();
}
