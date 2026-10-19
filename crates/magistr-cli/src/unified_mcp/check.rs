//! An actual stdio handshake and read-only context request for setup and diagnostics.
use std::{path::Path, process::Stdio, time::Duration};

use color_eyre::eyre::{self, Context};
use rmcp::{ServiceExt, model::CallToolRequestParams};
use tokio::process::Command;

/// Verify the installed executable and its worker startup without running project work.
pub async fn check_connection(root: &Path) -> eyre::Result<()> {
    let executable = std::env::current_exe()
        .wrap_err("Cannot locate Magistr; reinstall it and run magistr doctor")?;
    let mut command = Command::new(executable);
    command
        .args(["mcp", "--access", "read-only", "--dir"])
        .arg(root)
        .current_dir(root);
    check_command(command).await?;
    for command in customized_commands(root)? {
        check_command(command).await?;
    }
    Ok(())
}

async fn check_command(mut command: Command) -> eyre::Result<()> {
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .kill_on_drop(true).spawn().wrap_err("Cannot start the configured Magistr MCP command; check its command and arguments, then run magistr doctor")?;
    let input = child.stdout.take().expect("piped stdout");
    let output = child.stdin.take().expect("piped stdin");
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let client = ().serve((input, output)).await?;
        let context = client.call_tool(CallToolRequestParams::new("context")).await?;
        let mut request = CallToolRequestParams::new("execute");
        request.arguments = Some(serde_json::json!({"source":"return magistr.call('workspace_status', {});"})
            .as_object().expect("argument object").clone());
        let workspace = client.call_tool(request).await;
        client.cancel().await?;
        for result in [context, workspace?] {
            if result.is_error == Some(true) || super::client::value(&result)["ok"] == false {
                eyre::bail!("The MCP check failed: {}. Run magistr doctor to inspect the project configuration.",
                    super::client::value(&result));
            }
        }
        eyre::Ok(())
    }).await;
    match result {
        Ok(Ok(())) => {
            let status = tokio::time::timeout(Duration::from_secs(10), child.wait()).await??;
            eyre::ensure!(
                status.success(),
                "Magistr MCP exited with {status}; run magistr doctor"
            );
            Ok(())
        }
        Ok(Err(error)) => {
            let _ = child.kill().await;
            Err(error).wrap_err("MCP connection check failed; run magistr doctor")
        }
        Err(_) => {
            let _ = child.kill().await;
            eyre::bail!("MCP connection check exceeded 30 seconds; run magistr doctor");
        }
    }
}

fn customized_commands(root: &Path) -> eyre::Result<Vec<Command>> {
    let mut entries = Vec::new();
    let json = root.join(".mcp.json");
    if json.exists() {
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&json)?).wrap_err("Cannot read .mcp.json")?;
        if let Some(entry) = config
            .get("mcpServers")
            .and_then(|servers| servers.get("magistr"))
        {
            entries.push((".mcp.json", entry.clone()));
        }
    }
    let toml = root.join(".codex/config.toml");
    if toml.exists() {
        let config: toml::Value = toml::from_str(&std::fs::read_to_string(&toml)?)
            .wrap_err("Cannot read .codex/config.toml")?;
        if let Some(entry) = config
            .get("mcp_servers")
            .and_then(|servers| servers.get("magistr"))
        {
            entries.push((".codex/config.toml", serde_json::to_value(entry)?));
        }
    }
    let mut commands = Vec::new();
    for (file, entry) in entries {
        eyre::ensure!(
            entry["enabled"] != false,
            "Magistr is disabled in {file}; enable it to check the connection"
        );
        eyre::ensure!(
            entry.get("url").is_none(),
            "The custom Magistr URL in {file} is preserved. Check it in your agent; this installation check supports stdio connections"
        );
        let program = entry["command"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("Missing Magistr command in {file}"))?;
        let arguments: Vec<String> = serde_json::from_value(
            entry
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )
        .wrap_err_with(|| format!("Invalid Magistr arguments in {file}"))?;
        let generated = serde_json::json!({"type":"stdio","command":program,"args":arguments});
        // Generated registrations use the installed executable already checked above.
        // Customized wrappers must themselves answer, rather than being silently skipped.
        if crate::mcp_target::McpTarget.owns_entry(root, &generated)
            && entry.get("env").is_none()
            && entry.get("cwd").is_none()
        {
            continue;
        }
        let path = Path::new(program);
        let mut command = Command::new(if path.is_relative() && path.components().count() > 1 {
            root.join(path)
        } else {
            path.to_path_buf()
        });
        command.args(arguments).current_dir(root);
        if let Some(cwd) = entry["cwd"].as_str() {
            command.current_dir(root.join(cwd));
        }
        if let Some(env) = entry.get("env") {
            let env: std::collections::HashMap<String, String> =
                serde_json::from_value(env.clone())
                    .wrap_err_with(|| format!("Invalid Magistr environment in {file}"))?;
            command.envs(env);
        }
        commands.push(command);
    }
    Ok(commands)
}
