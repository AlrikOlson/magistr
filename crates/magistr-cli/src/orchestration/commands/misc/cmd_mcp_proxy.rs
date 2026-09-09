use color_eyre::eyre;

/// MCP stdio proxy: spawn a child process, pass stdin through, and filter
/// its stdout so only valid JSON-RPC lines (starting with `{`) reach the
/// caller. Non-JSON lines (e.g. log timestamps) are redirected to stderr.
pub fn cmd_mcp_proxy(cmd: Vec<String>) -> eyre::Result<()> {
    use std::io::{BufRead, BufReader, Write};

    if cmd.is_empty() {
        return Err(eyre::eyre!("mcp-proxy requires a command to run"));
    }

    let mut child = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| eyre::eyre!("failed to spawn {}: {}", cmd[0], e))?;

    let child_stdout = child.stdout.take().expect("stdout was piped");
    let reader = BufReader::new(child_stdout);
    let mut stdout = std::io::stdout().lock();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('{') {
            writeln!(stdout, "{line}")?;
            stdout.flush()?;
        } else {
            // Redirect non-JSON lines to stderr so they're visible in logs
            // but don't corrupt the JSON-RPC transport.
            eprintln!("[mcp-proxy] {line}");
        }
    }

    let status = child.wait()?;
    std::process::exit(status.code().unwrap_or(1));
}
