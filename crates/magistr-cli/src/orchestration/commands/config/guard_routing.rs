//! Workspace-search routing for the Bash hook: `grep`, `rg`, `find` and `sed`
//! aimed at the project are refused with a pointer to magistr, whose
//! `search`, `read` and `edit` cover them with ranking, anchors and a
//! parse-verified write pipeline. The same commands stay available for paths
//! outside the workspace and for filtering piped output, which workspace
//! cannot reach. A hook does this where a `permissions.deny` entry cannot:
//! a deny rule blocks unconditionally, even under bypass permissions.

use std::path::{Component, Path, PathBuf};

/// The commands workspace replaces, matched on the executable's basename.
const ROUTED_COMMANDS: &[&str] = &["grep", "egrep", "fgrep", "rg", "find", "sed"];

/// `find` options that precede its starting points.
const FIND_LEADING_OPTIONS: &[&str] = &["-H", "-L", "-P"];

/// `sed` options whose argument is the script or a script file, so the
/// following word is not a target file.
const SED_SCRIPT_OPTIONS: &[&str] = &["-e", "-f", "--expression", "--file"];

/// The reason a routed command is refused, or `None` when the command line
/// never aims one of the routed commands at the workspace. `project` is the
/// canonical project root; relative paths resolve against it, following any
/// `cd` earlier in the same command line.
pub(crate) fn workspace_search_refusal(command: &str, project: &Path) -> Option<String> {
    let mut cwd = project.to_path_buf();
    for segment in segments(command) {
        let words: Vec<&str> = segment
            .iter()
            .map(String::as_str)
            .skip_while(|w| w.contains('=') && !w.starts_with('-'))
            .collect();
        let Some((head, args)) = words.split_first() else {
            continue;
        };
        if *head == "cd" {
            cwd = change_directory(&cwd, args.first().copied());
            continue;
        }
        let tool = basename(head);
        if !ROUTED_COMMANDS.contains(&tool) {
            continue;
        }
        let targets = target_paths(tool, args);
        if !targets.is_empty() && targets.iter().all(|t| is_inside(project, &cwd, t)) {
            return Some(refusal(tool));
        }
    }
    None
}

/// The command line as segments of words: unquoted `;`, `|`, `&` and
/// newlines end a segment, unquoted whitespace ends a word, and quotes are
/// stripped so a quoted separator (a commit message, a pattern) stays text.
fn segments(command: &str) -> Vec<Vec<String>> {
    let mut segments = vec![Vec::new()];
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars();
    let end_word = |word: &mut String, segments: &mut Vec<Vec<String>>| {
        if !word.is_empty() {
            segments
                .last_mut()
                .expect("one segment")
                .push(std::mem::take(word));
        }
    };
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => quote = Some(c),
            (None, '\\') => word.extend(chars.next()),
            (None, ';' | '|' | '&' | '\n') => {
                end_word(&mut word, &mut segments);
                segments.push(Vec::new());
            }
            (None, c) if c.is_whitespace() => end_word(&mut word, &mut segments),
            (None, c) => word.push(c),
        }
    }
    end_word(&mut word, &mut segments);
    segments
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// `cd` with no argument or `~` leaves the workspace; `-` is unknowable and
/// treated the same way so a later relative search is not refused blindly.
fn change_directory(cwd: &Path, target: Option<&str>) -> PathBuf {
    match target {
        None | Some("-") => PathBuf::from("/"),
        Some(t) if t.starts_with('~') => PathBuf::from("/"),
        Some(t) => normalize(&cwd.join(t)),
    }
}

/// Where the command reads from: an empty list means standard input.
fn target_paths<'a>(tool: &str, args: &[&'a str]) -> Vec<&'a str> {
    match tool {
        "find" => find_paths(args),
        "sed" => sed_paths(args),
        "rg" => grep_paths(args, true),
        _ => grep_paths(args, is_recursive_grep(args)),
    }
}

/// grep and rg: the first operand is the pattern, the rest are paths. With
/// no path, a recursive search reads the working directory and any other
/// reads standard input.
fn grep_paths<'a>(args: &[&'a str], recursive: bool) -> Vec<&'a str> {
    let operands = operands(args);
    if operands.len() > 1 {
        return operands[1..].to_vec();
    }
    if recursive {
        return vec!["."];
    }
    Vec::new()
}

fn is_recursive_grep(args: &[&str]) -> bool {
    args.iter().any(|a| {
        *a == "--recursive"
            || *a == "--dereference-recursive"
            || (a.starts_with('-') && !a.starts_with("--") && a.contains(['r', 'R']))
    })
}

/// find: starting points come after the leading options and before the
/// first expression word; none means the working directory.
fn find_paths<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let paths: Vec<&str> = args
        .iter()
        .skip_while(|a| FIND_LEADING_OPTIONS.contains(a))
        .take_while(|a| !a.starts_with(['-', '(', '!']))
        .copied()
        .collect();
    if paths.is_empty() {
        return vec!["."];
    }
    paths
}

/// sed: the script is the first operand unless `-e`/`-f` supplied it; every
/// other operand is a file. No file means standard input.
fn sed_paths<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut operands = Vec::new();
    let mut skip_next = false;
    let mut scripted = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if SED_SCRIPT_OPTIONS.contains(arg) {
            skip_next = true;
            scripted = true;
            continue;
        }
        if arg.starts_with("--expression=") || arg.starts_with("--file=") {
            scripted = true;
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            continue;
        }
        operands.push(*arg);
    }
    if scripted {
        return operands;
    }
    operands.into_iter().skip(1).collect()
}

/// Words that are not options; `--` ends option parsing.
fn operands<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut only_operands = false;
    for arg in args {
        if only_operands {
            out.push(*arg);
        } else if *arg == "--" {
            only_operands = true;
        } else if !(arg.starts_with('-') && arg.len() > 1) {
            out.push(*arg);
        }
    }
    out
}

/// Does `target`, resolved against `cwd`, lie under the project root?
fn is_inside(project: &Path, cwd: &Path, target: &str) -> bool {
    if target.starts_with('~') {
        return false;
    }
    normalize(&cwd.join(target)).starts_with(project)
}

/// Lexical `.`/`..` resolution, no filesystem access.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn refusal(tool: &str) -> String {
    format!(
        "`{tool}` over the workspace is routed to magistr: use `search` (mode \
         lexical, files, structural or references) to find, `read` to view and `edit` \
         to change files in this project. {tool} stays available for paths outside the \
         workspace and for filtering piped output."
    )
}

#[cfg(test)]
#[path = "guard_routing.tests.rs"]
mod tests;
