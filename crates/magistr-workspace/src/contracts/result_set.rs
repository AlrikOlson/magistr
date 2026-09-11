use super::{ErrorKind, Truncation, TruncationReason, WorkspaceError};

/// Ordered results. Cutting through this constructor cannot lose its manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSet<R> {
    rows: Vec<R>,
    truncation: Option<Truncation>,
}

impl<R> ResultSet<R> {
    #[must_use]
    pub fn complete(rows: Vec<R>) -> Self {
        Self {
            rows,
            truncation: None,
        }
    }

    /// A missing cursor on an actual cut is rejected before a response exists.
    /// This native constructor uses BudgetTooSmall for the unusable result set;
    /// it is not an incoming-envelope decoder or a token-budget selector.
    pub fn truncate(
        mut rows: Vec<R>,
        keep: usize,
        reason: TruncationReason,
        narrow: Vec<String>,
        cursor: String,
    ) -> Result<Self, WorkspaceError> {
        if keep >= rows.len() {
            return Ok(Self::complete(rows));
        }
        if cursor.is_empty() {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "a cut requires a non-empty cursor",
            ));
        }
        let dropped = rows.len() - keep;
        rows.truncate(keep);
        Ok(Self {
            rows,
            truncation: Some(Truncation {
                dropped,
                reason,
                narrow,
                cursor,
            }),
        })
    }

    /// Rows that are complete as rows but stand in for a cut made inside
    /// them (a head+tail window, a fitted diagnostics list): the manifest
    /// names what the cut dropped and how to widen it.
    pub fn with_manifest(
        rows: Vec<R>,
        dropped: usize,
        reason: TruncationReason,
        narrow: Vec<String>,
        cursor: String,
    ) -> Result<Self, WorkspaceError> {
        if dropped == 0 {
            return Ok(Self::complete(rows));
        }
        if cursor.is_empty() {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "a cut requires a non-empty cursor",
            ));
        }
        Ok(Self {
            rows,
            truncation: Some(Truncation {
                dropped,
                reason,
                narrow,
                cursor,
            }),
        })
    }

    #[must_use]
    pub fn rows(&self) -> &[R] {
        &self.rows
    }
    #[must_use]
    pub fn truncation(&self) -> Option<&Truncation> {
        self.truncation.as_ref()
    }
}
