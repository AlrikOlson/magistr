//! Preset management commands: list, create, test.

mod cmd_create;
mod cmd_list;
mod cmd_test;

pub use cmd_create::cmd_preset_create;
pub use cmd_list::cmd_preset_list;
pub use cmd_test::cmd_preset_test;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
