//! Select the actual persistence owner before applying a mutation.

use std::path::Path;

pub enum TransactionTarget<'a, T> {
    Memory(&'a T),
    File { path: &'a Path, lock_path: &'a Path },
}
