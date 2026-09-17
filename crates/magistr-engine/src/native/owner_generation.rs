//! Lock-file generation allocation stays with native owner admission.
pub use magistr_workflow_contracts::OwnerGeneration;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

const WIDTH: usize = 20;

/// Read the counter at the head of `lock`, advance it, and write it
/// back in place. Called once per admission, under the lock the file
/// carries; a file that holds nothing (or something that is not a
/// counter) starts at one.
pub(crate) fn mint(lock: &mut File) -> io::Result<OwnerGeneration> {
    lock.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(WIDTH);
    (&*lock).take(WIDTH as u64).read_to_end(&mut bytes)?;
    let previous = String::from_utf8_lossy(&bytes)
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let next = previous
        .checked_add(1)
        .ok_or_else(|| io::Error::other("magistr: the owner generation counter is exhausted"))?;
    lock.seek(SeekFrom::Start(0))?;
    write!(lock, "{next:0WIDTH$}")?;
    lock.flush()?;
    OwnerGeneration::admitted(next)
}

#[cfg(test)]
#[path = "owner_generation.tests.rs"]
mod tests;
