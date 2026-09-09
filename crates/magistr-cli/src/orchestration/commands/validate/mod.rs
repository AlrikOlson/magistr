//! Validation commands: validate (plain shell output).

mod cmd_validate;
mod gate_policy_violations;

pub use cmd_validate::cmd_validate;
pub use gate_policy_violations::gate_policy_violations;
