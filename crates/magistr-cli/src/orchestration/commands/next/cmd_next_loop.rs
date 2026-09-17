use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow;
use crate::workflow::iteration::{OperationKind, OperationOutcome};
use color_eyre::eyre;

pub async fn cmd_next_loop(
    dir: &std::path::Path,
    max_iterations: u32,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
) -> eyre::Result<()> {
    use workflow::iteration::{IterationLoop, PostCheckResult, PreCheckResult};

    let dir = super::canonicalize_dir(dir)?;

    let wf = workflow::WorkflowConfig::load(&dir).suggest()?;

    // Clean up leftover state
    wf.cleanup()?;

    // Initialize iteration loop (native roadmap engines when the project
    // selects think-and-ship; the spec file otherwise)
    let mut iter_loop = IterationLoop::start_or_recover(
        &dir,
        &wf.spec_path,
        max_iterations,
        crate::roadmap::admit_native_host(&dir)?,
    )
    .suggest()?;
    let max_iterations = iter_loop.max_iterations();
    iter_loop.persist().suggest()?;
    // The host's owner endpoint, for a cockpit opened beside this loop;
    // it lives for the loop and drops before the writer.
    let _owner_endpoint = crate::roadmap::serve_owner_endpoint(iter_loop.native()).await?;
    // The workspace this host owns for the loop: every iteration's session
    // writes through it, or is refused when it could not be admitted.
    let workspace = crate::commands::run::admit_workspace(&dir, |line| eprintln!("{line}")).await;
    let source = workspace.source_route();

    eprintln!(
        "{}\n",
        fmt::info(&format!(
            "Starting next-loop (max {max_iterations} iterations)"
        ))
    );

    loop {
        // Pre-iteration checks (spec complete? max reached?)
        match iter_loop.pre_iteration_check().suggest()? {
            PreCheckResult::Continue => {}
            PreCheckResult::Stopped(reason) => {
                eprintln!("{}", fmt::warning(&format!("Loop stopped: {reason}")));
                break;
            }
            PreCheckResult::SpecComplete => {
                eprintln!("{}", fmt::success("All spec items complete!"));
                break;
            }
            PreCheckResult::NoReadyChunk => {
                eprintln!(
                    "{}",
                    fmt::success(
                        "No work is ready to start or continue. Check the roadmap for blocked items."
                    )
                );
                break;
            }
            PreCheckResult::MaxIterations => {
                eprintln!(
                    "{}",
                    fmt::warning(&format!(
                        "Max iterations ({max_iterations}) reached. Stopping."
                    ))
                );
                break;
            }
            PreCheckResult::Unreconciled(open) => {
                let open: Vec<String> = open
                    .iter()
                    .map(|record| format!("{} {}", record.kind, record.target))
                    .collect();
                if let Err(err) = workspace.shutdown().await {
                    eprintln!("magistr: workspace shutdown: {err}");
                }
                return Err(eyre::eyre!(
                    "recovery could not establish what happened to: {} — reconcile before retrying, \
                     or preserve/remove .magistr/loop-state.json for an explicit new run",
                    open.join("; ")
                ));
            }
        }

        let summary = iter_loop.spec_summary().suggest()?;
        eprintln!(
            "{}",
            fmt::cyan(&format!(
                "━━━ Iteration {} ━━━",
                iter_loop.display_iteration()
            ))
        );
        eprintln!("{} {}", fmt::cyan("→"), summary.status_line());

        iter_loop.persist().suggest()?;
        // Run one /next cycle. The dispatch is written before the run and
        // carries the runtime's handle as soon as the session is started,
        // so recovery asks that particular process rather than Git.
        let prompt = iter_loop.next_prompt(&wf.project_config)?;
        let dispatch = iter_loop
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .suggest()?;
        let Some(dispatch) = dispatch else {
            eprintln!(
                "{}",
                fmt::warning(
                    "provider dispatch already established for this attempt — settled without launching again"
                )
            );
            iter_loop.post_iteration_check();
            iter_loop.advance();
            iter_loop.persist().suggest()?;
            continue;
        };
        let ran = crate::commands::run::cmd_run_workflow(
            &prompt,
            &dir,
            backend,
            profile,
            source.clone(),
            |pid| {
                iter_loop
                    .start_operation(
                        &dispatch,
                        pid.and_then(workflow::runtime_handle::RuntimeHandle::observe)
                            .map(|handle| handle.to_string()),
                    )
                    .suggest()?;
                iter_loop.persist().suggest()
            },
        )
        .await;
        iter_loop
            .settle_operation(
                &dispatch,
                if ran.is_ok() {
                    OperationOutcome::Completed
                } else {
                    OperationOutcome::Failed
                },
            )
            .suggest()?;
        if let Err(error) = ran {
            iter_loop.post_iteration_check();
            iter_loop.advance();
            iter_loop.mark_failed();
            iter_loop.persist().suggest()?;
            if let Err(err) = workspace.shutdown().await {
                eprintln!("magistr: workspace shutdown: {err}");
            }
            return Err(error);
        }

        // Post-iteration check: did a commit happen?
        let post = iter_loop.post_iteration_check();
        iter_loop.advance();
        iter_loop.persist().suggest()?;
        match post {
            PostCheckResult::CommitMade { .. } => {}
            PostCheckResult::NoCommit { guidance } => {
                eprintln!("{}", fmt::warning(&guidance));
                if iter_loop.should_stop_no_commits() {
                    eprintln!(
                        "{}",
                        fmt::warning("Too many iterations without commits. Stopping.")
                    );
                    break;
                }
            }
        }

        // Clean up plan state for next iteration
        wf.cleanup()?;
    }

    // Clean up
    iter_loop.persist().suggest()?;
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    eprintln!(
        "\n{}",
        fmt::success(&format!(
            "Loop complete — {} iterations",
            iter_loop.iteration()
        ))
    );

    Ok(())
}
