//! Which build of Magistr a handshake party is.

use serde::{Deserialize, Serialize};

use super::attach_refusal::AttachRefusal;

/// The identity a client offers in its `Hello` and an owner compares with its
/// own: the package version, the contract digest the build script computed
/// over this crate, its path dependencies and the workspace lock file, the
/// build profile and the target triple. It is fixed when the binary is
/// compiled, so it is independent of the package version alone, of any
/// workspace source snapshot and of the owner generation an admission mints.
///
/// The compatibility rule is exact build: every field equal. Two binaries
/// compiled from one source tree at one profile for one target carry the
/// same identity wherever and whenever they were built, so a copied install
/// attaches to a running owner of the same build; a rebuilt tree, another
/// checkout, or the other profile of the same tree is refused with both
/// identities named. There is no equal-version relaxation: widening what
/// attaches is a deliberate change to the wire, made by editing this rule
/// under a new protocol version, never by an ambient override.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildIdentity {
    package: String,
    contract: String,
    profile: String,
    target: String,
}

impl BuildIdentity {
    /// The identity of the binary this code was compiled into.
    #[must_use]
    pub fn this_build() -> Self {
        Self {
            package: env!("CARGO_PKG_VERSION").to_owned(),
            contract: env!("MAGISTR_OWNER_CONTRACT_SOURCE").to_owned(),
            profile: env!("MAGISTR_OWNER_BUILD_PROFILE").to_owned(),
            target: env!("MAGISTR_OWNER_BUILD_TARGET").to_owned(),
        }
    }

    /// An identity spelled out field by field — what a fixture standing in
    /// for another build, or a diagnostic reading one back, holds. Nothing
    /// built this way is trusted more than a `Hello`: the owner compares,
    /// it never believes.
    #[must_use]
    pub fn new(
        package: impl Into<String>,
        contract: impl Into<String>,
        profile: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            package: package.into(),
            contract: contract.into(),
            profile: profile.into(),
            target: target.into(),
        }
    }

    /// The package version (`CARGO_PKG_VERSION`).
    #[must_use]
    pub fn package(&self) -> &str {
        &self.package
    }

    /// The contract digest: sixteen hex digits over the owner-wire source
    /// closure and the lock file.
    #[must_use]
    pub fn contract(&self) -> &str {
        &self.contract
    }

    /// `debug` or `release`.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// The target triple the binary was built for.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Whether an owner of this build serves a client of `offered`.
    ///
    /// # Errors
    /// [`AttachRefusal::Build`] naming both identities when any field
    /// differs.
    pub fn admits(&self, offered: &Self) -> Result<(), AttachRefusal> {
        if self == offered {
            Ok(())
        } else {
            Err(AttachRefusal::Build {
                served: Box::new(self.clone()),
                offered: Box::new(offered.clone()),
            })
        }
    }
}

impl std::fmt::Display for BuildIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "magistr {} (contract {}, {}, {})",
            self.package, self.contract, self.profile, self.target
        )
    }
}

#[cfg(test)]
#[path = "build_identity.tests.rs"]
mod tests;
