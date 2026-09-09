//! What a session amounted to, for the scrollback.

use std::fmt;

use magistr_engine::FeedStatus;

use super::model::Model;
use crate::job::JobOutput;
use crate::render::Color;
use crate::theme::Theme;

/// The session's summary: what the terminal was and answered, and what
/// the cockpit counted. The entry points print it once the terminal is
/// handed back, so it lands in the scrollback after the alternate screen
/// is gone; the view's footer and producers line are built from the same
/// pieces.
///
/// Pure: `Display` over a borrowed [`Model`], nothing else.
#[derive(Clone, Copy, Debug)]
pub struct Summary<'a> {
    model: &'a Model,
}

impl<'a> Summary<'a> {
    /// The summary of `model` as it stands.
    #[must_use]
    pub const fn of(model: &'a Model) -> Self {
        Self { model }
    }

    /// `dark` or `light`, and whether `MAGISTR_THEME` forced it.
    #[must_use]
    pub fn theme(&self) -> String {
        let theme = self.model.theme();
        if theme.is_forced() {
            format!("{} ({})", theme.mode(), Theme::ENV)
        } else {
            theme.mode().to_string()
        }
    }

    /// The background the terminal reported, as `#rrggbb`, or `unknown`.
    #[must_use]
    pub fn background(&self) -> String {
        match self.model.background() {
            Some(Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
            Some(other) => format!("{other:?}"),
            None => "unknown".to_owned(),
        }
    }

    /// `kitty <flags>` when the terminal speaks the keyboard protocol,
    /// else `legacy`.
    #[must_use]
    pub fn keyboard(&self) -> String {
        self.model
            .keyboard_flags()
            .map_or("legacy".to_owned(), |flags| format!("kitty {flags}"))
    }

    /// One cell's size in pixels, once known.
    #[must_use]
    pub fn cell(&self) -> Option<String> {
        self.model
            .cell_pixels()
            .map(|(w, h)| format!("cell {w}×{h}px"))
    }

    /// The feed's attachment in a word or two.
    #[must_use]
    pub fn feed(&self) -> &'static str {
        match self.model.feed() {
            None => "no feed",
            Some(FeedStatus::Waiting) => "feed waiting",
            Some(FeedStatus::Connected { reconnect: false }) => "feed live",
            Some(FeedStatus::Connected { reconnect: true }) => "feed reattached",
            Some(FeedStatus::Disconnected) => "feed lost",
        }
    }

    /// The turn: its state and how many rows it wrapped to.
    #[must_use]
    pub fn turn(&self) -> String {
        format!(
            "turn {} ({} rows)",
            self.model.turn_state(),
            self.model.turn().len()
        )
    }

    /// The loop, once one ran: `loop iteration i/N · <gate LEDs> · <elapsed>
    /// · <ended why | paused | running>`.
    #[must_use]
    pub fn loop_status(&self) -> Option<String> {
        let status = self.model.loop_status()?;
        let mut line = format!("loop {}", status.iteration_text());
        for gate in status.gates() {
            line.push_str(" · ");
            line.push_str(&gate.text(self.model.theme().icons()));
        }
        line.push_str(" · ");
        line.push_str(&status.elapsed_text());
        line.push_str(" · ");
        match status.ended() {
            Some(why) => line.push_str(&format!("ended: {why}")),
            None if status.is_paused() => line.push_str("paused"),
            None => line.push_str("running"),
        }
        if let Some(verdict) = status.verdict() {
            line.push_str(&format!(
                " · judge ({} verdicts): {}",
                status.verdicts(),
                verdict.text()
            ));
        }
        Some(line)
    }

    /// The last job's result, if any: `<id>: <text>` or `<id> failed: <why>`.
    #[must_use]
    pub fn last_job(&self) -> Option<String> {
        self.model.last_job().map(|(id, output)| match output {
            JobOutput::Text(text) => format!("{id}: {text}"),
            JobOutput::Launcher(_) => format!("{id}: launcher data"),
            JobOutput::Galaxy(_) => format!("{id}: galaxy data"),
            JobOutput::Shelf(shelf) => format!("{id}: shelf {}", shelf.chunk),
            JobOutput::Survey(report) => format!("{id}: survey {}", report.name),
            JobOutput::Proposal(proposal) => format!("{id}: proposal {}", proposal.summary()),
            JobOutput::Born(data) => format!("{id}: born {}", data.name),
            JobOutput::Failed(why) => format!("{id} failed: {why}"),
        })
    }
}

impl fmt::Display for Summary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = self.model.size();
        write!(
            f,
            "magistr cockpit — {}×{} cells · theme {} · background {} · keyboard {}",
            size.cols,
            size.rows,
            self.theme(),
            self.background(),
            self.keyboard()
        )?;
        if let Some(cell) = self.cell() {
            write!(f, " · {cell}")?;
        }
        write!(
            f,
            "\n{} frames · {} inputs · {} agent events · {} · {} engine frames ({}) · {} jobs",
            self.model.frames(),
            self.model.inputs(),
            self.model.agent_events(),
            self.turn(),
            self.model.engine_frames(),
            self.feed(),
            self.model.jobs_done()
        )?;
        if let Some(job) = self.last_job() {
            write!(f, " · last {job}")?;
        }
        if let Some(status) = self.loop_status() {
            write!(f, "\n{status}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "summary.tests.rs"]
mod tests;
