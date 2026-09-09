//! Tool classification — workflow-relevant categorization of tool calls.
//!
//! Provides `ToolCategory` classification, declarative pattern matching via
//! `ToolPatterns`, and MCP tool name normalization. These are used by the
//! workflow tracker to infer phase transitions from tool usage evidence.

pub mod classify;
pub mod gate_patterns;
pub mod normalize;
pub mod patterns;
