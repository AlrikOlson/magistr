//! Reuse byte digests only for an unchanged, non-racy filesystem identity.
//! Selection is always walked again; cached hashes never stand in for a tree
//! generation, and platforms without a strong change stamp read bytes afresh.

use super::ManifestEntry;
use std::collections::{HashMap, VecDeque};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
#[path = "digest_cache.tests.rs"]
mod tests;

const CAPACITY: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}

#[cfg(unix)]
fn stamp(metadata: &Metadata) -> Option<Stamp> {
    use std::os::unix::fs::MetadataExt;
    Some(Stamp {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        mode: metadata.mode(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    })
}
#[cfg(not(unix))]
fn stamp(_metadata: &Metadata) -> Option<Stamp> {
    None
}

struct Entry {
    stamp: Stamp,
    record: ManifestEntry,
    reusable: bool,
}
#[derive(Default)]
struct Cache {
    entries: HashMap<PathBuf, Entry>,
    order: VecDeque<PathBuf>,
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

pub(super) fn get(path: &Path, metadata: &Metadata) -> Option<ManifestEntry> {
    let stamp = stamp(metadata)?;
    let cache = cache().lock().ok()?;
    let entry = cache.entries.get(path)?;
    (entry.reusable && entry.stamp == stamp).then(|| entry.record.clone())
}

pub(super) fn put(path: &Path, metadata: &Metadata, record: &ManifestEntry) {
    let Some(stamp) = stamp(metadata) else { return };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0);
    // Files changed within the timestamp resolution window must be rehashed.
    // ctime cannot be restored by setting mtime. Coarse timestamps, future
    // clocks and unsupported platforms deliberately miss the cache.
    let reusable = stamp.modified.1 != 0
        && stamp.changed.1 != 0
        && stamp.modified.0 < now.saturating_sub(2)
        && stamp.changed.0 < now.saturating_sub(2);
    let Ok(mut cache) = cache().lock() else {
        return;
    };
    if !cache.entries.contains_key(path) {
        while cache.entries.len() >= CAPACITY {
            let Some(oldest) = cache.order.pop_front() else {
                break;
            };
            cache.entries.remove(&oldest);
        }
        cache.order.push_back(path.to_owned());
    }
    cache.entries.insert(
        path.to_owned(),
        Entry {
            stamp,
            record: record.clone(),
            reusable,
        },
    );
}

pub(super) fn same(before: &Metadata, after: &Metadata) -> bool {
    match (stamp(before), stamp(after)) {
        (Some(before), Some(after)) => before == after,
        _ => {
            before.len() == after.len()
                && before.modified().ok() == after.modified().ok()
                && super::capture::mode(before) == super::capture::mode(after)
        }
    }
}
