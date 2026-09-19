//! Team identity & role-based access control.
//!
//! Multi-user foundations: who is running magistr ([`Identity`]), the team
//! roster ([`TeamConfig`] at `.magistr/team.json`), the five-role model
//! ([`TeamRole`]) with its capability matrix ([`Capability`]), and role
//! resolution ([`resolve_role`]).
//!
//! Identity is self-asserted by default (git-config style): this module
//! does attribution, not authentication. Solo mode is the absence of
//! `.magistr/team.json` — the local user resolves to `Owner` and nothing
//! changes for existing single-user setups. The machine's say in all of
//! this (the identity override, the OS user, the CI flag, the state
//! directory) comes from the captured `ProjectInputs`, never from the
//! process environment at resolution time.
//!
//! A team that needs authentication sets `identity_provider` in the
//! roster and its members run `magistr login`; see [`auth`] for the
//! device-code flow, the cache and the refusal.

pub mod auth;
pub mod directory;

mod capability;
mod identity;
mod resolve_role;
mod roster_access;
mod roster_change;
mod session_access;
mod session_action;
mod team_config;
mod team_member;
mod team_role;

pub use capability::Capability;
pub use identity::Identity;
pub use resolve_role::resolve_role;
pub use roster_access::roster_access;
pub use roster_change::RosterChange;
pub use session_access::session_access;
pub use session_action::SessionAction;
pub use team_config::TeamConfig;
pub use team_member::TeamMember;
pub use team_role::TeamRole;
