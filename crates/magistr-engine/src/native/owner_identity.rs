//! Identity at a local owner endpoint, independent of its transport.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(any(unix, windows))]
use super::AttachRefusal;
use super::WriterRole;

/// Workflow ownership follows shared project/store identity; workspace
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
    #[cfg(any(unix, windows))]
    pub(crate) fn admit(&self, offered: &Self) -> Result<(), AttachRefusal> {
        match (self, offered) {
            (
                Self::Workflow {
                    project_id: expected,
                    role: _,
                },
                Self::Workflow {
                    project_id: offered,
                    role: _,
                },
            ) => {
                if expected != offered {
                    return Err(AttachRefusal::Project {
                        expected: expected.clone(),
                        offered: offered.clone(),
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
