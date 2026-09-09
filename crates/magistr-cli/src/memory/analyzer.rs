//! Project analysis for building initial memory.
//!
//! The analyzer examines a project's detected stack and existing configuration
//! to generate an initial set of memory entries. It uses magistr's detection engine
//! rather than running an AI agent — this is fast, deterministic, and free.

use std::path::Path;

use super::memory_category::MemoryCategory;
use super::memory_entry::MemoryEntry;
use super::project_memory::ProjectMemory;
use crate::types::{
    Formatter, Framework, Language, Linter, PackageManager, Stack, TestFramework, Workspace,
};

/// Analyze a project and build initial memory from its detected stack.
///
/// Does not overwrite existing entries — returns a fresh `ProjectMemory`
/// that the caller can merge with any existing memory.
pub(crate) fn analyze(stack: &Stack, project_dir: &Path) -> ProjectMemory {
    let mut memory = ProjectMemory::new(&stack.project_name);

    // Architecture entries
    add_architecture_entries(&mut memory, stack, project_dir);

    // Convention entries from detected tools
    add_convention_entries(&mut memory, stack);

    // Pattern entries from language/framework
    add_pattern_entries(&mut memory, stack);

    // Common mistake entries
    add_mistake_entries(&mut memory, stack);

    memory
}

/// Merge new entries into an existing memory without duplicating.
///
/// An entry is considered a duplicate if it has the same category and text.
pub(crate) fn merge(existing: &mut ProjectMemory, new_entries: &ProjectMemory) {
    for entry in &new_entries.entries {
        let already_exists = existing
            .entries
            .iter()
            .any(|e| e.category == entry.category && e.text == entry.text);

        if !already_exists {
            existing.entries.push(entry.clone());
        }
    }
}

fn add_architecture_entries(memory: &mut ProjectMemory, stack: &Stack, project_dir: &Path) {
    // Language
    if stack.language != Language::Unknown {
        memory.add(MemoryEntry::new(
            MemoryCategory::Architecture,
            format!("Primary language: {}", stack.language),
        ));
    }

    // Framework
    if stack.framework != Framework::None {
        memory.add(MemoryEntry::new(
            MemoryCategory::Architecture,
            format!("Framework: {}", stack.framework),
        ));
    }

    // Workspace/monorepo
    if stack.workspace != Workspace::None {
        memory.add(MemoryEntry::new(
            MemoryCategory::Architecture,
            format!("Monorepo using {} workspaces", stack.workspace),
        ));
    }

    // Check for common directory structures
    let has_src = project_dir.join("src").is_dir();
    let has_lib = project_dir.join("lib").is_dir();
    let has_tests = project_dir.join("tests").is_dir();

    if has_src && has_tests {
        memory.add(MemoryEntry::new(
            MemoryCategory::Architecture,
            "Standard src/ + tests/ directory layout",
        ));
    } else if has_lib && has_tests {
        memory.add(MemoryEntry::new(
            MemoryCategory::Architecture,
            "Standard lib/ + tests/ directory layout",
        ));
    }
}

fn add_convention_entries(memory: &mut ProjectMemory, stack: &Stack) {
    // Package manager
    if stack.package_manager != PackageManager::Unknown {
        memory.add(MemoryEntry::new(
            MemoryCategory::Conventions,
            format!("Package manager: {}", stack.package_manager),
        ));
    }

    // Test framework
    if stack.test_framework != TestFramework::Unknown {
        memory.add(MemoryEntry::new(
            MemoryCategory::Conventions,
            format!("Test framework: {}", stack.test_framework),
        ));
    }

    // Linter
    if stack.linter != Linter::None {
        memory.add(MemoryEntry::new(
            MemoryCategory::Conventions,
            format!("Linter: {}", stack.linter),
        ));
    }

    // Formatter
    if stack.formatter != Formatter::None {
        memory.add(MemoryEntry::new(
            MemoryCategory::Conventions,
            format!("Formatter: {}", stack.formatter),
        ));
    }
}

fn add_pattern_entries(memory: &mut ProjectMemory, stack: &Stack) {
    match stack.language {
        Language::Rust => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Use `?` operator for error propagation",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Prefer iterators and combinators over manual loops",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Builder pattern for structs with many optional fields",
            ));
        }
        Language::TypeScript | Language::JavaScript => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Use async/await over raw promises",
            ));
            if stack.framework == Framework::Next {
                memory.add(MemoryEntry::new(
                    MemoryCategory::Patterns,
                    "Server Components by default, 'use client' only when needed",
                ));
            }
        }
        Language::Python => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Use type hints for function signatures",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Context managers for resource cleanup",
            ));
        }
        Language::Go => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Return errors as values, check err != nil",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Patterns,
                "Use interfaces for dependency injection",
            ));
        }
        _ => {}
    }
}

fn add_mistake_entries(memory: &mut ProjectMemory, stack: &Stack) {
    match stack.language {
        Language::Rust => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "No `unwrap()` in library code — use `?` or `expect()` with context",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "No `clone()` to silence the borrow checker — fix ownership model",
            ));
        }
        Language::TypeScript | Language::JavaScript => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "No `any` type — use proper type definitions or `unknown`",
            ));
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "No `console.log` left in production code",
            ));
        }
        Language::Python => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "No bare `except:` clauses — catch specific exceptions",
            ));
        }
        Language::Go => {
            memory.add(MemoryEntry::new(
                MemoryCategory::Mistakes,
                "Never ignore returned errors — handle or explicitly discard with `_`",
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "analyzer.tests.rs"]
mod tests;
