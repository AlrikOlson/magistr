//! One entry of the ship timeline.

/// One moment on the ship half of the Trace tab: an action the agent
/// recorded, or a quality-gate check with its exit code and duration —
/// parsed leniently off the ship feed's `action_recorded` and
/// `check_recorded` frames.
#[derive(Clone, Debug, PartialEq)]
pub enum ShipMoment {
    /// An `action_recorded` frame: what the agent did under a task.
    Action {
        /// The task the action was recorded under.
        task_id: String,
        /// The action's type (`code`, `test`, `research`, …).
        kind: String,
        /// What was done.
        description: String,
        /// The action's ordinal on the ship ledger (`#198`), when the
        /// frame carries one — how a receipt is named.
        id: Option<u64>,
        /// The files the action touched, as recorded.
        files: Vec<String>,
    },
    /// A `check_recorded` frame: one quality gate and how it went.
    Check {
        /// The gate's name.
        name: String,
        /// Whether it passed.
        passed: bool,
        /// The command's exit code, when one was recorded.
        exit_code: Option<i64>,
        /// The runner-reported duration, when one was recorded.
        duration_ms: Option<u64>,
    },
}

impl ShipMoment {
    /// The moment behind an `action_recorded` frame's payload.
    #[must_use]
    pub fn from_action(payload: &serde_json::Value) -> Self {
        let action = &payload["action"];
        Self::Action {
            task_id: payload["task_id"].as_str().unwrap_or_default().to_owned(),
            kind: action["action_type"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            description: action["description"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            id: action["id"].as_u64(),
            files: action["files_touched"]
                .as_array()
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|file| file.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// The moment behind a `check_recorded` frame's payload. The
    /// duration sits on the runner's results when the check carried a
    /// report, or flat on the check when the recorder put it there.
    #[must_use]
    pub fn from_check(payload: &serde_json::Value) -> Self {
        let check = &payload["check"];
        Self::Check {
            name: check["name"].as_str().unwrap_or("check").to_owned(),
            passed: check["passed"].as_bool().unwrap_or(false),
            exit_code: check["exit_code"].as_i64(),
            duration_ms: check["results"]["duration_ms"]
                .as_u64()
                .or_else(|| check["duration_ms"].as_u64()),
        }
    }
}

#[cfg(test)]
#[path = "ship_moment.tests.rs"]
mod tests;
