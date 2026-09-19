//! Captured project, preset, provider, plugin, workspace, machine and
//! session inputs.
//!
//! [`ProjectInputs`] is the composition-time counterpart of the native
//! workflow's `NativeConfig`: the host reads the environment once
//! (`from_env`) or injects a lookup (`from_lookup`), and every use case
//! afterwards resolves paths and loads files from the captured set instead
//! of consulting the process environment again. Each captured setting
//! carries a non-secret [`InputSource`]; loaded files report an
//! [`InputOrigin`] so an absent file, an unlocatable path and an unreadable
//! file stay distinguishable from a value that was actually read.
//!
//! The machine inputs are the user's home and the directories derived from
//! it (configuration, data, state, the Codex home), the identity override,
//! the OS user and the CI flag. The session inputs are the goal mode and the
//! budget exports: a host that decides the governed budget binds it here
//! with `with_budget`, and a child process that inherits the exports
//! captures them from its own environment. Both are read at the same
//! composition points as everything else, so no use case reads them again.

mod captured_input;
mod input_origin;
mod input_source;
mod project_inputs;

pub use captured_input::CapturedInput;
pub use input_origin::InputOrigin;
pub use input_source::InputSource;
pub use project_inputs::ProjectInputs;
