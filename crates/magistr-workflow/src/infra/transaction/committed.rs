//! A successful mutation returns the state and result from the same commit.

use magistr_workflow_contracts::ReadSource;

#[derive(Debug)]
pub struct Committed<T, R> {
    pub state: T,
    pub value: R,
    pub source: ReadSource,
}
