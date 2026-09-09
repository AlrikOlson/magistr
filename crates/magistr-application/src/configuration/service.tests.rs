use super::*;
use crate::configuration::{ConfigurationOwner, ProjectSettings, ResolvedValue};
use crate::{
    Revision,
    test_support::{command, ready, run},
};
use alloc::boxed::Box;
use core::num::NonZeroU32;

struct FakeConfiguration {
    writes: usize,
    snapshot: ConfigurationSnapshot,
    result: Result<CommandReceipt, ApplicationError>,
}

impl ConfigurationPort for FakeConfiguration {
    fn read(&mut self, _: &ProjectId) -> crate::PortFuture<'_, ConfigurationSnapshot> {
        Box::pin(core::future::ready(Ok(self.snapshot.clone())))
    }

    fn update(&mut self, _: ConfigurationUpdate) -> crate::PortFuture<'_, CommandReceipt> {
        self.writes += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }

    fn setup(&mut self, _: SetupProject) -> crate::PortFuture<'_, CommandReceipt> {
        self.writes += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }
}

fn fixture() -> (ProjectScope, ConfigurationUpdate, FakeConfiguration) {
    let current = run("project", "run");
    let context = command(&current);
    let snapshot = ConfigurationSnapshot {
        project: current.project().clone(),
        revision: Revision(7),
        settings: ProjectSettings {
            project_name: ResolvedValue {
                value: "Project".into(),
                owner: ConfigurationOwner::Project,
            },
            max_iterations: ResolvedValue {
                value: NonZeroU32::new(10).unwrap(),
                owner: ConfigurationOwner::User,
            },
        },
    };
    let port = FakeConfiguration {
        writes: 0,
        snapshot,
        result: Ok(CommandReceipt {
            attempt: context.attempt.clone(),
            revision: Revision(8),
        }),
    };
    let request = ConfigurationUpdate {
        command: context,
        project_name: Some("Renamed project".into()),
        max_iterations: None,
    };
    (ProjectScope::new(current.project().clone()), request, port)
}

#[test]
fn reads_preserve_resolved_value_ownership() {
    let (scope, _, mut port) = fixture();
    let snapshot = ready(Configuration::new(&scope, &mut port).read()).unwrap();
    assert_eq!(
        snapshot.settings.max_iterations.owner,
        ConfigurationOwner::User
    );
    assert_eq!(
        snapshot.settings.project_name.owner,
        ConfigurationOwner::Project
    );
    assert_eq!(port.writes, 0);
}

#[test]
fn queries_reject_a_foreign_project_snapshot() {
    let (scope, _, mut port) = fixture();
    port.snapshot.project = run("foreign", "run").project().clone();
    assert!(matches!(
        ready(Configuration::new(&scope, &mut port).read()),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}

#[test]
fn empty_names_are_rejected_before_persistence() {
    let (scope, mut request, mut port) = fixture();
    request.project_name = Some(" ".into());
    assert!(matches!(
        ready(Configuration::new(&scope, &mut port).update(request)),
        Err(ApplicationError::InvalidRequest { .. })
    ));
    assert_eq!(port.writes, 0);
}

#[test]
fn stale_revision_conflict_is_forwarded_without_overwrite() {
    let (scope, request, mut port) = fixture();
    let error = ApplicationError::RevisionConflict {
        expected: Revision(7),
        actual: Revision(9),
    };
    port.result = Err(error.clone());
    assert_eq!(
        ready(Configuration::new(&scope, &mut port).update(request)),
        Err(error)
    );
    assert_eq!(port.writes, 1);
}

#[test]
fn setup_preserves_existing_configuration_refusal() {
    let (scope, update, mut port) = fixture();
    let request = SetupProject {
        command: update.command,
        project_name: "Project".into(),
        max_iterations: NonZeroU32::new(10).unwrap(),
    };
    let error = ApplicationError::InvalidRequest {
        reason: "project configuration already exists".into(),
    };
    port.result = Err(error.clone());
    assert_eq!(
        ready(Configuration::new(&scope, &mut port).setup(request)),
        Err(error)
    );
    assert_eq!(port.writes, 1);
}

#[test]
fn setup_rejects_a_foreign_command_before_creation() {
    let (scope, _, mut port) = fixture();
    let request = SetupProject {
        command: command(&run("foreign", "run")),
        project_name: "Project".into(),
        max_iterations: NonZeroU32::new(10).unwrap(),
    };
    assert!(matches!(
        ready(Configuration::new(&scope, &mut port).setup(request)),
        Err(ApplicationError::ProjectMismatch { .. })
    ));
    assert_eq!(port.writes, 0);
}

#[test]
fn updates_preserve_the_command_receipt() {
    let (scope, request, mut port) = fixture();
    let expected = port.result.clone();
    assert_eq!(
        ready(Configuration::new(&scope, &mut port).update(request)),
        expected
    );
    assert_eq!(port.writes, 1);
}
