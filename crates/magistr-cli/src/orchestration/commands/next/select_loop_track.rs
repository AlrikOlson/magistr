//! Track selection before terminal takeover; recovered runs retain their scope.

use crate::workflow::iteration::IterationLoop;
use std::io::{self, BufRead, IsTerminal, Write};

pub(super) fn select_loop_track(
    run: &mut IterationLoop,
    requested: Option<&str>,
    interactive: bool,
) -> color_eyre::eyre::Result<()> {
    if requested.is_some() {
        return run.select_track(requested).map_err(Into::into);
    }
    if run.is_recovered() || !interactive || !io::stdin().is_terminal() {
        return Ok(());
    }
    let Some(engines) = run.native() else {
        return Ok(());
    };
    let tracks = engines.roadmap_tracks();
    if tracks.is_empty() {
        return Ok(());
    }
    let chosen = choose(&tracks, &mut io::stdin().lock(), &mut io::stderr().lock())?;
    run.select_track(chosen.as_deref())?;
    Ok(())
}

fn choose(
    tracks: &[String],
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> color_eyre::eyre::Result<Option<String>> {
    writeln!(output, "Choose a track to loop on:")?;
    for (index, track) in tracks.iter().enumerate() {
        writeln!(output, "  {}. {track}", index + 1)?;
    }
    writeln!(output, "  0. All tracks")?;
    loop {
        write!(output, "Track number: ")?;
        output.flush()?;
        let mut answer = String::new();
        if input.read_line(&mut answer)? == 0 {
            color_eyre::eyre::bail!(
                "No track was chosen. Run again with --track followed by a track name."
            );
        }
        match answer.trim().parse::<usize>() {
            Ok(0) => return Ok(None),
            Ok(index) if index <= tracks.len() => return Ok(Some(tracks[index - 1].clone())),
            _ => writeln!(output, "Choose one of the listed numbers.")?,
        }
    }
}

#[cfg(test)]
#[path = "select_loop_track.tests.rs"]
mod tests;
