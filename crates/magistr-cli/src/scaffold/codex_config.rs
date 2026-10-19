use super::{codex_ownership::CodexOwnership, managed_content::ManagedContent};
use crate::error::{MagistrError, Result};
use std::collections::HashMap;
use toml_edit::{Array, DocumentMut, Item, Table, Value, value};

/// Comment-preserving Codex transport serializer and component ownership boundary.
pub(super) struct CodexConfig;

impl CodexConfig {
    pub(super) fn parse(content: &str) -> Result<DocumentMut> {
        let doc = content.parse::<DocumentMut>().map_err(|e| {
            MagistrError::Config(format!(
                ".codex/config.toml: {e}; fix the TOML before setup; no overwrite"
            ))
        })?;
        if let Some(servers) = doc.get("mcp_servers") {
            let servers = servers
                .as_table_like()
                .ok_or_else(|| invalid("mcp_servers must be a table"))?;
            for server in ["magistr"] {
                if let Some(entry) = servers.get(server) {
                    let entry = entry
                        .as_table_like()
                        .ok_or_else(|| invalid("MCP server must be a table"))?;
                    for key in ["command", "url", "cwd"] {
                        if entry.get(key).is_some_and(|v| !v.is_str()) {
                            return Err(invalid("MCP command, url and cwd must be strings"));
                        }
                    }
                    for key in ["args", "disabled_tools"] {
                        if let Some(item) = entry.get(key) {
                            let a = item.as_array().ok_or_else(|| {
                                invalid("MCP args and disabled_tools must be string arrays")
                            })?;
                            if a.iter().any(|v| !v.is_str()) {
                                return Err(invalid(
                                    "MCP args and disabled_tools must be string arrays",
                                ));
                            }
                        }
                    }
                    for key in ["enabled", "required"] {
                        if entry.get(key).is_some_and(|v| !v.is_bool()) {
                            return Err(invalid("MCP enabled and required must be booleans"));
                        }
                    }
                }
            }
        }
        Ok(doc)
    }

    pub(super) fn merge(
        existing: &str,
        owned: &CodexOwnership,
        vars: &HashMap<String, String>,
        native_only: bool,
        repair: bool,
    ) -> Result<String> {
        let mut doc = Self::parse(existing)?;
        let shared = super::mcp_config::build(vars)?;
        if let Some(servers) = doc.get_mut("mcp_servers").and_then(Item::as_table_like_mut) {
            servers.remove("workbench-mcp");
            servers.remove("think-and-ship");
        }
        for server in ["magistr"] {
            let is_new = doc.get("mcp_servers").and_then(|v| v.get(server)).is_none();
            let custom_transport = has_custom_transport(&doc, owned, server);
            let metadata_only = get(&doc, server, "command").is_none()
                && get(&doc, server, "url").is_none()
                && get(&doc, server, "args").is_none()
                && !owned.config.contains_key(&format!("{server}.command"))
                && !owned.config.contains_key(&format!("{server}.args"));
            if !is_new
                && get(&doc, server, "command").is_none()
                && get(&doc, server, "url").is_none()
                && !metadata_only
            {
                return Err(invalid(&format!(
                    "{server} has no command or URL; restore its transport before setup"
                )));
            }
            let mut desired = Table::new();
            {
                let spec = &shared["mcpServers"][server];
                desired["command"] = value(
                    spec["command"]
                        .as_str()
                        .ok_or_else(|| invalid("missing launch command"))?,
                );
                let args: Array = spec["args"]
                    .as_array()
                    .ok_or_else(|| invalid("missing launch args"))?
                    .iter()
                    .map(|s| Value::from(s.as_str().unwrap_or_default()))
                    .collect();
                desired["args"] = value(args);
                desired["startup_timeout_sec"] = value(60);
                desired["tool_timeout_sec"] = value(1800);
                desired["required"] = value(true);
            }
            for (key, desired) in desired.iter() {
                if custom_transport && ["command", "args"].contains(&key) {
                    continue;
                }
                let current = get(&doc, server, key);
                let id = format!("{server}.{key}");
                let owned_current = owned
                    .config
                    .get(&id)
                    .is_some_and(|o| current.map(Item::to_string).as_deref() == Some(&o.generated));
                let missing_launch = metadata_only && ["command", "args"].contains(&key);
                if ((is_new || missing_launch) && !repair) || owned_current {
                    // An explicit transport remains intact, including an HTTP endpoint.
                    put(&mut doc, server, key, desired.clone());
                }
            }
        }
        if native_only && !repair {
            let current = get(&doc, "magistr", "disabled_tools");
            let mut denied = current
                .and_then(Item::as_array)
                .cloned()
                .unwrap_or_default();
            if !denied.iter().any(|v| v.as_str() == Some("roadmap_export")) {
                denied.push("roadmap_export");
                put(&mut doc, "magistr", "disabled_tools", value(denied));
            }
        }
        Ok(doc.to_string())
    }

    pub(super) fn record(before: &str, after: &str, owned: &mut CodexOwnership) -> Result<()> {
        let old = Self::parse(before)?;
        let new = Self::parse(after)?;
        for server in ["magistr"] {
            for key in FIELDS {
                let before = get(&old, server, key).map(Item::to_string);
                let after = get(&new, server, key).map(Item::to_string);
                if before == after {
                    continue;
                }
                let id = format!("{server}.{key}");
                if let Some(generated) = after {
                    let entry = ManagedContent::updated(owned.config.get(&id), before, generated);
                    owned.config.insert(id, entry);
                }
            }
        }
        owned.selected = true;
        Ok(())
    }

    pub(super) fn remove(existing: &str, owned: &CodexOwnership) -> Result<String> {
        let mut doc = Self::parse(existing)?;
        let custom: Vec<_> = ["magistr"]
            .into_iter()
            .filter(|server| has_custom_transport(&doc, owned, server))
            .collect();
        for (id, entry) in &owned.config {
            let Some((server, key)) = id.split_once('.') else {
                continue;
            };
            if !["magistr"].contains(&server) || !FIELDS.contains(&key) {
                continue;
            }
            if custom.contains(&server) && ["command", "args"].contains(&key) {
                continue;
            }
            if get(&doc, server, key).map(Item::to_string).as_deref() != Some(&entry.generated) {
                continue;
            }
            if let Some(original) = &entry.original {
                let mut parsed = format!("restored = {original}\n")
                    .parse::<DocumentMut>()
                    .map_err(|e| invalid(&e.to_string()))?;
                put(
                    &mut doc,
                    server,
                    key,
                    std::mem::take(&mut parsed["restored"]),
                );
            } else if let Some(table) = doc
                .get_mut("mcp_servers")
                .and_then(|v| v.get_mut(server))
                .and_then(Item::as_table_like_mut)
            {
                table.remove(key);
            }
        }
        if let Some(servers) = doc.get_mut("mcp_servers").and_then(Item::as_table_like_mut) {
            for server in ["magistr"] {
                if servers
                    .get(server)
                    .and_then(Item::as_table_like)
                    .is_some_and(|t| t.is_empty())
                {
                    servers.remove(server);
                }
            }
        }
        Ok(doc.to_string())
    }
}

const FIELDS: &[&str] = &[
    "command",
    "args",
    "enabled",
    "startup_timeout_sec",
    "tool_timeout_sec",
    "required",
    "disabled_tools",
];

fn has_custom_transport(doc: &DocumentMut, owned: &CodexOwnership, server: &str) -> bool {
    ["command", "args"].into_iter().any(|key| {
        let current = get(doc, server, key).map(Item::to_string);
        owned
            .config
            .get(&format!("{server}.{key}"))
            .is_some_and(|o| current.as_deref() != Some(&o.generated))
    })
}

fn invalid(message: &str) -> MagistrError {
    MagistrError::Config(format!(".codex/config.toml: {message}; left untouched"))
}

fn get<'a>(doc: &'a DocumentMut, server: &str, key: &str) -> Option<&'a Item> {
    doc.get("mcp_servers")?.get(server)?.get(key)
}

fn put(doc: &mut DocumentMut, server: &str, key: &str, mut item: Item) {
    if let Some(old) = get(doc, server, key).and_then(Item::as_value)
        && let Some(new) = item.as_value_mut()
    {
        *new.decor_mut() = old.decor().clone();
    }
    if doc.get("mcp_servers").is_none() {
        let mut servers = Table::new();
        servers.set_implicit(true);
        doc["mcp_servers"] = Item::Table(servers);
    }
    if doc["mcp_servers"].get(server).is_none() {
        doc["mcp_servers"][server] = Item::Table(Table::new());
    }
    doc["mcp_servers"][server][key] = item;
}
