use super::*;
use crate::scaffold::{ApplyFailure, Expected, Ownership, PlannedChange, Refusal};

fn receipt() -> ApplyReceipt {
    let mut receipt = ApplyReceipt::new(AdminOperation::Setup);
    let created = PlannedChange::merged("a.md", None, "new".to_owned());
    let unchanged = PlannedChange::merged("b.md", Some("same".to_owned()), "same".to_owned());
    let refused =
        PlannedChange::refused("c.md", Refusal::NotRegularFile, None, Ownership::Internal);
    let overwritten = PlannedChange::merged("d.md", Some("old".to_owned()), "new".to_owned());
    receipt
        .changes
        .push(ChangeReceipt::new(&created, ApplyOutcome::Applied));
    receipt.changes.push(ChangeReceipt::new(
        &unchanged,
        ApplyOutcome::Skipped {
            reason: unchanged.reason().unwrap_or_default(),
        },
    ));
    receipt.changes.push(ChangeReceipt::new(
        &refused,
        ApplyOutcome::Refused {
            reason: refused.reason().unwrap_or_default(),
        },
    ));
    receipt.changes.push(ChangeReceipt::new(
        &overwritten,
        ApplyOutcome::Failed(ApplyFailure::Stale {
            expected: overwritten.expected(),
            found: Expected::Absent,
        }),
    ));
    receipt
}

#[test]
fn a_receipt_counts_every_outcome_and_lists_what_landed_and_what_did_not() {
    let receipt = receipt();
    assert_eq!(receipt.applied_paths(), ["a.md"]);
    assert_eq!(
        receipt
            .failed()
            .map(|c| c.path.as_str())
            .collect::<Vec<_>>(),
        ["d.md"]
    );
    assert!(receipt.is_partial());
    assert_eq!(
        receipt.summary(),
        "setup: 1 applied, 1 skipped, 1 refused, 1 failed; manifest untouched"
    );
    assert_eq!(receipt.get("a.md").unwrap().expected, Expected::Absent);
    assert_eq!(
        receipt.get("b.md").unwrap().describe(),
        "b.md: skipped (already up to date)"
    );
    assert_eq!(
        receipt.get("c.md").unwrap().describe(),
        "c.md: refused (target is not a regular file)"
    );
    assert!(
        receipt
            .get("d.md")
            .unwrap()
            .describe()
            .starts_with("d.md: failed (changed after planning: expected "),
        "{}",
        receipt.get("d.md").unwrap().describe()
    );
}

#[test]
fn the_partial_error_names_the_failures_the_manifest_and_what_was_applied_first() {
    let mut receipt = receipt();
    receipt.manifest = ManifestOutcome::Failed {
        cause: "disk full".to_owned(),
    };
    let error = receipt.partial_error().expect("partial").to_string();
    assert!(
        error.starts_with("setup did not complete (setup: 1 applied"),
        "{error}"
    );
    assert!(error.contains("d.md: failed"), "{error}");
    assert!(error.contains("manifest not saved: disk full"), "{error}");
    assert!(error.contains("applied before that: a.md"), "{error}");
}

#[test]
fn a_complete_receipt_has_no_error_even_with_skips_and_refusals() {
    let mut receipt = receipt();
    receipt.changes.retain(|c| c.path != "d.md");
    receipt.manifest = ManifestOutcome::Saved;
    assert!(!receipt.is_partial());
    assert!(receipt.partial_error().is_none());
    assert_eq!(
        receipt.summary(),
        "setup: 1 applied, 1 skipped, 1 refused, 0 failed; manifest saved"
    );
}
