use serde_json::{Map, Value};

/// Parsed ephemeral `key=value` flags and usage errors (never silently ignored).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionFlags {
    pub flags: Map<String, Value>,
    pub errors: Vec<String>,
}

impl SessionFlags {
    #[must_use]
    pub fn parse(pairs: &[impl AsRef<str>]) -> Self {
        let mut parsed = Self::default();
        for pair in pairs {
            let pair = pair.as_ref();
            if let Some((key, raw)) = pair.split_once('=')
                && !key.is_empty()
            {
                parsed.flags.insert(key.into(), coerce(raw));
            } else {
                parsed
                    .errors
                    .push(format!("--config expects key=value, got '{pair}'"));
            }
        }
        parsed
    }
}

fn coerce(raw: &str) -> Value {
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    let unsigned = raw.strip_prefix('-').unwrap_or(raw);
    let numeric = unsigned.split_once('.').map_or_else(
        || !unsigned.is_empty() && unsigned.bytes().all(|c| c.is_ascii_digit()),
        |(whole, fraction)| {
            !whole.is_empty()
                && !fraction.is_empty()
                && whole
                    .bytes()
                    .chain(fraction.bytes())
                    .all(|c| c.is_ascii_digit())
        },
    );
    if numeric
        && let Ok(number) = raw.parse::<f64>()
        && let Some(value) = serde_json::Number::from_f64(number)
    {
        // Preserve integral numbers as integers for the TOML/JSON bridge.
        if let Ok(integer) = raw.parse::<i64>() {
            return Value::from(integer);
        }
        return Value::Number(value);
    }
    if raw.starts_with('[')
        && let Ok(value) = serde_json::from_str(raw)
    {
        return value;
    }
    Value::String(raw.into())
}

#[cfg(test)]
#[path = "session_flags.tests.rs"]
mod tests;
