//! Bash command patterns that indicate high risk.

/// Bash command patterns that indicate high risk.
pub const HIGH_RISK_BASH_PATTERNS: &[&str] = &[
    "rm ",
    "rm\t",
    "rmdir",
    "git push",
    "git reset --hard",
    "git checkout -- .",
    "git clean",
    "git branch -D",
    "sudo ",
    "chmod ",
    "chown ",
    "curl ",
    "wget ",
    "ssh ",
    "> /dev/",
    "dd if=",
    "mkfs",
    "fdisk",
    "kill ",
    "killall ",
    "pkill ",
    "shutdown",
    "reboot",
];
