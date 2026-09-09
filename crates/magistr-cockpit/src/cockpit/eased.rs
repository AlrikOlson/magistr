//! A value that glides to its target instead of jumping there.

/// A fraction that eases from where it was to where it is asked to be,
/// measured in animation steps ([`Model::now`](super::Model::now)) so it
/// stays a pure function of the model.
///
/// The step is a **rational** number of ticks, not a whole one. A
/// terminal only ever asks at whole steps, so its curve is the one it
/// always was, bit for bit; a window asks at `ticks + phase` on every
/// display frame, which is what turns a seven-step glide from seven
/// visible positions into as many as the display can show.
///
/// The first target it is ever given seeds it in place — a startup
/// state is not a change, and the settled frame is the one a snapshot
/// pins. Every later target starts a glide from wherever the value is
/// at that moment, so a retarget mid-glide never snaps. The curve is
/// ease-out cubic: fast off the mark, settling gently.
///
/// A glide spans [`Eased::STEPS`] unless the value was made with
/// [`Eased::spanning`] — a scroll's paint has less far to go than a
/// fill and earns a shorter run, on the same curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eased {
    from: f32,
    to: f32,
    /// The step the current glide started at; `None` until seeded.
    start: Option<f32>,
    /// Steps one glide of this value takes.
    steps: f32,
}

impl Eased {
    /// Steps a glide takes: about 400 ms at the runtime's tick.
    pub const STEPS: u64 = 7;

    /// Unseeded: the first target lands at once.
    #[must_use]
    pub const fn new() -> Self {
        Self::spanning(Self::STEPS)
    }

    /// Unseeded, and gliding over `steps` rather than [`Eased::STEPS`]
    /// — the same ease-out cubic run in fewer (or more) ticks. Zero is
    /// read as one: a glide with no duration would divide by it.
    #[must_use]
    pub const fn spanning(steps: u64) -> Self {
        let steps = if steps == 0 { 1 } else { steps };
        Self {
            from: 0.0,
            to: 0.0,
            start: None,
            steps: steps as f32,
        }
    }

    /// Whether a target has been given yet.
    #[must_use]
    pub const fn is_seeded(&self) -> bool {
        self.start.is_some()
    }

    /// The target.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.to
    }

    /// Ask for `to` at step `now`. The first ask seeds in place; a repeat
    /// of the current target changes nothing; any other target starts a
    /// glide from the value at `now` — or, with `animate` off, lands at
    /// once.
    pub fn retarget(&mut self, to: f32, now: f32, animate: bool) {
        let Some(_) = self.start else {
            *self = Self {
                from: to,
                to,
                start: Some(now),
                steps: self.steps,
            };
            return;
        };
        if (to - self.to).abs() <= f32::EPSILON {
            return;
        }
        let here = if animate { self.at(now) } else { to };
        *self = Self {
            from: here,
            to,
            start: Some(now),
            steps: self.steps,
        };
    }

    /// Glide from `from` to `to`, starting at step `now`.
    ///
    /// Where [`Eased::retarget`] reads the glide's starting value off
    /// its own curve, this one is *told* it — which is what a value
    /// measured against a frame that moved underneath it needs, since
    /// "where it is" is no longer where the last glide left it. A
    /// scrolled row offset is the case in point: the rows it counts
    /// from arrive and depart while the glide runs. With `animate` off,
    /// or with nowhere to travel, it lands at once.
    pub const fn glide_from(&mut self, from: f32, to: f32, now: f32, animate: bool) {
        *self = Self {
            from: if animate { from } else { to },
            to,
            start: Some(now),
            steps: self.steps,
        };
    }

    /// The value at step `now`.
    #[must_use]
    pub fn at(&self, now: f32) -> f32 {
        let Some(start) = self.start else {
            return self.to;
        };
        let elapsed = (now - start).max(0.0);
        if elapsed >= self.steps {
            return self.to;
        }
        let t = elapsed / self.steps;
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    /// Whether the value is still gliding at step `now`.
    #[must_use]
    pub fn is_settling(&self, now: f32) -> bool {
        self.start.is_some_and(|start| {
            (self.to - self.from).abs() > f32::EPSILON && (now - start).max(0.0) < self.steps
        })
    }
}

impl Default for Eased {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "eased.tests.rs"]
mod tests;
