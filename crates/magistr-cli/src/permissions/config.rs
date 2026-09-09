//! Permission configuration — re-exports from split submodules.

pub use super::auto_allow_rule::AutoAllowRule;
pub use super::config_path::config_path;
pub use super::load_config::load_config;
pub use super::permission_config::PermissionConfig;
pub use super::permission_profile::PermissionProfile;
pub use super::save_config::save_config;

#[cfg(test)]
#[path = "config.tests.rs"]
mod tests;
