//! The units as the copy laws spell them — one function per kind.

/// Shared unit formatting for labels and meters.
///
/// Money labels use `2.07 USD`; meters use a unitless pair such as
/// `1.84 / 12.00`. Clocks use `04:12` and durations use their coarsest
/// single unit, such as `12 min`, `4 h`, or `2 d`.
///
/// Visible text uses product terms such as Roadmap, Projects, and Report.
/// The metaphor vocabulary below is reserved for renderer internals.
pub struct Copy;

impl Copy {
    /// The words a string on screen never says: the rendering's
    /// own nouns, which read absurd on the day chart. Lowercase, whole
    /// words; a sweep lowercases what it reads and splits on anything
    /// that is not a letter.
    pub const METAPHORS: [&'static str; 5] = ["sky", "star", "stars", "constellation", "weather"];

    /// `184` cents as `1.84` — the unitless amount a meter's pair is
    /// made of, and the capture's own spelling.
    #[must_use]
    pub fn amount(cents: u64) -> String {
        format!("{}.{:02}", cents / 100, cents % 100)
    }

    /// Money in a label — `2.07 USD`. The unit is the ISO code after
    /// the amount, never a `$` before it.
    #[must_use]
    pub fn usd(cents: u64) -> String {
        format!("{} USD", Self::amount(cents))
    }

    /// A meter's pair — `1.84 / 12.00` — unitless, because the meter's
    /// label carries the unit.
    #[must_use]
    pub fn pair(spent_cents: u64, limit_cents: u64) -> String {
        format!(
            "{} / {}",
            Self::amount(spent_cents),
            Self::amount(limit_cents)
        )
    }

    /// A clock — `04:12` — minutes and seconds, the minutes running
    /// past sixty rather than rolling into an hour, so a status row's
    /// meta column keeps one width.
    #[must_use]
    pub fn clock(secs: u64) -> String {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    /// A duration in its coarsest single unit — `2 d`, `4 h`,
    /// `12 min`, and under a minute `45 s` — so a cap stays a glance.
    #[must_use]
    pub fn duration(secs: u64) -> String {
        if secs >= 86_400 {
            format!("{} d", secs / 86_400)
        } else if secs >= 3_600 {
            format!("{} h", secs / 3_600)
        } else if secs >= 60 {
            format!("{} min", secs / 60)
        } else {
            format!("{secs} s")
        }
    }

    /// A token count at a glance — `162k` — the nearest thousand with
    /// its `k`, and under a thousand the count itself: the foot's
    /// `162k left` beside the exact `38,204 tok`, so the headroom is a
    /// size and the level a number.
    #[must_use]
    pub fn kilo(n: u64) -> String {
        if n < 1_000 {
            n.to_string()
        } else {
            format!("{}k", (n + 500) / 1_000)
        }
    }

    /// The first metaphor word in `text`, when it carries one — the
    /// word a sweep names in its failure. Case-insensitive, whole
    /// words: `Sky` and `sky` both answer, `skyline` does not.
    #[must_use]
    pub fn metaphor_in(text: &str) -> Option<&'static str> {
        text.split(|c: char| !c.is_alphabetic())
            .filter(|word| !word.is_empty())
            .find_map(|word| {
                let lower = word.to_lowercase();
                Self::METAPHORS
                    .into_iter()
                    .find(|metaphor| *metaphor == lower)
            })
    }
}

#[cfg(test)]
#[path = "copy.tests.rs"]
mod tests;
