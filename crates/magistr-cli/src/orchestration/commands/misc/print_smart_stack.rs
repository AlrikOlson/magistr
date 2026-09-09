use super::*;
use crate::types;

/// Pretty-print a SmartStack with confidence scores and flagged categories.
pub fn print_smart_stack(smart: &types::SmartStack) {
    println!("Project: {}", smart.project_name);
    println!();
    println!(
        "  language:        {:<15} {:>3}  {}",
        smart.language.value,
        smart.language.confidence,
        flag_if_low(&smart.language),
    );
    println!(
        "  package_manager: {:<15} {:>3}  {}",
        smart.package_manager.value,
        smart.package_manager.confidence,
        flag_if_low(&smart.package_manager),
    );
    let fw_detail = smart
        .framework_detail
        .as_ref()
        .map(|d| format!(" ({})", d))
        .unwrap_or_default();
    println!(
        "  framework:       {:<15} {:>3}  {}{}",
        smart.framework.value,
        smart.framework.confidence,
        flag_if_low(&smart.framework),
        fw_detail,
    );
    println!(
        "  test_framework:  {:<15} {:>3}  {}",
        smart.test_framework.value,
        smart.test_framework.confidence,
        flag_if_low(&smart.test_framework),
    );
    println!(
        "  linter:          {:<15} {:>3}  {}",
        smart.linter.value,
        smart.linter.confidence,
        flag_if_low(&smart.linter),
    );
    println!(
        "  formatter:       {:<15} {:>3}  {}",
        smart.formatter.value,
        smart.formatter.confidence,
        flag_if_low(&smart.formatter),
    );
    println!(
        "  workspace:       {:<15} {:>3}  {}",
        smart.workspace.value,
        smart.workspace.confidence,
        flag_if_low(&smart.workspace),
    );

    let low = smart.low_confidence_categories();
    if !low.is_empty() {
        println!();
        println!(
            "  ⚠ Low confidence: {}. Consider manual override.",
            low.join(", ")
        );
    }
}
