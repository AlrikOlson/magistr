//! Doctor diagnostics — check project health for Claude integration.
//!
//! Examines the project directory for common issues that cause Claude
//! to ignore or misinterpret your configuration.

use std::path::Path;

pub use super::check_status::CheckStatus;
pub use super::doctor_check::DoctorCheck;

/// Run all doctor checks against a project directory.
pub fn run_all(dir: &Path) -> Vec<DoctorCheck> {
    vec![
        check_claude_md_exists(dir),
        check_claude_md_size(dir),
        check_rules_directory(dir),
        check_settings_valid(dir),
        check_project_json(dir),
        check_claude_dir_exists(dir),
    ]
}

/// Check: Does CLAUDE.md exist at the project root?
fn check_claude_md_exists(dir: &Path) -> DoctorCheck {
    let path = dir.join("CLAUDE.md");
    if path.exists() {
        DoctorCheck {
            name: "claude-md",
            description: "CLAUDE.md exists".into(),
            status: CheckStatus::Pass,
            detail: None,
        }
    } else {
        DoctorCheck {
            name: "claude-md",
            description: "CLAUDE.md not found".into(),
            status: CheckStatus::Fail,
            detail: Some(
                "Create a CLAUDE.md at the project root. Run `magistr init` to generate one."
                    .into(),
            ),
        }
    }
}

/// Check: Is CLAUDE.md a reasonable size?
fn check_claude_md_size(dir: &Path) -> DoctorCheck {
    let path = dir.join("CLAUDE.md");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return DoctorCheck {
            name: "claude-md-size",
            description: "CLAUDE.md size check skipped (file not found)".into(),
            status: CheckStatus::Warn,
            detail: None,
        };
    };

    let line_count = content.lines().count();
    let byte_size = content.len();

    if line_count > 500 {
        DoctorCheck {
            name: "claude-md-size",
            description: format!("CLAUDE.md is {line_count} lines ({} KB) — too large", byte_size / 1024),
            status: CheckStatus::Fail,
            detail: Some(
                "Claude may truncate or deprioritize very long CLAUDE.md files. Move detailed rules to .claude/rules/."
                    .into(),
            ),
        }
    } else if line_count > 300 {
        DoctorCheck {
            name: "claude-md-size",
            description: format!("CLAUDE.md is {line_count} lines — approaching limit"),
            status: CheckStatus::Warn,
            detail: Some("Consider splitting detailed sections into .claude/rules/ files.".into()),
        }
    } else {
        DoctorCheck {
            name: "claude-md-size",
            description: format!("CLAUDE.md is {line_count} lines — good size"),
            status: CheckStatus::Pass,
            detail: None,
        }
    }
}

/// Check: Does .claude/rules/ exist with content?
fn check_rules_directory(dir: &Path) -> DoctorCheck {
    let rules_dir = dir.join(".claude").join("rules");
    if !rules_dir.exists() {
        return DoctorCheck {
            name: "rules-dir",
            description: ".claude/rules/ directory not found".into(),
            status: CheckStatus::Warn,
            detail: Some(
                "Create .claude/rules/ for detailed conventions. Claude auto-loads all .md files from this directory."
                    .into(),
            ),
        };
    }

    let rule_files: Vec<_> = std::fs::read_dir(&rules_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .collect();

    if rule_files.is_empty() {
        DoctorCheck {
            name: "rules-dir",
            description: ".claude/rules/ exists but has no .md files".into(),
            status: CheckStatus::Warn,
            detail: Some("Add .md files with coding conventions and project rules.".into()),
        }
    } else {
        DoctorCheck {
            name: "rules-dir",
            description: format!(".claude/rules/ has {} rule file(s)", rule_files.len()),
            status: CheckStatus::Pass,
            detail: None,
        }
    }
}

/// Check: Is .claude/settings.json valid JSON?
fn check_settings_valid(dir: &Path) -> DoctorCheck {
    let path = dir.join(".claude").join("settings.json");
    if !path.exists() {
        return DoctorCheck {
            name: "settings",
            description: ".claude/settings.json not found".into(),
            status: CheckStatus::Warn,
            detail: Some("Run `magistr init` to generate settings.".into()),
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
                DoctorCheck {
                    name: "settings",
                    description: ".claude/settings.json is valid JSON".into(),
                    status: CheckStatus::Pass,
                    detail: None,
                }
            } else {
                DoctorCheck {
                    name: "settings",
                    description: ".claude/settings.json contains invalid JSON".into(),
                    status: CheckStatus::Fail,
                    detail: Some(
                        "Fix the JSON syntax. Claude will ignore malformed settings.".into(),
                    ),
                }
            }
        }
        Err(e) => DoctorCheck {
            name: "settings",
            description: format!("Cannot read .claude/settings.json: {e}"),
            status: CheckStatus::Fail,
            detail: None,
        },
    }
}

/// Check: Does .claude/project.json exist?
fn check_project_json(dir: &Path) -> DoctorCheck {
    let path = crate::paths::project_json(dir);
    if !path.exists() {
        return DoctorCheck {
            name: "project-json",
            description: ".magistr/project.json not found".into(),
            status: CheckStatus::Warn,
            detail: Some("Run `magistr init` to generate project configuration.".into()),
        };
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
                DoctorCheck {
                    name: "project-json",
                    description: ".magistr/project.json is valid".into(),
                    status: CheckStatus::Pass,
                    detail: None,
                }
            } else {
                DoctorCheck {
                    name: "project-json",
                    description: ".magistr/project.json contains invalid JSON".into(),
                    status: CheckStatus::Fail,
                    detail: Some("Fix the JSON syntax in project.json.".into()),
                }
            }
        }
        Err(e) => DoctorCheck {
            name: "project-json",
            description: format!("Cannot read .magistr/project.json: {e}"),
            status: CheckStatus::Fail,
            detail: None,
        },
    }
}

/// Check: Does .claude/ directory exist at all?
fn check_claude_dir_exists(dir: &Path) -> DoctorCheck {
    let claude_dir = dir.join(".claude");
    if claude_dir.exists() && claude_dir.is_dir() {
        DoctorCheck {
            name: "claude-dir",
            description: ".claude/ directory exists".into(),
            status: CheckStatus::Pass,
            detail: None,
        }
    } else {
        DoctorCheck {
            name: "claude-dir",
            description: ".claude/ directory not found".into(),
            status: CheckStatus::Fail,
            detail: Some("Run `magistr init` to scaffold the .claude/ directory.".into()),
        }
    }
}

#[cfg(test)]
#[path = "doctor.tests.rs"]
mod tests;
