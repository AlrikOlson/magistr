//! The scope a permission answer is tallied under and a delegation is
//! written over.

use std::path::{Component, Path};

use magistr_engine::policy::PolicyCondition;

use super::resolve_against::glob_spelling;

/// The scope of one ask, as the charter keys it: a tool over a path
/// prefix (`Edit · crates/magistr-tui`), a tool over a shell program
/// (`Bash · cargo`), or the bare tool — with the [`PolicyCondition`] a
/// rule over that scope carries.
///
/// The grain is the design's example: "allowed 14 of 14 edit asks
/// under crates/magistr-tui". A path scopes to its first two
/// directories under the project root, which is a crate in a
/// workspace and a module in a flat tree; a command scopes to its
/// program, `cd … &&` skipped the way the risk classifier skips it.
/// The condition names the same tool *and* the same place — a scope
/// is never the path alone, so an always-allow on edits under a crate
/// says nothing about deleting it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationScope {
    /// The stable key the tally and the rule ids are made from.
    pub key: String,
    /// The scope in the reader's words.
    pub subject: String,
    /// The condition a rule over this scope carries.
    pub condition: PolicyCondition,
}

impl DelegationScope {
    /// The fields a call names its path in, in the order they are read.
    const PATH_FIELDS: [&'static str; 4] = ["file_path", "path", "notebook_path", "target_file"];

    /// The scope of `tool` called with `input` in a session rooted at
    /// `root`. `None` for a call with no tool name; an absolute path
    /// outside the root, or a command with no program, falls back to
    /// the tool alone.
    #[must_use]
    pub fn of(tool: &str, input: &serde_json::Value, root: Option<&Path>) -> Option<Self> {
        let tool = tool.trim();
        if tool.is_empty() {
            return None;
        }
        if let Some(program) = input
            .get("command")
            .and_then(|v| v.as_str())
            .and_then(program_of)
        {
            return Some(Self::command(tool, program));
        }
        if let Some(prefix) = Self::PATH_FIELDS
            .iter()
            .find_map(|field| input.get(field).and_then(|v| v.as_str()))
            .and_then(|path| prefix_of(Path::new(path), root))
        {
            return Some(Self::under(tool, &prefix, root));
        }
        Some(Self {
            key: format!("tool:{}", tool.to_lowercase()),
            subject: tool.to_owned(),
            condition: PolicyCondition::tool(tool),
        })
    }

    /// `tool` over the path prefix `prefix` — spelled both ways the
    /// agent spells a path: relative to the root, and absolute under
    /// it. The engine's glob is anchored, so the absolute spelling is
    /// the root's own when the session knows it and an open `*/`
    /// prefix when it does not.
    fn under(tool: &str, prefix: &str, root: Option<&Path>) -> Self {
        let absolute = root.map_or_else(
            || format!("*/{prefix}"),
            |root| glob_spelling(&root.join(prefix)),
        );
        Self {
            key: format!("{}:{prefix}", tool.to_lowercase()),
            subject: format!("{tool} \u{b7} {prefix}"),
            condition: PolicyCondition::all_of(vec![
                PolicyCondition::tool(tool),
                PolicyCondition::any_of(vec![
                    PolicyCondition::path(format!("{prefix}/*")),
                    PolicyCondition::path(format!("{absolute}/*")),
                    PolicyCondition::path(prefix),
                    PolicyCondition::path(absolute),
                ]),
            ]),
        }
    }

    /// `tool` running `program` — bare, with arguments, and behind the
    /// `cd … &&` agents prefix.
    fn command(tool: &str, program: &str) -> Self {
        Self {
            key: format!("{}:{program}", tool.to_lowercase()),
            subject: format!("{tool} \u{b7} {program}"),
            condition: PolicyCondition::all_of(vec![
                PolicyCondition::tool(tool),
                PolicyCondition::any_of(vec![
                    PolicyCondition::command(program),
                    PolicyCondition::command(format!("{program} *")),
                    PolicyCondition::command(format!("cd * && {program}")),
                    PolicyCondition::command(format!("cd * && {program} *")),
                ]),
            ]),
        }
    }
}

/// The program a shell command runs: the first word of its last `&&`
/// segment, so `cd x && cargo test` is `cargo`. `None` for an empty
/// command or a program spelled with a glob character — a scope must
/// never be wider than the command it was cut from.
fn program_of(command: &str) -> Option<&str> {
    let segment = command.rsplit("&&").next()?.trim();
    let program = segment.split_whitespace().next()?;
    (!program.contains(['*', '?'])).then_some(program)
}

/// The scope prefix of `path` under `root`: its first two directories
/// relative to the root (one when that is all there is), or the file
/// itself at the root. `None` for an absolute path outside the root.
fn prefix_of(path: &Path, root: Option<&Path>) -> Option<String> {
    let relative = match root {
        Some(root) if path.is_absolute() => path.strip_prefix(root).ok()?,
        _ => path,
    };
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let (file, dirs) = parts.split_last()?;
    let prefix: Vec<&str> = if dirs.is_empty() {
        vec![*file]
    } else {
        dirs.iter().take(2).copied().collect()
    };
    if prefix.iter().any(|part| part.contains(['*', '?'])) {
        return None;
    }
    Some(prefix.join("/"))
}

#[cfg(test)]
#[path = "delegation_scope.tests.rs"]
mod tests;
