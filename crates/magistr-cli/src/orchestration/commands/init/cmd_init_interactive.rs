use crate::agent;
use crate::commands;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_init_interactive(dir: &std::path::Path, force: bool) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    commands::misc::ensure_no_existing_scaffold(&dir, force)?;

    eprintln!(
        "{}",
        fmt::info(&format!("Smart-detecting stack in {}...", dir.display()))
    );
    let smart = detect::detect_stack_smart(&dir).suggest()?;
    let stack = smart.to_stack();
    eprintln!(
        "{}",
        fmt::info(&format!(
            "Detected: {} + {} ({})",
            stack.language, stack.framework, stack.package_manager
        ))
    );

    // Show wizard intelligence before launching agent
    commands::misc::print_wizard_summary(&smart);

    eprintln!(
        "{}",
        fmt::info("Launching AI agent for interactive setup...")
    );
    eprintln!();

    let dir_str = dir.display().to_string();
    let init_prompt = agent::system_prompt::build_init_prompt(&stack, &dir_str);

    let config =
        agent::AgentConfig::new("Set up this project with AI-assisted configuration.", &dir)
            .system_prompt(init_prompt)
            .permission_mode(crate::agent::PermissionMode::AcceptEdits);

    let mut session = agent::session::AgentSession::start(config)
        .await
        .suggest()?;

    loop {
        let mut turn_done = false;
        while let Some(msg_result) = session.next_message().await {
            match msg_result {
                Ok(msg) => match &msg {
                    crate::agent::message::AgentMessage::Assistant { message, .. } => {
                        for block in &message.content {
                            if let crate::agent::message::ContentBlock::Text { text } = block {
                                print!("{text}");
                            }
                        }
                    }
                    crate::agent::message::AgentMessage::Result { is_error, .. } => {
                        eprintln!();
                        if *is_error {
                            eprintln!("{}", fmt::error("Agent ended with error"));
                        } else {
                            eprintln!("{}", fmt::success("Project scaffolded successfully"));
                        }
                        turn_done = true;
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    eprintln!("{} {e}", fmt::red("error:"));
                    return Ok(());
                }
            }
        }

        if !turn_done {
            break;
        }

        eprint!("{} Follow up (empty to quit): ", fmt::cyan("→"));
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }
        session.send_followup(input.trim())?;
    }

    Ok(())
}
