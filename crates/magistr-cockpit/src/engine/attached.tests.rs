use super::*;

#[test]
fn a_fresh_handle_reads_detached_and_every_clone_sees_the_same_bit() {
    let attached = FeedAttached::new();
    assert!(!attached.is_attached());

    let other = attached.clone();
    attached.set(true);
    assert!(other.is_attached(), "the clone shares the bit");
    other.set(false);
    assert!(!attached.is_attached());
}
