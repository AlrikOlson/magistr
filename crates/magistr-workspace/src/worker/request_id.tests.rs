use super::*;

#[test]
fn concurrent_request_ids_are_unique_and_exhaustion_never_wraps() {
    let next = AtomicU64::new(u64::MAX - 1_000);
    let ids = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    let mut ids = Vec::new();
                    while let Ok(id) = next_request_id(&next) {
                        ids.push(id);
                    }
                    ids
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    let unique: std::collections::BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(ids.len(), 1_000);
    assert_eq!(unique.len(), ids.len());
    assert_eq!(unique.first(), Some(&(u64::MAX - 1_000)));
    assert_eq!(unique.last(), Some(&(u64::MAX - 1)));
    assert_eq!(next.load(Ordering::Relaxed), u64::MAX);
    assert!(next_request_id(&next).is_err());
    assert_eq!(next.load(Ordering::Relaxed), u64::MAX);
}
