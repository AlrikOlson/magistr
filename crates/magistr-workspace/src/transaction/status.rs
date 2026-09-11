/// Whole-batch effect state; a mixed or unreadable filesystem is indeterminate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionStatus {
    Applied,
    Failed,
    Indeterminate,
}
