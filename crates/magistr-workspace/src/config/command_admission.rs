use crate::contracts::{ErrorKind, WorkspaceError};

/// Declared-command admission before any executor is consulted. This is not a
/// process sandbox; later execution must use the returned argv without a shell.
pub struct CommandAdmission;

impl CommandAdmission {
    pub fn admit(
        config: &super::ResolvedConfig,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, WorkspaceError> {
        let line = config.commands.get(command).ok_or_else(|| {
            let declared = config.commands.keys().map(String::as_str).collect::<Vec<_>>().join(", ");
            WorkspaceError::new(ErrorKind::CommandNotDeclared, format!(
                "command {command:?} is not in the merged [commands] table. Declared: {}. A human can declare it in workbench.toml [commands] (or the user/org config layer, SDD §7.2) — the server cannot amend its own allowlist.",
                if declared.is_empty() { "(none)" } else { &declared }))
        })?;
        let mut argv: Vec<String> = line
            .split(is_ecmascript_whitespace)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect();
        // Native safety delta: arguments cannot turn an empty allowlist entry
        // into an agent-selected program.
        if argv.is_empty() {
            return Err(WorkspaceError::new(
                ErrorKind::RunRedirected,
                format!(
                    "command {command:?} has an empty declared argv; declare a program before supplying arguments"
                ),
            ));
        }
        argv.extend(args.iter().cloned());
        guard(&argv, command)?;
        Ok(argv)
    }
}

const OPERATORS: &[&str] = &[
    "|", "||", "&&", ";", "&", ">", ">>", "<", "<<", "2>", "2>>", "2>&1", "1>&2", "$(", "`",
];
const SHELLS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "fish",
    "ksh",
    "csh",
    "tcsh",
    "cmd",
    "command",
    "powershell",
    "pwsh",
];
const MUNGING: &[&str] = &[
    "sed", "awk", "gawk", "mawk", "grep", "egrep", "fgrep", "rg", "ag", "ack", "cat", "head",
    "tail", "cut", "tr", "sort", "uniq", "perl", "patch", "ed", "ex", "sd", "find", "fd",
];
const WRAPPERS: &[&str] = &["env", "xargs", "npx", "bunx", "exec", "nice", "time"];

fn guard(argv: &[String], command: &str) -> Result<(), WorkspaceError> {
    let redirect = "use `search` (or `read`) to inspect files and `edit` to modify them — run executes declared project commands only (SDD §9.6, §1.1)";
    let mut program_position = true;
    let mut env_options = false;
    for token in argv {
        let message = if OPERATORS.contains(&token.as_str()) {
            Some(format!(
                "run executes a single argv with no shell: {token:?} in command \"{command}\" would be passed as a literal argument, never interpreted; {redirect}"
            ))
        } else if program_position {
            // env -S reparses one argv element as a command line. Refuse that
            // second interpretation rather than inspect a different argv from
            // the one the eventual process would execute (native safety delta).
            if env_options && splits_env_string(token) {
                return Err(WorkspaceError::new(
                    ErrorKind::RunRedirected,
                    format!(
                        "command \"{command}\" uses env split-string option {token:?}, which reparses arguments; {redirect}"
                    ),
                ));
            }
            if token.starts_with('-') || is_assignment(token) {
                continue;
            }
            let program = token
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(token)
                .to_lowercase();
            let program = program.strip_suffix(".exe").unwrap_or(&program);
            program_position = WRAPPERS.contains(&program);
            env_options = program == "env";
            if SHELLS.contains(&program) {
                Some(format!(
                    "command \"{command}\" resolves to a shell ({token:?}), which is general shell execution; {redirect}"
                ))
            } else if MUNGING.contains(&program) {
                Some(format!(
                    "command \"{command}\" is text/file munging via {token:?}; {redirect}"
                ))
            } else {
                None
            }
        } else {
            None
        };
        if let Some(message) = message {
            return Err(WorkspaceError::new(ErrorKind::RunRedirected, message));
        }
    }
    Ok(())
}

fn splits_env_string(token: &str) -> bool {
    if token == "--split-string" || token.starts_with("--split-string=") {
        return true;
    }
    let Some(short) = token
        .strip_prefix('-')
        .filter(|short| !short.starts_with('-'))
    else {
        return false;
    };
    for option in short.chars() {
        if option == 'S' {
            return true;
        }
        // These short options consume the remainder as data, not option letters.
        if matches!(option, 'u' | 'C' | 'a') {
            break;
        }
    }
    false
}

fn is_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.bytes();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

fn is_ecmascript_whitespace(character: char) -> bool {
    // Preserve the reference /\s+/ token boundary: Rust's Unicode whitespace
    // includes NEL (U+0085) but excludes the BOM (U+FEFF); JavaScript reverses it.
    matches!(character, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}'
        | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}'
        | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

#[cfg(test)]
#[path = "command_admission.tests.rs"]
mod tests;
