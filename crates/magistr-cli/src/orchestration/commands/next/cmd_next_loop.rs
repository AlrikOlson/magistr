use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use crate::workflow;
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
    let mut iter_loop = IterationLoop::open_or_recover(
        &dir,
        &wf.spec_path,
        max_iterations,
        crate::roadmap::open_native_engines(&dir),
    )
    .suggest()?;
    let max_iterations = iter_loop.max_iterations();
    iter_loop.persist().suggest()?;

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
                    fmt::success("No ready roadmap chunk — nothing left for the loop to take.")
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
        // Run one /next cycle
        let prompt = iter_loop.next_prompt(&wf.project_config)?;
        if let Err(error) =
            crate::commands::run::cmd_run_workflow(&prompt, &dir, backend, profile).await
        {
            iter_loop.post_iteration_check();
            iter_loop.advance();
            iter_loop.mark_failed();
            iter_loop.persist().suggest()?;
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
    eprintln!(
        "\n{}",
        fmt::success(&format!(
            "Loop complete — {} iterations",
            iter_loop.iteration()
        ))
    );

    Ok(())
}
