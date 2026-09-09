use super::*;
use crate::detect;
use crate::error::{self, ResultExt};
use crate::lint;
use crate::output::OutputRenderer;
use color_eyre::eyre;

pub fn cmd_lint(
    dir: &std::path::Path,
    suggest: bool,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let claude_md = dir.join("CLAUDE.md");
    if !claude_md.exists() {
        eyre::bail!(
            "No CLAUDE.md found at {}. Run `magistr init` first.",
            dir.display()
        );
    }

    let report = lint::lint_file(&claude_md).suggest()?;

    if renderer.is_machine_readable() {
        print_lint_json(&report, suggest, &dir)?;
    } else {
        print_lint_report(&report);

        // workbench.toml [commands] is seeded from project.json once and
        // then owned by a human; say so when the two no longer agree.
        if let Some(findings) = crate::scaffold::workbench_drift::workbench_drift(&dir)
            && !findings.is_empty()
        {
            eprintln!();
            for finding in &findings {
                eprintln!("{}", crate::utils::format::warning(finding));
            }
        }

        if suggest {
            let stack = detect::detect_stack(&dir).suggest()?;
            let content = std::fs::read_to_string(&claude_md)
                .map_err(|e| error::MagistrError::io(&claude_md, e))
                .suggest()?;
            let suggestions = lint::suggest::suggest_improvements(&stack, &content);
            print_suggestions(&suggestions);
        }
    }

    if !report.passed() {
        std::process::exit(1);
    }

    Ok(())
}
