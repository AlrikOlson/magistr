use super::{ParsedManifest, PluginLimits, PluginToolDecl, RESERVED_TOOL_NAMES};

/// Default export address for the plugin's invoke function.
const INVOKE_EXPORT_DEFAULT: &str = "invoke";
/// The tables a manifest may carry; anything else warns and is ignored.
const KNOWN_TABLES: [&str; 5] = ["plugin", "component", "tools", "capabilities", "limits"];

/// One validated `plugin.toml`: identity, component, tools, the WIT world
/// import names it declares, fs-read scopes and requested limits. Validation
/// is fail-closed on identity, tools and the component path and warn-tolerant
/// on unknown tables; wire names are derived, never declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginManifest {
    pub publisher: String,
    pub pkg: String,
    pub version: String,
    /// Component wasm, relative to the plugin's own directory.
    pub component_path: String,
    /// A root export name or `pkg:iface/name@ver#func` inside an exported interface.
    pub invoke_export: String,
    pub tools: Vec<PluginToolDecl>,
    pub imports: Vec<String>,
    pub fs_read: Vec<String>,
    pub limits: PluginLimits,
}

impl PluginManifest {
    /// Parse and validate one manifest; every error is reported at once.
    pub fn parse(source: &str) -> Result<ParsedManifest, Vec<String>> {
        let root: toml::Table = match toml::from_str(source) {
            Ok(table) => table,
            Err(error) => return Err(vec![format!("plugin.toml is not valid TOML: {error}")]),
        };
        let mut errors = Vec::new();
        let warnings: Vec<String> = root
            .keys()
            .filter(|key| !KNOWN_TABLES.contains(&key.as_str()))
            .map(|key| format!("unknown plugin.toml table [{key}] ignored"))
            .collect();
        let plugin = table(&root, "plugin");
        let publisher = name_field(&plugin, "plugin.publisher", &mut errors);
        let pkg = name_field(&plugin, "plugin.pkg", &mut errors);
        let version = match plugin.get("version").and_then(toml::Value::as_str) {
            Some(version) if !version.is_empty() => version.to_owned(),
            _ => {
                errors.push("plugin.version must be a non-empty string".into());
                String::new()
            }
        };
        let component = table(&root, "component");
        let component_path = component_path(&component, &mut errors);
        let invoke_export = match component.get("invoke") {
            None => INVOKE_EXPORT_DEFAULT.to_owned(),
            Some(toml::Value::String(export)) if !export.is_empty() => export.clone(),
            Some(_) => {
                errors.push("component.invoke must be a non-empty string when present".into());
                String::new()
            }
        };
        let tools = tools(&root, &mut errors);
        let capabilities = table(&root, "capabilities");
        let imports = string_list(
            &capabilities,
            "imports",
            "capabilities.imports",
            &mut errors,
        );
        let fs_read = string_list(
            &capabilities,
            "fs-read",
            "capabilities.fs-read",
            &mut errors,
        );
        let limits_table = table(&root, "limits");
        let limits = PluginLimits {
            fuel: positive(&limits_table, "fuel", "limits.fuel", &mut errors),
            memory_bytes: positive(
                &limits_table,
                "memory_bytes",
                "limits.memory_bytes",
                &mut errors,
            )
            .map(|value| usize::try_from(value).unwrap_or(usize::MAX)),
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(ParsedManifest {
            manifest: Self {
                publisher,
                pkg,
                version,
                component_path,
                invoke_export,
                tools,
                imports,
                fs_read,
                limits,
            },
            warnings,
        })
    }

    /// `publisher.pkg`: the inventory name and the breaker's plugin label.
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}.{}", self.publisher, self.pkg)
    }

    /// The one place wire names come from. Every wire name contains `/`, so it
    /// can never collide with a core tool name.
    #[must_use]
    pub fn wire_tool_name(&self, tool: &str) -> String {
        format!("{}/{tool}", self.label())
    }
}

/// publisher, pkg and tool names: lowercase kebab, 1–32 chars.
fn is_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && value.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn table(root: &toml::Table, key: &str) -> toml::Table {
    root.get(key)
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default()
}

fn name_field(table: &toml::Table, field: &str, errors: &mut Vec<String>) -> String {
    let key = field.rsplit('.').next().unwrap_or(field);
    match table.get(key).and_then(toml::Value::as_str) {
        Some(value) if is_name(value) => value.to_owned(),
        _ => {
            errors.push(format!("{field} must match ^[a-z][a-z0-9-]{{0,31}}$"));
            String::new()
        }
    }
}

fn component_path(component: &toml::Table, errors: &mut Vec<String>) -> String {
    match component.get("path").and_then(toml::Value::as_str) {
        Some(path)
            if !path.is_empty()
                && !path.contains("..")
                && !path.starts_with('/')
                && !is_drive_path(path) =>
        {
            path.to_owned()
        }
        _ => {
            errors
                .push("component.path must be a relative path inside the plugin directory".into());
            String::new()
        }
    }
}

fn is_drive_path(path: &str) -> bool {
    let mut chars = path.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.next() == Some(':')
}

fn tools(root: &toml::Table, errors: &mut Vec<String>) -> Vec<PluginToolDecl> {
    let mut tools = Vec::new();
    let Some(entries) = root.get("tools").and_then(toml::Value::as_array) else {
        errors.push("at least one [[tools]] entry is required".into());
        return tools;
    };
    if entries.is_empty() {
        errors.push("at least one [[tools]] entry is required".into());
        return tools;
    }
    for entry in entries {
        let record = entry.as_table().cloned().unwrap_or_default();
        let Some(name) = record.get("name").and_then(toml::Value::as_str) else {
            errors.push("tools[].name must match ^[a-z][a-z0-9-]{0,31}$".into());
            continue;
        };
        if !is_name(name) {
            errors.push("tools[].name must match ^[a-z][a-z0-9-]{0,31}$".into());
            continue;
        }
        if RESERVED_TOOL_NAMES.contains(&name) {
            errors.push(format!(
                "tools[].name {name:?} is a reserved core tool name (SDD §1.1: the seven core tools plus undo are immutable)"
            ));
            continue;
        }
        if tools.iter().any(|tool: &PluginToolDecl| tool.name == name) {
            errors.push(format!("duplicate tool name {name:?}"));
            continue;
        }
        tools.push(PluginToolDecl {
            name: name.to_owned(),
            description: record
                .get("description")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        });
    }
    tools
}

fn string_list(
    table: &toml::Table,
    key: &str,
    field: &str,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let Some(value) = table.get(key) else {
        return Vec::new();
    };
    let strings: Option<Vec<String>> = value.as_array().and_then(|items| {
        items
            .iter()
            .map(|item| item.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
            .collect()
    });
    strings.unwrap_or_else(|| {
        errors.push(format!("{field} must be an array of non-empty strings"));
        Vec::new()
    })
}

fn positive(table: &toml::Table, key: &str, field: &str, errors: &mut Vec<String>) -> Option<u64> {
    match table.get(key) {
        None => None,
        Some(toml::Value::Integer(value)) if *value >= 1 => Some(*value as u64),
        Some(_) => {
            errors.push(format!("{field} must be a positive integer"));
            None
        }
    }
}

#[cfg(test)]
#[path = "manifest.tests.rs"]
mod tests;
