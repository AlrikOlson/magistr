use super::*;
use crate::{Observed, ReadErrorKind, ReadSource};

#[test]
fn workstream_projection_and_worker_failure_keep_family_provenance() {
    let snapshot = NativeSnapshot {
        plan: ReadState::Current(Observed {
            value: PlanData::default(),
            source: ReadSource::Missing,
        }),
        run: ReadState::Current(Observed {
            value: RunData::default(),
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
        failed.plan,
        ReadState::Stale {
            last: Observed {
                source: ReadSource::Missing,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        failed.run,
        ReadState::Stale {
            last: Observed {
                source: ReadSource::Memory,
                ..
            },
            ..
        }
    ));
    assert_eq!(failed.requests, ReadState::Unavailable(error.clone()));
    assert_eq!(failed.decisions, ReadState::Unavailable(error));
    assert!(failed.workstreams().current().is_err());
}
