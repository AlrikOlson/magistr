use super::*;
use crate::{Observed, ReadErrorKind, ReadSource};

#[test]
fn workstream_projection_and_worker_failure_keep_family_provenance() {
    let snapshot = NativeSnapshot {
        roadmap: ReadState::Current(Observed {
            value: RoadmapData::default(),
            source: ReadSource::Missing,
        }),
        ship: ReadState::Current(Observed {
            value: ShipData::default(),
            source: ReadSource::Memory,
        }),
        ..NativeSnapshot::default()
    };
    assert_eq!(
        snapshot.workstreams().current().unwrap().source,
        ReadSource::Missing
    );
    assert!(snapshot.needs_retry());

    let error = ReadError::new(
        ReadErrorKind::Interrupted,
        "The work could not be read. Try again.",
    );
    let failed = snapshot.failed(error.clone());
    assert!(matches!(
        failed.roadmap,
        ReadState::Stale {
            last: Observed {
                source: ReadSource::Missing,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        failed.ship,
        ReadState::Stale {
            last: Observed {
                source: ReadSource::Memory,
                ..
            },
            ..
        }
    ));
    assert_eq!(failed.signals, ReadState::Unavailable(error.clone()));
    assert_eq!(failed.think, ReadState::Unavailable(error));
    assert!(failed.workstreams().current().is_err());
}
