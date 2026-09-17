use super::*;
use crate::scaffold::manifest::content_hash;
use crate::scaffold::{AdminOperation, ChangePlan, Refusal};

#[test]
fn the_expected_state_is_the_hash_of_what_was_read_or_absence() {
    let created = PlannedChange::merged("a.md", None, "new".to_owned());
    assert_eq!(created.expected(), Expected::Absent);
    assert_eq!(created.expected().label(), "absent");

    let overwritten = PlannedChange::merged("a.md", Some("old".to_owned()), "new".to_owned());
    assert_eq!(
        overwritten.expected(),
        Expected::Content {
            hash: content_hash("old")
        }
    );
    assert_eq!(overwritten.expected().label(), content_hash("old"));
    assert!(overwritten.diff.is_some());
}

#[test]
fn every_action_has_one_intent_and_a_reason_when_it_is_not_applied() {
    let create = PlannedChange::merged("a.md", None, "new".to_owned());
    assert_eq!(create.intent(), Intent::Apply);
    assert_eq!(create.reason(), None);

    let unchanged = PlannedChange::merged("a.md", Some("same".to_owned()), "same".to_owned());
    assert_eq!(unchanged.action, FileAction::Unchanged);
    assert_eq!(unchanged.intent(), Intent::Skip);
    assert_eq!(unchanged.reason().as_deref(), Some("already up to date"));

    let removal = PlannedChange::removal("a.md", "old".to_owned(), Ownership::Generated);
    assert_eq!(removal.action, FileAction::Remove);
    assert_eq!(removal.intent(), Intent::Apply);
    assert!(removal.new_content.is_empty());

    let customized =
        PlannedChange::preserved("a.md", Some("mine".to_owned()), Ownership::Customized);
    assert_eq!(customized.intent(), Intent::Skip);
    assert_eq!(
        customized.reason().as_deref(),
        Some("customized after magistr wrote it")
    );
    let deleted = PlannedChange::preserved("a.md", None, Ownership::User);
    assert_eq!(
        deleted.reason().as_deref(),
        Some("removed by the user; not recreated")
    );

    let refused = PlannedChange::refused(
        "a.md",
        Refusal::CanonicalUnavailable {
            source: "templates/a.md".to_owned(),
            cause: "missing".to_owned(),
        },
        Some("current".to_owned()),
        Ownership::Canonical,
    );
    assert_eq!(refused.intent(), Intent::Refuse);
    assert_eq!(refused.action.label(), "refused");
    assert_eq!(
        refused.reason().as_deref(),
        Some("canonical input templates/a.md is unavailable: missing")
    );
    // A refused path keeps its current bytes: nothing else is proposed.
    assert_eq!(refused.new_content, "current");
}

#[test]
fn a_plan_counts_what_it_applies_skips_and_refuses() {
    let mut plan = ChangePlan::new(AdminOperation::Repair);
    plan.files
        .push(PlannedChange::merged("a.md", None, "a".to_owned()));
    plan.files.push(PlannedChange::merged(
        "b.md",
        Some("b".to_owned()),
        "b".to_owned(),
    ));
    plan.files.push(PlannedChange::removal(
        "c.md",
        "c".to_owned(),
        Ownership::Internal,
    ));
    plan.files.push(PlannedChange::preserved(
        "d.md",
        Some("d".to_owned()),
        Ownership::User,
    ));
    plan.files.push(PlannedChange::refused(
        "e.md",
        Refusal::NotRegularFile,
        None,
        Ownership::Internal,
    ));
    assert_eq!(plan.operation.label(), "repair");
    assert_eq!(plan.applies(), 2);
    assert_eq!(plan.skips(), 2);
    assert_eq!(plan.refusals(), 1);
    assert_eq!(plan.creates(), 1);
    assert_eq!(plan.removes(), 1);
    assert_eq!(plan.unchanged(), 1);
    assert_eq!(plan.preserved(), 1);
    assert_eq!(
        plan.applied().map(|f| f.path.as_str()).collect::<Vec<_>>(),
        ["a.md", "c.md"]
    );
    assert_eq!(
        plan.refused(),
        vec![("e.md", "target is not a regular file".to_owned())]
    );
    assert!(
        plan.get("d.md")
            .is_some_and(|f| f.ownership == Ownership::User)
    );
}
