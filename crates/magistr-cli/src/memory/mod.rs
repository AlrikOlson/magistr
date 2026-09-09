//! Project memory — persistent knowledge about a project that grows over sessions.
//!
//! This module provides:
//! - **Types** — `ProjectMemory`, `MemoryEntry`, `MemoryCategory`
//! - **Storage** — markdown-based persistence in `.claude/memory.md`
//! - **Analyzer** — extract initial memory from detected project stack

pub mod analyzer;
pub mod memory_category;
pub mod memory_entry;
pub mod project_memory;
pub mod storage;
pub mod storage_error;

#[allow(unused_imports)] // Public API — incrementally wired into CLI and TUI
pub(crate) use analyzer::{analyze, merge};
#[allow(unused_imports)]
pub(crate) use memory_category::MemoryCategory;
#[allow(unused_imports)]
pub(crate) use memory_entry::MemoryEntry;
#[allow(unused_imports)]
pub(crate) use project_memory::ProjectMemory;
#[allow(unused_imports)]
pub(crate) use storage::{load, memory_path, render, save};
#[allow(unused_imports)]
pub(crate) use storage_error::MemoryStorageError;
