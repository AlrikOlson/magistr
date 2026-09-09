use super::*;

/// The same name is the same stream, whatever its case and margins.
#[test]
fn a_name_is_one_stream() {
    let mut a = Seed::of("magistr");
    let mut b = Seed::of("  Magistr ");
    for _ in 0..16 {
        assert_eq!(a.byte(), b.byte());
    }
}

/// Two names are two streams — a seed that did not tell projects apart
/// would draw every project the same mark.
#[test]
fn two_names_are_two_streams() {
    let mut a = Seed::of("magistr");
    let mut b = Seed::of("quicksilver");
    let drawn: Vec<(u8, u8)> = (0..8).map(|_| (a.byte(), b.byte())).collect();
    assert!(drawn.iter().any(|(x, y)| x != y));
}

/// A shuffle is a permutation, and a unit stays a share.
#[test]
fn a_shuffle_permutes_and_a_unit_is_a_share() {
    let mut seed = Seed::of("think-and-ship");
    let mut items = vec![0, 1, 2, 3, 4, 5];
    seed.shuffle(&mut items);
    let mut sorted = items.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5]);
    for _ in 0..32 {
        let unit = seed.unit();
        assert!((0.0..=1.0).contains(&unit));
    }
}
