/// A site deliberately left alone, with the reason, so it can be finished by hand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustUnrewritten {
    pub path: String,
    /// 1-based line.
    pub line: usize,
    pub reason: String,
}
