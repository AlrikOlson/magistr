use super::{ReadSegment, ReadUnit};
use serde::Serialize;

/// Ordered read output: a header followed by optional imports and source spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReadRow {
    Unit(ReadUnit),
    Lines(ReadSegment),
    Imports(ReadSegment),
}
