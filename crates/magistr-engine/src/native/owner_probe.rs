//! One bounded handshake that asks an endpoint who owns it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::attach_error::AttachError;
use super::attach_kind::AttachKind;
use super::build_identity::BuildIdentity;
use super::caller_identity::CallerIdentity;
use super::owner_identity::OwnerIdentity;
use super::owner_liveness::OwnerLiveness;
use super::owner_wire::{Hello, OWNER_PROTOCOL_VERSION};
use super::workspace_socket_path::workspace_socket_path;

/// A diagnostic client of an owner endpoint: it connects, sends a `probe`
/// `Hello`, reads the greeting and — when welcomed — the owner's report,
/// and closes. It takes no lock, opens no store, mints no generation and
/// is not counted among the owner's sessions. Everything it learns comes
/// from the owner's own answer, typed as [`OwnerLiveness`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerProbe {
    /// The endpoint the probe derived and connected to.
    pub endpoint: PathBuf,
    /// What the handshake established.
    pub liveness: OwnerLiveness,
}

impl OwnerProbe {
    /// Probe the owner of the workspace at `root`, at the endpoint every
    /// workspace client derives ([`workspace_socket_path`]). A root that
    /// cannot be canonicalized or an endpoint that cannot be derived is
    /// reported as [`OwnerLiveness::Unanswered`] at the root itself.
    #[must_use]
    pub fn workspace(root: &Path, caller: CallerIdentity, deadline: Duration) -> Self {
        let root = match root.canonicalize() {
            Ok(root) => root,
            Err(error) => {
                return Self {
                    endpoint: root.to_path_buf(),
                    liveness: OwnerLiveness::Unanswered {
                        reason: format!("workspace root cannot be resolved: {error}"),
                    },
                };
            }
        };
        match workspace_socket_path(&root) {
            Ok(path) => Self::at(&path, OwnerIdentity::Workspace { root }, caller, deadline),
            Err(error) => Self {
                endpoint: root,
                liveness: OwnerLiveness::Unanswered {
                    reason: format!("no workspace endpoint can be derived: {error}"),
                },
            },
        }
    }

    /// Probe the endpoint at `path` as a client of `identity`. The deadline
    /// bounds the connect, the hello, the greeting and the report together.
    #[must_use]
    pub fn at(
        path: &Path,
        identity: OwnerIdentity,
        caller: CallerIdentity,
        deadline: Duration,
    ) -> Self {
        let hello = Hello {
            protocol: OWNER_PROTOCOL_VERSION,
            build: BuildIdentity::this_build(),
            identity,
            attach: AttachKind::Probe,
            context: None,
            caller,
        };
        let liveness = match super::owner_client::probe_line(path, &hello, deadline) {
            Ok(report) => OwnerLiveness::Live(Box::new(report)),
            Err(AttachError::Refused(refusal)) => OwnerLiveness::Refused(refusal),
            Err(AttachError::Deadline(deadline)) => OwnerLiveness::Silent { deadline },
            Err(AttachError::Unavailable(error))
                if error.kind() == std::io::ErrorKind::NotFound =>
            {
                OwnerLiveness::Absent
            }
            Err(error) => OwnerLiveness::Unanswered {
                reason: error.to_string(),
            },
        };
        Self {
            endpoint: path.to_path_buf(),
            liveness,
        }
    }
}
