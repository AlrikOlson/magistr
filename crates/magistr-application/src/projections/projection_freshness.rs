/// Read provenance reported by the projection adapter, not an authority lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFreshness {
    /// Read from the authority at query time; later commands still need revision checks.
    Authoritative,
    /// A cached observation; it may lag the authority.
    Cached,
    /// The adapter cannot establish freshness. Consumers must display that
    /// uncertainty and may not treat the snapshot as evidence for mutation.
    Unknown,
}
