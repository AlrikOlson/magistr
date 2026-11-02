//! People: identity and role-based access control.
//!
//! Multi-user foundations: who is running magistr ([`Identity`]), the people
//! roster ([`PeopleConfig`] at `.magistr/people.json`), the five-role model
//! ([`PersonRole`]) with its capability matrix ([`Capability`]), and role
//! resolution ([`resolve_role`]).
//!
//! Identity is self-asserted by default (git-config style): this module
//! does attribution, not authentication. Solo mode is the absence of
//! `.magistr/people.json` — the local user resolves to `Owner` and nothing
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
mod people_config;
mod person;
mod person_role;
mod resolve_role;
mod roster_access;
mod roster_change;
mod session_access;
mod session_action;

pub use capability::Capability;
pub use identity::Identity;
pub use people_config::PeopleConfig;
pub use person::Person;
pub use person_role::PersonRole;
pub use resolve_role::resolve_role;
pub use roster_access::roster_access;
pub use roster_change::RosterChange;
pub use session_access::session_access;
pub use session_action::SessionAction;
