/// The operation whose permission or availability prevented dispatch.
/// Providers map their wire capabilities to these use-case requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    StartSession,
    ResumeSession,
    Prompt,
    CancelTurn,
    CloseSession,
    ResolvePermission,
    RunChecks,
    MutateWorkflow,
    ConfigureProject,
    ReadProjection,
}
