use serde_json::{Value, json};

pub(super) enum KeySpec {
    Boolean {
        default: bool,
        tighter: Option<bool>,
    },
    Number(u32),
    Enum {
        default: &'static str,
        levels: &'static [&'static str],
        capped: bool,
    },
    AdditiveList,
}

impl KeySpec {
    pub(super) const REGISTRY: [(&'static str, Self); 10] = [
        (
            "security.redaction",
            Self::Boolean {
                default: true,
                tighter: Some(true),
            },
        ),
        (
            "security.allow_sideload",
            Self::Boolean {
                default: true,
                tighter: Some(false),
            },
        ),
        ("security.protected_paths", Self::AdditiveList),
        (
            "telemetry.consent",
            Self::Enum {
                default: "off",
                levels: &["off", "crash", "full"],
                capped: true,
            },
        ),
        (
            "edit.force_confirm",
            Self::Boolean {
                default: false,
                tighter: Some(true),
            },
        ),
        (
            "edit.confirm_mode",
            Self::Enum {
                default: "token",
                levels: &["elicit", "token"],
                capped: false,
            },
        ),
        ("budgets.default", Self::Number(4000)),
        (
            "format.enabled",
            Self::Boolean {
                default: true,
                tighter: None,
            },
        ),
        (
            "tracing.enabled",
            Self::Boolean {
                default: true,
                tighter: None,
            },
        ),
        (
            "watcher.mode",
            Self::Enum {
                default: "auto",
                levels: &["auto", "native", "poll", "off"],
                capped: false,
            },
        ),
    ];

    pub(super) fn default_value(&self) -> Value {
        match self {
            Self::Boolean { default, .. } => json!(default),
            Self::Number(default) => json!(default),
            Self::Enum { default, .. } => json!(default),
            Self::AdditiveList => json!([]),
        }
    }

    pub(super) fn is_security(&self) -> bool {
        matches!(
            self,
            Self::Boolean {
                tighter: Some(_),
                ..
            } | Self::Enum { capped: true, .. }
                | Self::AdditiveList
        )
    }

    pub(super) fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::Boolean { .. } => value.is_boolean(),
            Self::Number(_) => value.is_number(),
            Self::Enum { levels, .. } => value.as_str().is_some_and(|v| levels.contains(&v)),
            Self::AdditiveList => value
                .as_array()
                .is_some_and(|v| v.iter().all(Value::is_string)),
        }
    }

    pub(super) fn expected(&self) -> String {
        match self {
            Self::Boolean { .. } => "a boolean".into(),
            Self::Number(_) => "a number".into(),
            Self::Enum { levels, .. } => format!("one of {}", levels.join(" | ")),
            Self::AdditiveList => "an array of strings".into(),
        }
    }

    pub(super) fn loosens(&self, current: &Value, candidate: &Value) -> bool {
        match self {
            Self::Boolean {
                tighter: Some(tighter),
                ..
            } => current.as_bool() == Some(*tighter) && candidate.as_bool() != Some(*tighter),
            Self::Enum {
                levels,
                capped: true,
                ..
            } => {
                let index = |v: &Value| levels.iter().position(|level| Some(*level) == v.as_str());
                index(candidate) > index(current)
            }
            _ => false,
        }
    }
}
