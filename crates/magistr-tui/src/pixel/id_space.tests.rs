use super::*;

fn raw(ids: [ImageId; 2]) -> [u32; 2] {
    [ids[0].raw(), ids[1].raw()]
}

#[test]
fn the_first_space_counts_from_one_like_the_wire_tests_expect() {
    assert_eq!(IdSpace::FIRST.tag(), 0);
    assert_eq!(raw(IdSpace::FIRST.image_ids(0)), [1, 2]);
    assert_eq!(raw(IdSpace::FIRST.image_ids(1)), [3, 4]);
    assert_eq!(IdSpace::FIRST.placement(0).raw(), 1);
    assert_eq!(IdSpace::FIRST.placement(1).raw(), 2);
}

#[test]
fn a_tag_shifts_every_image_id_into_its_own_range() {
    let space = IdSpace::new(1);
    assert_eq!(raw(space.image_ids(0)), [4097, 4098]);
    assert_eq!(raw(space.image_ids(1)), [4099, 4100]);
    // Placements are per image, so they need no tag.
    assert_eq!(space.placement(0).raw(), 1);
}

#[test]
fn two_spaces_never_share_an_image_id() {
    let (a, b) = (IdSpace::new(7), IdSpace::new(8));
    let ids = |space: IdSpace| -> Vec<u32> {
        (0..IdSpace::SLOTS)
            .flat_map(|n| raw(space.image_ids(n)))
            .collect()
    };
    let (a, b) = (ids(a), ids(b));
    assert!(a.iter().all(|id| !b.contains(id)));
    assert_eq!(a.len(), 2 * IdSpace::SLOTS as usize);
}

#[test]
fn every_id_stays_under_the_24_bit_ceiling() {
    let last = IdSpace::new(IdSpace::TAGS - 1);
    assert_eq!(last.tag(), IdSpace::TAGS - 1);
    let [_, top] = raw(last.image_ids(IdSpace::SLOTS - 1));
    assert!(top < 1 << IdSpace::ID_BITS, "{top}");
    assert!(last.placement(IdSpace::SLOTS - 1).raw() < 1 << IdSpace::ID_BITS);
}

#[test]
fn a_tag_past_the_last_wraps_and_so_does_a_slot_past_the_last() {
    assert_eq!(IdSpace::new(IdSpace::TAGS + 3), IdSpace::new(3));
    let space = IdSpace::new(3);
    assert_eq!(space.image_ids(IdSpace::SLOTS), space.image_ids(0));
    assert_eq!(space.placement(IdSpace::SLOTS), space.placement(0));
}

#[test]
fn the_process_space_is_in_range() {
    let space = IdSpace::for_process();
    assert!(space.tag() < IdSpace::TAGS);
    let [first, _] = raw(space.image_ids(0));
    assert!((1..1 << IdSpace::ID_BITS).contains(&first));
}
