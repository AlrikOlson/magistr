//! Identity at a local owner endpoint, independent of its transport.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(unix)]
use super::AttachRefusal;
use super::WriterRole;

/// Workflow ownership follows project/store identity and role; workspace
/// ownership follows a canonical directory, so linked worktrees never share
/// their files, index or journal merely because their workflow identity does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OwnerIdentity {
    Workflow {
        project_id: String,
        role: WriterRole,
    },
    Workspace {
        root: PathBuf,
    },
}

impl OwnerIdentity {
    #[cfg(unix)]
    pub(crate) fn admit(&self, offered: &Self) -> Result<(), AttachRefusal> {
        match (self, offered) {
            (
                Self::Workflow {
                    project_id: expected,
                    role: expected_role,
                },
                Self::Workflow {
                    project_id: offered,
                    role: offered_role,
                },
            ) => {
                if expected != offered {
                    return Err(AttachRefusal::Project {
                        expected: expected.clone(),
                        offered: offered.clone(),
                    });
                }
                if expected_role != offered_role {
                    return Err(AttachRefusal::Role {
                        expected: *expected_role,
                        offered: *offered_role,
                    });
                }
                Ok(())
            }
            _ if self == offered => Ok(()),
            _ => Err(AttachRefusal::Identity {
                expected: self.clone(),
                offered: offered.clone(),
            }),
        }
    }
}
