use std::collections::BTreeSet;

use super::*;

/// Sixteen names and no others, in declaration order: a seventeenth
/// variant would have to be added to `ALL` (the array's length is the
/// refusal) and to every table below (each `match` is exhaustive), and
/// this test would then fail on the count.
#[test]
fn sixteen_named_motions_and_no_others() {
    assert_eq!(MotionName::ALL.len(), 16);
    let words: Vec<&str> = MotionName::ALL.iter().map(|name| name.word()).collect();
    assert_eq!(
        words,
        [
            "sweep",
            "arrive",
            "breathe",
            "twinkle",
            "ready-pulse",
            "ignite",
            "edge-run",
            "wake",
            "toast · counter",
            "rise",
            "born",
            "radar",
            "caret · blink",
            "chrome fade",
            "skeleton",
            "parallax",
        ]
    );
    let distinct: BTreeSet<MotionName> = MotionName::ALL.into_iter().collect();
    assert_eq!(distinct.len(), 16, "no name is listed twice");
    for name in MotionName::ALL {
        assert!(!name.means().is_empty(), "{name:?} means something");
        assert!(!name.by_night().is_empty(), "{name:?} has a night column");
    }
}

/// Section III's day column: every motion has a day form but the
/// twinkle, whose column reads **none — ink is still**.
#[test]
fn twinkle_is_absent_by_day_and_every_other_motion_is_not() {
    for name in MotionName::ALL {
        assert_eq!(
            name.by_day().is_none(),
            name == MotionName::Twinkle,
            "{name:?}'s day column"
        );
        assert_eq!(name.by(ThemeMode::Dark), Some(name.by_night()));
        assert_eq!(name.by(ThemeMode::Light), name.by_day());
    }
    assert_eq!(MotionName::Twinkle.by(ThemeMode::Light), None);
}

/// Partial animations remain distinguishable from complete ones.
#[test]
fn incomplete_motions_are_named() {
    let unshipped: Vec<MotionName> = MotionName::ALL
        .into_iter()
        .filter(|name| !name.is_shipped())
        .collect();
    assert_eq!(
        unshipped,
        [
            MotionName::Sweep,
            MotionName::Wake,
            MotionName::ToastCounter,
            MotionName::Caret,
            MotionName::Skeleton,
        ]
    );
}

/// The documented motion vocabulary and day/night behavior match the registry.
#[test]
fn the_architecture_doc_describes_each_motion() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reference/cockpit-modules.md");
    let doc = std::fs::read_to_string(&path).expect("the architecture doc is beside the crates");
    for name in MotionName::ALL {
        let word = format!("`{}`", name.word());
        let means = name.means();
        let night = name.by_night();
        let day = name.by_day().unwrap_or("none");
        assert!(
            doc.lines().any(|line| line.starts_with('|')
                && line.contains(&word)
                && line.contains(means)
                && line.contains(night)
                && line.contains(day)),
            "the doc describes {word}: {means}; night: {night}; day: {day}"
        );
    }
}

/// The hero is a function of the set: news leads texture, the ship
/// moment leads everything, and a still screen has none.
#[test]
fn the_hero_is_the_news_ahead_of_the_texture() {
    use MotionName::{Breathe, Ignite, Rise, Twinkle};
    assert_eq!(MotionName::hero([Twinkle, Breathe, Ignite]), Some(Ignite));
    assert_eq!(MotionName::hero([Twinkle, Rise]), Some(Rise));
    assert_eq!(
        MotionName::hero([Twinkle, Breathe]),
        Some(Breathe),
        "among the texture, the working thing leads"
    );
    assert_eq!(MotionName::hero([]), None);
    let ambient: Vec<MotionName> = MotionName::ALL
        .into_iter()
        .filter(|name| name.is_ambient())
        .collect();
    assert_eq!(
        ambient,
        [
            MotionName::Breathe,
            MotionName::Twinkle,
            MotionName::ReadyPulse,
            MotionName::Caret,
            MotionName::Skeleton,
        ]
    );
    for news in MotionName::ALL.into_iter().filter(|n| !n.is_ambient()) {
        for texture in &ambient {
            assert!(
                news.precedence() < texture.precedence(),
                "{news:?} leads {texture:?}"
            );
        }
    }
}
