//! Surface-reported geometry for one adjustable boundary, in physical pixels.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelAllocation {
    pub origin: f32,
    pub total: f32,
    pub first: f32,
    pub minimums: (f32, f32),
}

impl PanelAllocation {
    pub(crate) fn valid(self) -> bool {
        [
            self.origin,
            self.total,
            self.first,
            self.minimums.0,
            self.minimums.1,
        ]
        .into_iter()
        .all(|v| v.is_finite() && v >= 0.0)
            && self.total > 0.0
            && self.first <= self.total
    }
}
