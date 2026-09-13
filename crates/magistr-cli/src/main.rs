use clap::Parser;
use color_eyre::eyre;

use magistr::cli::{Cli, Command};
use magistr::{commands, config, output, utils};
use magistr_engine::cost::BudgetConfig as EngineBudgetConfig;

// Test-only imports: these modules are used in #[cfg(test)] tests below.
#[cfg(test)]
use magistr::{agent, cli, detect, scaffold, types};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Private owned worker role: bypass interactive hooks, config repair, MCP,
    // and workspace owner admission. Strict argv keeps it out of normal CLI use.
    if std::env::args_os()
        .skip(1)
        .eq([std::ffi::OsStr::new("--workspace-worker")])
    {
        return magistr_workspace::worker::serve_worker()
            .await
            .map_err(Into::into);
    }
    let cli = parse_cli();
    // MCP launch owns stdio and process shutdown. Do not install the cockpit's
    // terminal restoration handlers or propagate interactive-session settings.
    if let Some(Command::McpLaunch { target, ref dir }) = cli.command {
        color_eyre::install()?;
        return commands::misc::cmd_mcp_launch(target, dir).await;
    }
    if let Some(Command::CodexLaunch {
        ref project_root,
        diagnose,
        ref args,
    }) = cli.command
    {
        color_eyre::install()?;
        return magistr::codex_launch::launch(project_root, args, diagnose);
    }
    utils::panic::install_panic_hook()?;
    utils::signal_handler::install_signal_handler();
    #[cfg(unix)]
    utils::sigterm_handler::install_sigterm_handler();

    let json_output = cli.json;
    let use_tui = should_use_tui(cli.no_tui || cli.json);
    let renderer = output::create_renderer(json_output, false);

    let budget_decision = propagate_env_flags(&cli);
    init_tracing(&cli);
    // Every run records which scope bound its budget and what each scope
    // had left. Enforcement nobody can reconstruct afterwards is
    // indistinguishable from no enforcement.
    budget_decision.log();

    // Snapshot all MAGISTR_* env vars into a struct — threaded through the app
    // instead of scattered std::env::var() reads.
    let env = magistr::env::MagistrEnv::from_process_env();

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            if use_tui {
                return commands::home::cmd_home(&env).await;
            }
            // No TUI: print help and exit
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            return Ok(());
        }
    };

    let result = match command {
        Command::Detect { dir, smart, tree } => {
            if tree {
                commands::misc::cmd_detect_tree(&dir, &*renderer)
            } else if smart {
                commands::misc::cmd_detect_smart(&dir, &*renderer)
            } else {
                commands::misc::cmd_detect(&dir, &*renderer)
            }
        }
        Command::Init {
            dir,
            agents,
            interactive,
            yes,
            force,
            smart,
            dry_run,
            from,
            ci,
            check,
        } => {
            if check {
                commands::init::cmd_init_check(&dir)
            } else if ci {
                commands::config::cmd_init_ci(&dir, force, &*renderer)
            } else if let Some(preset_source) = from {
                commands::config::cmd_init_from_preset(&dir, &preset_source, force, &agents)
            } else if dry_run {
                commands::init::cmd_init_dry_run(&dir, &*renderer, &agents)
            } else if interactive {
                if use_tui {
                    commands::init::cmd_init_interactive_tui(&dir, force, &env).await
                } else {
                    commands::init::cmd_init_interactive(&dir, force).await
                }
            } else if smart {
                commands::init::cmd_init_smart(&dir, force, &agents).await
            } else if yes {
                commands::init::cmd_init_interview(&dir, force, true, &agents).await
            } else if !agents.is_empty() {
                commands::init::cmd_init(&dir, force, &agents).await
            } else if use_tui && std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                commands::init::cmd_init_interview(&dir, force, false, &agents).await
            } else {
                commands::init::cmd_init(&dir, force, &agents).await
            }
        }
        Command::Reset { dir } => commands::misc::cmd_reset(&dir),
        Command::Validate {
            dir,
            ci,
            annotations,
        } => {
            if ci || json_output || annotations {
                commands::config::cmd_validate_ci(&dir, annotations).await
            } else {
                commands::validate::cmd_validate(&dir).await
            }
        }
        Command::Run {
            prompt,
            resume,
            r#continue,
            model,
            backend,
            headless,
            format,
        } => {
            if r#continue || resume.is_some() {
                unsafe { std::env::set_var("MAGISTR_CONTINUE", "1") };
            }
            if let Some(ref m) = model {
                unsafe { std::env::set_var("MAGISTR_MODEL", m) };
            }
            let agent_backend = commands::resolve_backend(backend)?;
            let profile = commands::run::resolve_permission_profile(
                &std::env::current_dir()?,
                cli.trust.as_deref(),
            )?;
            if headless {
                commands::run::cmd_run_headless(
                    &prompt,
                    resume.as_deref(),
                    r#continue,
                    model.as_deref(),
                    agent_backend,
                    format,
                    profile,
                )
                .await
            } else if use_tui {
                commands::run::cmd_run_tui(
                    &prompt,
                    resume.as_deref(),
                    r#continue,
                    model.as_deref(),
                    agent_backend,
                    profile,
                    &env,
                )
                .await
            } else {
                commands::run::cmd_run(
                    &prompt,
                    resume.as_deref(),
                    r#continue,
                    model.as_deref(),
                    agent_backend,
                    profile,
                )
                .await
            }
        }
        Command::Next {
            dir,
            r#continue: continue_plan,
            backend,
            headless,
            format,
            gate_only,
        } => {
            if gate_only {
                commands::next::cmd_next_gate_only(&dir, format).await
            } else {
                let agent_backend = commands::resolve_backend(backend)?;
                let profile =
                    commands::run::resolve_permission_profile(&dir, cli.trust.as_deref())?;
                if headless {
                    commands::next::cmd_next_headless(&dir, agent_backend, format, profile).await
                } else if continue_plan {
                    commands::next::cmd_next_continue(&dir, agent_backend, profile).await
                } else if use_tui {
                    commands::next::cmd_next_tui(&dir, agent_backend, profile, &env).await
                } else {
                    commands::next::cmd_next(&dir, agent_backend, profile).await
                }
            }
        }
        Command::NextLoop {
            dir,
            max_iterations,
            backend,
            headless,
            format,
            max_cost,
        } => {
            let agent_backend = commands::resolve_backend(backend)?;
            let profile = commands::run::resolve_permission_profile(&dir, cli.trust.as_deref())?;
            if headless {
                commands::next::cmd_next_loop_headless(
                    &dir,
                    max_iterations,
                    agent_backend,
                    format,
                    max_cost,
                    profile,
                )
                .await
            } else if use_tui {
                commands::next::cmd_next_loop_tui(
                    &dir,
                    max_iterations,
                    agent_backend,
                    profile,
                    &env,
                )
                .await
            } else {
                commands::next::cmd_next_loop(&dir, max_iterations, agent_backend, profile).await
            }
        }
        Command::Migrate { dir } => commands::config::cmd_migrate(&dir),
        Command::Completions { shell, install } => {
            if install {
                commands::misc::cmd_completions_install(shell)
            } else {
                commands::misc::cmd_completions(shell)
            }
        }
        Command::ManPages { dir } => commands::misc::cmd_man_pages(&dir),
        Command::Config { action } => {
            commands::config_manage::cmd_config(action, use_tui, &env).await
        }
        Command::Archetypes { dir, detail, json } => {
            commands::archetypes::cmd_archetypes(&dir, detail, json)
        }
        Command::Lint { dir, suggest } => commands::config::cmd_lint(&dir, suggest, &*renderer),
        Command::Doctor { dir } => commands::config::cmd_doctor(&dir, &*renderer),
        Command::Guard { event, dir } => commands::config::cmd_guard(event, &dir),
        Command::Export { dir, output: out } => {
            commands::config::cmd_export(&dir, out.as_deref(), &*renderer)
        }
        Command::Import { source, dir, force } => {
            commands::config::cmd_import(&dir, &source, force)
        }
        Command::Costs { query } => commands::session::cmd_costs(&query),
        Command::Plans { dir, status, limit } => {
            commands::misc::cmd_plans(&dir, status, limit, &*renderer)
        }
        Command::Sessions {
            project,
            since_days,
            limit,
            format,
            tag,
            purge,
            owner,
            handoff,
        } => commands::session::cmd_sessions(
            project.as_deref(),
            since_days,
            limit,
            format,
            tag,
            purge,
            owner.as_deref(),
            handoff,
        ),
        Command::Login {
            provider,
            issuer,
            client_id,
            scope,
            status,
            dir,
        } => commands::misc::cmd_login(
            &dir,
            provider.as_deref(),
            issuer.as_deref(),
            client_id.as_deref(),
            scope.as_deref(),
            status,
            &*renderer,
        ),
        Command::Logout { provider } => commands::misc::cmd_logout(provider.as_deref(), &*renderer),
        Command::Team { action, dir } => commands::misc::cmd_team(&dir, action, &*renderer),
        Command::Policy { action, dir } => commands::misc::cmd_policy(&dir, action, &*renderer),
        Command::Permissions {
            limit,
            project,
            actor,
            export,
        } => commands::session::cmd_permissions(
            limit,
            project.as_deref(),
            actor.as_deref(),
            export,
            &*renderer,
        ),
        Command::Models => commands::models::cmd_models(&*renderer).await,
        Command::Learn { dir } => commands::misc::cmd_learn(&dir, &*renderer),
        Command::Memory { dir, action } => commands::misc::cmd_memory(&dir, action, &*renderer),
        Command::Plugins { action } => commands::misc::cmd_plugins(action, &*renderer),
        Command::Preset { action, dir } => {
            use magistr::cli::PresetAction;
            match action {
                Some(PresetAction::List) | None => {
                    commands::preset::cmd_preset_list(&dir, &*renderer)
                }
                Some(PresetAction::Create { output }) => {
                    commands::preset::cmd_preset_create(&dir, output.as_deref())
                }
                Some(PresetAction::Test { file }) => commands::preset::cmd_preset_test(&file),
            }
        }
        Command::McpProxy { cmd } => commands::misc::cmd_mcp_proxy(cmd),
        Command::McpServe { dir } => commands::misc::cmd_mcp_serve(&dir).await,
        Command::Workspace { action } => commands::misc::cmd_workspace(action).await,
        Command::McpLaunch { target, dir } => commands::misc::cmd_mcp_launch(target, &dir).await,
        Command::CodexLaunch { .. } => unreachable!("Codex launch dispatches before session setup"),
        Command::CodexRemove { dir } => {
            let paths = magistr::scaffold::CodexSetup::remove_target(&dir)?;
            eprintln!(
                "Removed managed Codex setup from {} artifacts.",
                paths.len()
            );
            Ok(())
        }
        Command::AcpAgent {
            provider,
            allow_tool,
        } => commands::misc::cmd_acp_agent(&provider, &allow_tool).await,
        Command::Roadmap { action } => commands::misc::cmd_roadmap(action, &*renderer, &env).await,
    };

    // An interrupted run has settled on its own terms by now — its cost,
    // its receipts, its checkpoint, its workspace — and exits as the
    // interrupt it was, whatever the command answered.
    if utils::shutdown::Shutdown::is_requested() {
        if let Err(report) = &result {
            eprintln!("magistr: {report}");
        }
        std::process::exit(utils::shutdown::Shutdown::EXIT_CODE);
    }

    // Attach actionable suggestions from MagistrError to the eyre report.
    result.map_err(|report| {
        use color_eyre::Section;
        match report.downcast_ref::<magistr::error::MagistrError>() {
            Some(err) => match err.suggestion() {
                Some(suggestion) => report.suggestion(suggestion.to_string()),
                None => report,
            },
            None => report,
        }
    })
}

/// Parse the command line. clap's own usage errors exit 2 everywhere in
/// `magistr`; under `magistr workspace` the retained EX_USAGE (64) is part
/// of the verbs' process contract, so a malformed workspace invocation
/// exits 64 instead. Help and version still exit 0.
fn parse_cli() -> Cli {
    Cli::try_parse().unwrap_or_else(|error| {
        let workspace_verb = std::env::args().nth(1).as_deref() == Some("workspace");
        if workspace_verb && error.use_stderr() {
            let _ = error.print();
            std::process::exit(64);
        }
        error.exit()
    })
}

/// Whether interactive surfaces (the cockpit, the init interview) may take
/// this terminal: an explicit opt-out wins, otherwise stdout must be a tty.
fn should_use_tui(no_tui: bool) -> bool {
    if no_tui {
        return false;
    }
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Propagate CLI flags to environment variables for downstream consumption.
///
/// Called once at startup before any threads are spawned.
///
/// Returns the budget decision it made, for the caller to log *after*
/// tracing is initialized — this runs before `init_tracing`, so logging
/// from in here would write to a subscriber that does not exist yet.
fn propagate_env_flags(cli: &Cli) -> magistr_engine::cost::BudgetDecision {
    // SAFETY: This runs at startup in main() before any threads are spawned,
    // so there are no concurrent readers of these environment variables.

    if let Some(ref theme_name) = cli.theme {
        unsafe { std::env::set_var("MAGISTR_THEME", theme_name) };
    }

    if cli.no_mouse {
        unsafe { std::env::set_var("MAGISTR_NO_MOUSE", "1") };
    }
    if cli.reduce_motion {
        unsafe { std::env::set_var("REDUCE_MOTION", "1") };
        unsafe { std::env::set_var("NO_ANIMATIONS", "1") };
    }
    if let Some(ref cvd) = cli.cvd {
        unsafe { std::env::set_var("MAGISTR_CVD", cvd) };
    }
    if cli.screen_reader {
        unsafe { std::env::set_var(magistr_cockpit::capability::Mode::SCREEN_READER_ENV, "1") };
    }

    let decision;

    // Merge budget defaults: CLI flags take precedence over user config
    // file — then hand the result to the team's policy, which can only
    // tighten it. Both inputs here are the developer's own (a flag and
    // their user config); the roster is not, which is what makes the cap
    // enforceable rather than advisory.
    {
        let user_cfg = config::user_config::load().unwrap_or_default();
        let local = EngineBudgetConfig {
            limit: cli.budget.or(user_cfg.budget.limit),
            warn_threshold: cli.budget_warn.or(user_cfg.budget.warn_threshold),
            max_iterations: None,
            max_duration_secs: None,
        };
        let resolved = match (
            std::env::current_dir(),
            magistr::cost::costs_path::costs_path(),
        ) {
            (Ok(dir), Some(ledger)) => {
                magistr::cost::governed_budget(&dir, &ledger, &local, chrono::Utc::now())
            }
            _ => magistr_engine::cost::BudgetPolicy::default()
                .resolve(&local, &magistr_engine::cost::ScopeSpend::new()),
        };
        if let Some(l) = resolved.effective.limit {
            unsafe { std::env::set_var("MAGISTR_BUDGET", l.to_string()) };
        }
        // The developer's own limit, beside the governed one: the rail's
        // meter runs to this width and puts its breaker at the governed
        // cap when a scope bound tighter.
        if let Some(requested) = local.limit {
            unsafe { std::env::set_var("MAGISTR_BUDGET_REQUESTED", requested.to_string()) };
        }
        if let Some(w) = resolved.effective.warn_threshold {
            unsafe { std::env::set_var("MAGISTR_BUDGET_WARN", w.to_string()) };
        }
        if let Some(scope) = resolved.binding {
            unsafe { std::env::set_var("MAGISTR_BUDGET_BINDING", scope.as_str()) };
        }
        decision = magistr_engine::cost::BudgetDecision::new(local.limit, &resolved);
    }

    if let Some(ref level) = cli.verbose {
        unsafe { std::env::set_var("MAGISTR_LOG", level) };
    }

    if cli.splash {
        eprintln!("{}", version_splash());
    }

    decision
}

/// ASCII logo + version banner behind `--splash`.
fn version_splash() -> String {
    const LOGO: &str = r"
     _ _
    (_|_) __ _
    | | |/ _` |
    | | | (_| |
   _/ |_|\__, |
  |__/   |___/
";
    format!(
        "{}\n  v{}\n  AI-powered development toolkit\n",
        LOGO.trim(),
        env!("CARGO_PKG_VERSION")
    )
}

/// Initialize tracing subscriber when `--verbose` or `MAGISTR_LOG` is set.
///
/// Output goes to stderr so it doesn't interfere with TUI rendering or
/// MCP stdio transport — or, when `MAGISTR_LOG_FILE` names a path, is
/// appended to that file: the cockpit owns fd 2 for the life of its
/// raw-mode session and replays it only on leave, so a run measured
/// from outside (scripts/goal-loop-measure.py) needs the trace live in
/// a file of its own. Only activates when explicitly requested.
fn init_tracing(cli: &Cli) {
    // Only init if --verbose flag or MAGISTR_LOG env var is set
    let has_flag = cli.verbose.is_some();
    let has_env = std::env::var("MAGISTR_LOG")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();

    if !has_flag && !has_env {
        return;
    }

    use tracing_subscriber::filter::{EnvFilter, LevelFilter};

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .with_env_var("MAGISTR_LOG")
        .from_env_lossy();

    let log_file = std::env::var("MAGISTR_LOG_FILE")
        .ok()
        .filter(|v| !v.is_empty())
        .and_then(|path| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });

    // Best-effort init — if another subscriber is already installed, skip silently.
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let _ = match log_file {
        Some(file) => builder
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .try_init(),
        None => builder.with_writer(std::io::stderr).try_init(),
    };
}

#[cfg(test)]
#[path = "main.tests.rs"]
mod tests;
