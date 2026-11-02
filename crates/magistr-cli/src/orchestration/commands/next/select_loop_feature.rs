//! Feature selection before terminal takeover; recovered runs retain their scope.

use crate::workflow::iteration::IterationLoop;
use std::io::{self, BufRead, IsTerminal, Write};

pub(super) fn select_loop_feature(
    run: &mut IterationLoop,
    requested: Option<&str>,
    interactive: bool,
) -> color_eyre::eyre::Result<()> {
    if requested.is_some() {
        return run.select_track(requested).map_err(Into::into);
    }
    run.select_track(None)?;
    if run.is_recovered() || !interactive || !io::stdin().is_terminal() {
        return Ok(());
    }
    let Some(engines) = run.native() else {
        return Ok(());
    };
    let features = engines
        .plan_workstreams()
        .map_err(|error| color_eyre::eyre::eyre!(error))?
        .current_names();
    if features.is_empty() {
        return Ok(());
    }
    let chosen = choose(&features, &mut io::stdin().lock(), &mut io::stderr().lock())?;
    run.select_track(chosen.as_deref())?;
    Ok(())
}

fn choose(
    features: &[String],
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> color_eyre::eyre::Result<Option<String>> {
    writeln!(output, "Choose a feature to loop on:")?;
    for (index, feature) in features.iter().enumerate() {
        writeln!(output, "  {}. {feature}", index + 1)?;
    }
    writeln!(output, "  0. All features")?;
    loop {
        write!(output, "Feature number: ")?;
        output.flush()?;
        let mut answer = String::new();
        if input.read_line(&mut answer)? == 0 {
            color_eyre::eyre::bail!(
                "No feature was chosen. Run again with --feature followed by a feature name."
            );
        }
        match answer.trim().parse::<usize>() {
            Ok(0) => return Ok(None),
            Ok(index) if index <= features.len() => return Ok(Some(features[index - 1].clone())),
            _ => writeln!(output, "Choose one of the listed numbers.")?,
        }
    }
}

#[cfg(test)]
#[path = "select_loop_feature.tests.rs"]
mod tests;
