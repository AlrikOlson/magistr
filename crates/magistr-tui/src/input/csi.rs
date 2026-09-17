//! Splitting a CSI parameter string into numbers.

/// The parameters of one CSI sequence: `;`-separated fields, each a
/// `:`-separated list of optional numbers (`CSI 1;5:3 A` is
/// `[[1], [5, 3]]`; an empty field or sub-field is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Params {
    fields: Vec<Vec<Option<u32>>>,
}

impl Params {
    /// Parse the bytes between the CSI introducer (and any private
    /// marker) and the final byte.
    pub(crate) fn parse(bytes: &[u8]) -> Self {
        let fields = bytes
            .split(|&b| b == b';')
            .map(|field| field.split(|&b| b == b':').map(number).collect())
            .collect();
        Self { fields }
    }

    /// The first number of field `index`, if present and non-empty.
    pub(crate) fn get(&self, index: usize) -> Option<u32> {
        self.sub(index, 0)
    }

    /// Sub-field `sub` of field `index`, if present and non-empty.
    pub(crate) fn sub(&self, index: usize, sub: usize) -> Option<u32> {
        self.fields.get(index)?.get(sub).copied().flatten()
    }
}

/// A decimal number, or `None` for an empty field; overflow saturates.
fn number(digits: &[u8]) -> Option<u32> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(digits.iter().fold(0u32, |acc, &d| {
        acc.saturating_mul(10).saturating_add(u32::from(d - b'0'))
    }))
}

#[cfg(test)]
#[path = "csi.tests.rs"]
mod tests;
