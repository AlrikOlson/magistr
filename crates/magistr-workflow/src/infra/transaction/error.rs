//! A refused domain operation stays distinct from a storage failure.

use magistr_workflow_contracts::WriteError;

#[derive(Debug)]
pub enum TransactionError<E> {
    Rejected(E),
    Store(WriteError),
}
