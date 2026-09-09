use std::path::Path;

use serde_json::Value;

use crate::error::Gui2Error;
use crate::medium::Medium;
use crate::region::Region;

/// The regions of a capture the parity diff does not measure, and the share of what is
/// left that a view may differ by — read from `tests/mocks/NN.mask.json`.
///
/// A rect is excluded because it is *live* — a clock, a token count, a sigil derived
/// per project, a browser frame's traffic lights — or because the CPU tier cannot draw
/// it yet, and every rect says which in its `why`, so a mask is a list of admissions
/// rather than a way to make a view pass. A rect may name one medium; the browser frame
/// is masked in both.
///
/// The file is read strictly: an unknown key, a missing side or a medium that is
/// neither word is an error rather than a rect that silently excludes nothing — a mask
/// that was mistyped into permissiveness would be exactly the review-by-judgement the
/// gate replaces.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask {
    threshold: f32,
    excluded: Vec<Excluded>,
    regions: Vec<Region>,
}

/// One excluded rect.
#[derive(Clone, Debug, PartialEq)]
struct Excluded {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    why: String,
    media: Option<Medium>,
}

impl Excluded {
    fn covers(&self, x: u32, y: u32, medium: Medium) -> bool {
        self.media.is_none_or(|m| m == medium)
            && x >= self.x
            && y >= self.y
            && x < self.x + self.w
            && y < self.y + self.h
    }
}

impl Mask {
    /// Where every view starts: 1.5% of its unmasked pixels, tightened
    /// as views land.
    pub const DEFAULT_THRESHOLD: f32 = 0.015;

    /// No exclusions and the default threshold — what a view with no
    /// mask file is diffed under.
    #[must_use]
    pub fn none() -> Self {
        Self {
            threshold: Self::DEFAULT_THRESHOLD,
            excluded: Vec::new(),
            regions: Vec::new(),
        }
    }

    /// The mask at `path`, or [`Mask::none`] when there is no file.
    ///
    /// # Errors
    ///
    /// [`Gui2Error::Mask`] when the file exists and cannot be read or
    /// does not parse strictly.
    pub fn load(path: &Path) -> Result<Self, Gui2Error> {
        if !path.exists() {
            return Ok(Self::none());
        }
        let text = std::fs::read_to_string(path).map_err(|e| Gui2Error::Mask {
            path: path.to_path_buf(),
            error: e.to_string(),
        })?;
        Self::parse(&text).map_err(|error| Gui2Error::Mask {
            path: path.to_path_buf(),
            error,
        })
    }

    /// The mask in `json`: `{"threshold": 0.015, "excluded": [{"x", "y",
    /// "w", "h", "why", "media"?}], "regions": [{"name", "x", "y", "w",
    /// "h", "budget"}]}`. All three top-level keys are optional;
    /// nothing else is accepted.
    ///
    /// # Errors
    ///
    /// A message naming the first thing that is not the format.
    pub fn parse(json: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let object = value.as_object().ok_or("the mask is not a JSON object")?;
        let mut mask = Self::none();
        for (key, value) in object {
            match key.as_str() {
                "threshold" => {
                    let threshold = value
                        .as_f64()
                        .filter(|t| (0.0..=1.0).contains(t))
                        .ok_or("threshold must be a number in 0..=1")?;
                    mask.threshold = threshold as f32;
                }
                "excluded" => {
                    let rects = value.as_array().ok_or("excluded must be an array")?;
                    mask.excluded = rects
                        .iter()
                        .enumerate()
                        .map(|(i, rect)| {
                            Excluded::parse(rect).map_err(|e| format!("excluded[{i}]: {e}"))
                        })
                        .collect::<Result<_, _>>()?;
                }
                "regions" => {
                    let rects = value.as_array().ok_or("regions must be an array")?;
                    mask.regions = rects
                        .iter()
                        .enumerate()
                        .map(|(i, rect)| {
                            parse_region(rect).map_err(|e| format!("regions[{i}]: {e}"))
                        })
                        .collect::<Result<_, _>>()?;
                    for (i, region) in mask.regions.iter().enumerate() {
                        if mask.regions[..i].iter().any(|r| r.name == region.name) {
                            return Err(format!("regions: duplicate name {:?}", region.name));
                        }
                    }
                }
                other => return Err(format!("unknown key {other:?}")),
            }
        }
        Ok(mask)
    }

    /// The share of unmasked pixels a view may differ by.
    #[must_use]
    pub const fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Whether the pixel at `(x, y)` is excluded from the diff for
    /// `medium`.
    #[must_use]
    pub fn covers(&self, x: u32, y: u32, medium: Medium) -> bool {
        self.excluded.iter().any(|r| r.covers(x, y, medium))
    }

    /// The named regions the diff measures on their own — the primary
    /// signal; the frame threshold is the backstop.
    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    /// Every exclusion that applies to `medium`, as `(x, y, w, h, why)`
    /// — what the gate prints under a view's row.
    pub fn exclusions(&self, medium: Medium) -> impl Iterator<Item = (u32, u32, u32, u32, &str)> {
        self.excluded
            .iter()
            .filter(move |r| r.media.is_none_or(|m| m == medium))
            .map(|r| (r.x, r.y, r.w, r.h, r.why.as_str()))
    }
}

impl Excluded {
    fn parse(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("not an object")?;
        let side = |name: &str| -> Result<u32, String> {
            object
                .get(name)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| format!("{name} must be a non-negative integer"))
        };
        let mut rect = Self {
            x: side("x")?,
            y: side("y")?,
            w: side("w")?,
            h: side("h")?,
            why: String::new(),
            media: None,
        };
        if rect.w == 0 || rect.h == 0 {
            return Err("w and h must be positive".to_owned());
        }
        for (key, value) in object {
            match key.as_str() {
                "x" | "y" | "w" | "h" => {}
                "why" => {
                    rect.why = value
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .ok_or("why must be a non-empty string")?
                        .to_owned();
                }
                "media" => {
                    let word = value.as_str().ok_or("media must be \"night\" or \"day\"")?;
                    rect.media =
                        Some(Medium::parse(word).ok_or("media must be \"night\" or \"day\"")?);
                }
                other => return Err(format!("unknown key {other:?}")),
            }
        }
        if rect.why.is_empty() {
            return Err("a rect must say why it is excluded".to_owned());
        }
        Ok(rect)
    }
}

/// One region, read as strictly as an excluded rect: every side named,
/// a non-empty name, a budget in `0..=1`, nothing else accepted.
fn parse_region(value: &Value) -> Result<Region, String> {
    let object = value.as_object().ok_or("not an object")?;
    let side = |name: &str| -> Result<u32, String> {
        object
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("{name} must be a non-negative integer"))
    };
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or("name must be a non-empty string")?
        .to_owned();
    let budget = object
        .get("budget")
        .and_then(Value::as_f64)
        .filter(|b| (0.0..=1.0).contains(b))
        .ok_or("budget must be a number in 0..=1")?;
    let region = Region {
        name,
        x: side("x")?,
        y: side("y")?,
        w: side("w")?,
        h: side("h")?,
        budget: budget as f32,
    };
    if region.w == 0 || region.h == 0 {
        return Err("w and h must be positive".to_owned());
    }
    if let Some(other) = object
        .keys()
        .find(|k| !matches!(k.as_str(), "name" | "x" | "y" | "w" | "h" | "budget"))
    {
        return Err(format!("unknown key {other:?}"));
    }
    Ok(region)
}

#[cfg(test)]
#[path = "mask.tests.rs"]
mod tests;
