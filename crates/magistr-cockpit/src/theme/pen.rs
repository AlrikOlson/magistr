//! The cockpit's styles, drawn from one palette.

use magistr_theme::math::{ensure_contrast, lerp, relative_luminance};
use magistr_theme::{CvdMode, Palette, SemanticRole, ThemeColor, ThemeMapping, xterm256};

use super::decision::Theme;
use super::icons::Icons;
use super::severity::Severity;
use crate::pixel::Icon;
use crate::render::{Attrs, Color, Style};
use crate::turn::{Accent, RowKind, SpanStyle, Voice};
use magistr_application::sessions::{PermissionKind, ToolStatus};

/// One [`Style`] per kind of thing the cockpit paints, every colour from
/// the same [`Palette`] so the screen reads as a whole: the base the
/// whole screen is filled with, the header and footer on a raised
/// surface, the phase rail in the accent, the stage in body text, and
/// the stage's rows by [`RowKind`] — thought and notes in secondary text,
/// headings in the accent, code on the first surface, tool calls by
/// status, errors in the error role — the permission strip and its
/// buttons on the first surface (allow in the success role, always in
/// the tool-call accent, deny in the error role), the prompt line in
/// body text with its mark in the tool-call accent and the cursor cell
/// inverted, the status strip's current phase in the tool-call accent
/// with the others dimmed, and the gate LEDs by outcome (pass in the
/// success role, fail in the error role, running dimmed).
///
/// Every text pen clears WCAG AA large-text contrast (3:1) by
/// construction: its colour is run through
/// [`magistr_theme::math::ensure_contrast`] against the background it is
/// drawn on, so a palette whose accent is too close to its own surface
/// (most of the built-ins have at least one — see
/// [`Palette::accent_violations`]) still yields a legible pen.
/// `pen.tests.rs` measures the result. The four gutter accents and the
/// panel ring are block glyphs rather than text and stay exactly the
/// palette's colour, pinned by their own role tests.
///
/// Four pens carry a rule of their own rather than a colour alone,
/// because a colour is the one thing a cell cannot spend twice: the
/// active tab wears a plain rule in the raw accent, a link and a link
/// under the pointer wear a dotted one, and a failed gate and a
/// merge-conflict marker wear the curly error rule an editor draws
/// under a diagnostic. A terminal that predates `SGR 4:x` is sent the
/// plain underline for all of them — the fallback lives in the encoder,
/// so no call site here knows about it.
///
/// The four signal roles — error, warning, success, info — resolve
/// through a [`ThemeMapping`], so a [`CvdMode`] moves every pen that
/// means "failed", "needs you" or "passed" at once: under deuteranopia
/// the error pens go peach and the pass pens blue, and the contrast floor
/// still holds because it is applied after the remapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pen {
    /// The screen: body text on the base colour.
    pub base: Style,
    /// The header line: the primary accent, bold, on the first surface.
    pub header: Style,
    /// The phase rail: the tool-call accent on the base colour.
    pub rail: Style,
    /// The stage: body text on the base colour.
    pub stage: Style,
    /// The footer line: secondary text on the first surface.
    pub footer: Style,
    /// The agent's thinking: secondary text on the base colour.
    pub thought: Style,
    /// The user's echoed prompt: the tool-call accent, bold.
    pub user: Style,
    /// A heading row: the primary accent, bold, on the base colour.
    pub heading: Style,
    /// A fenced code row: body text on the first surface.
    pub code: Style,
    /// A fence marker row (`` ``` ``): secondary text on the first
    /// surface — the block's frame, quieter than its content.
    pub fence: Style,
    /// A list item row: body text on the base colour.
    pub bullet: Style,
    /// A link label: the primary accent, underlined, on the base colour;
    /// the span's url rides along as an OSC 8 hyperlink.
    pub link: Style,
    /// A tool call announced, not started: secondary text.
    pub tool_pending: Style,
    /// A tool call running: the tool-call accent.
    pub tool_running: Style,
    /// A tool call finished: body text.
    pub tool_done: Style,
    /// A tool call failed: the error accent.
    pub tool_failed: Style,
    /// An added diff line on an edit card: the success accent on the
    /// first surface.
    pub diff_add: Style,
    /// A removed diff line on an edit card: the error accent on the
    /// first surface.
    pub diff_remove: Style,
    /// A merge-conflict marker in a diff: the error accent on the first
    /// surface under a curly rule in the error colour — the shape an
    /// editor marks a diagnostic with, for the one diff line the human
    /// has to resolve by hand.
    pub diff_conflict: Style,
    /// A note from the cockpit: secondary text, italic.
    pub note: Style,
    /// An error: the error accent, bold.
    pub error: Style,
    /// Something that needs the human before the work can go on: the
    /// warning accent, bold, on the first surface.
    pub warn: Style,
    /// The permission strip: body text, bold, on the first surface.
    pub permission: Style,
    /// The Allow button: the success role, bold, on the first surface.
    pub allow: Style,
    /// The Always button: the tool-call accent, bold, on the first surface.
    pub always: Style,
    /// The Deny and Never buttons: the error accent, bold, on the first
    /// surface.
    pub deny: Style,
    /// The prompt line: body text on the base colour.
    pub prompt: Style,
    /// The prompt's mark (`›`): the tool-call accent, bold.
    pub prompt_mark: Style,
    /// The cursor cell: the base colour on body text.
    pub cursor: Style,
    /// The phase the iteration is in: the tool-call accent, bold, on the
    /// header's chrome surface — the strip is the header bar's second row.
    pub phase_current: Style,
    /// The other phases on the strip: secondary text on the chrome
    /// surface.
    pub phase_other: Style,
    /// A phase the iteration has already crossed, on the window's
    /// bottom bar: the success role on the base colour, the way the
    /// volume declares it (`research ✓` in `--green`). Its own pen
    /// rather than [`gate_pass`](Self::gate_pass) or
    /// [`phase_other`](Self::phase_other) because both of those are
    /// grounded on the chrome surface a terminal's strip sits on, and
    /// the foot sits on the base.
    pub phase_done: Style,
    /// A gate that passed: the success role, bold, on the chrome
    /// surface.
    pub gate_pass: Style,
    /// A gate that failed: the error accent, bold, on the chrome surface.
    pub gate_fail: Style,
    /// A gate still running: secondary text, italic, on the chrome
    /// surface.
    pub gate_running: Style,
    /// The active stage tab: the tool-call accent, bold, on the base —
    /// the tab bar sits inside the stage panel, not on the header bar.
    pub tab_active: Style,
    /// The other stage tabs: secondary text on the base.
    pub tab_inactive: Style,
    /// A key cap on the footer bar or the accelerator sheet: body text,
    /// bold, on the second surface — a chip that rises off its bar.
    pub chip_key: Style,
    /// A panel's border ring: the palette's border surface on the base
    /// colour — structure, deliberately quieter than any text pen.
    pub panel_border: Style,
    /// The focused panel's border ring: the tool-call accent on the base
    /// colour, so focus reads at a glance.
    pub panel_focus: Style,
    /// The user's gutter bar on the turn feed: the tool-call accent on
    /// the base — the same voice as the echoed prompt's pen.
    pub accent_user: Style,
    /// The agent's gutter bar: the primary accent on the base.
    pub accent_agent: Style,
    /// The thinking gutter bar: secondary text on the base — quiet, like
    /// the thought pen it runs beside.
    pub accent_thought: Style,
    /// A tool card's gutter bar: peach on the base — hue-distant from
    /// both the user's mauve and the agent's blue at one cell wide.
    pub accent_tool: Style,
    /// The ground a hovered row or card is raised onto: the palette's
    /// second surface, the same cap the footer's key chips wear. Not a
    /// pen of its own — [`Pen::raise`] swaps it under whatever style the
    /// widget already had and re-floors the foreground against it.
    pub hover_ground: Color,
    /// The four severity bands' body pens, in [`Severity::ALL`]'s
    /// worst-first order: body text on the band's **tinted ground** —
    /// the base lerped [`SEVERITY_TINT`] of the way toward the band's
    /// signal hue, the severity treatment and the reason
    /// there is no border colour anywhere near this type.
    ///
    /// An array and not four named fields, because the one thing a
    /// caller ever does with them is index by a band it already holds;
    /// [`Pen::severity`] is that index, and [`Pen::all`] sweeps the
    /// array so every band pays the contrast floor like the rest.
    severity: [Style; Severity::ALL.len()],
    /// The same four bands' cap pens: the band's hue itself on the same
    /// tinted ground, for the icon and the caps beside it. Read through
    /// [`Pen::severity_cap`].
    severity_cap: [Style; Severity::ALL.len()],
    /// The palette's sixteen ANSI colours, for a program's own
    /// `SGR 30–37` / `90–97` on a terminal pane — read through
    /// [`Pen::term`], where the floor is applied per cell.
    ansi: [ThemeColor; 16],
    /// The glyph vocabulary the surfaces draw their marks from — a pen
    /// has a colour and a mark, and both come from the theme.
    pub icons: &'static Icons,
}

impl Pen {
    /// The floor every text pen is held to: WCAG AA for large text and
    /// UI elements, which is what a cell of a terminal grid is.
    pub const MIN_CONTRAST: f64 = 3.0;

    /// The glyphs that are *structure* rather than text — the panel
    /// ring, the gutter bar, the progress bar's cells and the rail's
    /// divider — which are drawings that happen to be spelled with
    /// characters and deliberately sit under
    /// [`MIN_CONTRAST`](Self::MIN_CONTRAST).
    ///
    /// `▀` is an image's pixels rather than a glyph: its colours are the
    /// screenshot's, and a screenshot has no contrast contract.
    ///
    /// Declared here rather than inside a test because two renderers ask
    /// the same question — the cell sweep in `view.tests.rs` and the
    /// native painter's run sweep in `magistr-gui2` — and two copies of
    /// an exemption list is one copy too many.
    pub const STRUCTURAL: &'static str = "─│╭╮╰╯▎▰▱▀";

    /// The pens for `palette`, with no colour vision remapping.
    #[must_use]
    pub fn of(palette: &Palette) -> Self {
        Self::with(palette, None)
    }

    /// The pens for `palette`, with the signal roles remapped for `cvd`.
    #[must_use]
    pub fn with(palette: &Palette, cvd: Option<CvdMode>) -> Self {
        let default = ThemeMapping::default();
        let mapping = cvd.map_or(default.clone(), |mode| mode.apply_to_mapping(&default));
        // A text pen: the colour nudged until it clears the large-text
        // floor on the ground it is drawn over.
        let text = |color: ThemeColor, ground: ThemeColor| {
            Style::new()
                .fg(rgb(ensure_contrast(color, ground, Self::MIN_CONTRAST)))
                .bg(rgb(ground))
        };
        let on_base = |color: ThemeColor| text(color, palette.base);
        let on_surface = |color: ThemeColor| text(color, palette.surface0).attrs(Attrs::BOLD);
        // A signal pen: the role's colour under the mapping in force.
        let role = |role: SemanticRole| mapping.resolve(role, palette);
        // A structural pen: block glyphs, not text, so the palette's
        // colour stands exactly as chosen.
        let block = |color: ThemeColor| Style::new().fg(rgb(color)).bg(rgb(palette.base));
        // A severity band's ground: the base carrying the band's hue at
        // [`SEVERITY_TINT`], which supplies the tinted background
        // — a wash the card is made of rather than a stripe laid beside
        // it. The quiet band tints toward secondary text, so it reads as
        // a raised surface and not as a fourth colour.
        let tint = |severity: Severity| {
            let hue = severity.role().map_or(palette.subtext0, role);
            lerp(palette.base, hue, SEVERITY_TINT)
        };
        let base = on_base(palette.text);
        Self {
            header: on_surface(palette.blue),
            rail: on_base(palette.mauve),
            stage: base.clone(),
            footer: text(palette.subtext0, palette.surface0),
            thought: on_base(palette.subtext0),
            user: on_base(palette.mauve).attrs(Attrs::BOLD),
            heading: on_base(palette.blue).attrs(Attrs::BOLD),
            code: text(palette.text, palette.surface0),
            fence: text(palette.subtext0, palette.surface0),
            bullet: base.clone(),
            link: on_base(palette.blue).attrs(Attrs::UNDERLINE_DOTTED),
            tool_pending: on_base(palette.subtext0),
            tool_running: on_base(palette.mauve),
            tool_done: base.clone(),
            tool_failed: on_base(role(SemanticRole::Error)),
            diff_add: text(role(SemanticRole::Success), palette.surface0),
            diff_remove: text(role(SemanticRole::Error), palette.surface0),
            diff_conflict: text(role(SemanticRole::Error), palette.surface0)
                .attrs(Attrs::BOLD | Attrs::UNDERLINE_CURLY)
                .underline_color(rgb(role(SemanticRole::Error))),
            note: on_base(palette.subtext0).attrs(Attrs::ITALIC),
            error: on_base(role(SemanticRole::Error)).attrs(Attrs::BOLD),
            warn: on_surface(role(SemanticRole::Warning)),
            permission: on_surface(palette.text),
            allow: on_surface(role(SemanticRole::Success)),
            always: on_surface(palette.mauve),
            deny: on_surface(role(SemanticRole::Error)),
            prompt: base.clone(),
            prompt_mark: on_base(palette.mauve).attrs(Attrs::BOLD),
            cursor: Style::new().fg(rgb(palette.base)).bg(rgb(palette.text)),
            phase_current: on_surface(palette.mauve),
            phase_other: text(palette.subtext0, palette.surface0),
            phase_done: on_base(role(SemanticRole::Success)),
            gate_pass: on_surface(role(SemanticRole::Success)),
            gate_fail: on_surface(role(SemanticRole::Error))
                .attrs(Attrs::BOLD | Attrs::UNDERLINE_CURLY)
                .underline_color(rgb(role(SemanticRole::Error))),
            gate_running: text(palette.subtext0, palette.surface0).attrs(Attrs::ITALIC),
            tab_active: on_base(palette.mauve)
                .attrs(Attrs::BOLD | Attrs::UNDERLINE)
                .underline_color(rgb(palette.mauve)),
            tab_inactive: on_base(palette.subtext0),
            chip_key: text(palette.text, palette.surface1).attrs(Attrs::BOLD),
            panel_border: block(palette.surface1),
            panel_focus: block(palette.mauve),
            accent_user: block(palette.mauve),
            accent_agent: block(palette.blue),
            accent_thought: block(palette.subtext0),
            accent_tool: block(palette.peach),
            hover_ground: rgb(palette.surface1),
            severity: Severity::ALL.map(|band| text(palette.text, tint(band))),
            severity_cap: Severity::ALL
                .map(|band| text(band.role().map_or(palette.subtext0, role), tint(band))),
            ansi: magistr_theme::ansi16(palette),
            icons: &Icons::UNICODE,
            base,
        }
    }

    /// The pens `theme` asks for: its palette, its colour vision
    /// remapping and its glyph vocabulary in one.
    #[must_use]
    pub fn for_theme(theme: &Theme) -> Self {
        Self::with(&theme.palette(), theme.cvd()).glyphs(theme.icons())
    }

    /// The same pens drawing their marks from `icons`.
    #[must_use]
    pub const fn glyphs(mut self, icons: &'static Icons) -> Self {
        self.icons = icons;
        self
    }

    /// The pen for a turn row's gutter bar. Structural like the panel
    /// ring — the bar is a block glyph, not text — so these sit outside
    /// [`Pen::all`]'s AA sweep, pinned by their own role test instead.
    #[must_use]
    pub const fn accent(&self, accent: Accent) -> &Style {
        match accent {
            Accent::User => &self.accent_user,
            Accent::Agent => &self.accent_agent,
            Accent::Thought => &self.accent_thought,
            Accent::Tool => &self.accent_tool,
            Accent::Meta => &self.stage,
        }
    }

    /// The pen for a panel's ring: the accent when the panel has focus,
    /// the quiet border surface otherwise.
    #[must_use]
    pub const fn panel(&self, focused: bool) -> &Style {
        if focused {
            &self.panel_focus
        } else {
            &self.panel_border
        }
    }

    /// The colour a vector glyph is drawn in: keyed to what the glyph
    /// means, so a tick is the success colour whatever line it sits on,
    /// a pin is violet and a checkpoint amber — the status vocabulary
    /// the text pens already grade, read off them.
    #[must_use]
    pub const fn ink(&self, icon: Icon) -> Color {
        match icon {
            Icon::FoldClosed
            | Icon::FoldOpen
            | Icon::Collapse
            | Icon::Pending
            | Icon::Planned
            | Icon::Waiting
            | Icon::Skipped => self.note.fg,
            Icon::Done => self.diff_add.fg,
            Icon::Failed | Icon::Blocker | Icon::Stopped => self.error.fg,
            Icon::Ready | Icon::Signal | Icon::Undock | Icon::File | Icon::Edit | Icon::Keys => {
                self.heading.fg
            }
            Icon::Blocked | Icon::Warning | Icon::Checkpoint => self.warn.fg,
            Icon::InProgress | Icon::Here | Icon::Running | Icon::Spinner(_) => self.rail.fg,
            Icon::Pinned | Icon::Pin => self.always.fg,
            Icon::Proposal => self.accent_tool.fg,
            Icon::Bullet => self.bullet.fg,
        }
    }

    /// The pen for a phase on the strip: bright when it is the current
    /// one, dim otherwise.
    #[must_use]
    pub const fn phase(&self, current: bool) -> &Style {
        if current {
            &self.phase_current
        } else {
            &self.phase_other
        }
    }

    /// The pen for a stage tab: bright when it is the active one, dim
    /// otherwise.
    #[must_use]
    pub const fn tab(&self, active: bool) -> &Style {
        if active {
            &self.tab_active
        } else {
            &self.tab_inactive
        }
    }

    /// The pen for a gate LED by its outcome (`None` while it runs).
    #[must_use]
    pub const fn gate(&self, passed: Option<bool>) -> &Style {
        match passed {
            None => &self.gate_running,
            Some(true) => &self.gate_pass,
            Some(false) => &self.gate_fail,
        }
    }

    /// The pen for a stage row of `kind`.
    #[must_use]
    pub const fn row(&self, kind: RowKind) -> &Style {
        match kind {
            RowKind::Prose(Voice::Agent) => &self.stage,
            RowKind::Prose(Voice::Thought) => &self.thought,
            RowKind::Prose(Voice::User) => &self.user,
            RowKind::Heading => &self.heading,
            RowKind::Code => &self.code,
            RowKind::Fence => &self.fence,
            RowKind::Bullet => &self.bullet,
            RowKind::Tool(ToolStatus::Pending) => &self.tool_pending,
            RowKind::Tool(ToolStatus::InProgress) => &self.tool_running,
            RowKind::Tool(ToolStatus::Completed) => &self.tool_done,
            RowKind::Tool(ToolStatus::Failed) => &self.tool_failed,
            RowKind::Check(true) => &self.tool_done,
            RowKind::Check(false) => &self.tool_failed,
            RowKind::DiffAdd => &self.diff_add,
            RowKind::DiffRemove => &self.diff_remove,
            RowKind::Conflict => &self.diff_conflict,
            RowKind::EditButtons => &self.code,
            // An image row is filled cell by cell; the pen only
            // supplies the ground the placement sits on.
            RowKind::Image => &self.stage,
            // A pane row is filled cell by cell too, on the code pen's
            // ground — the well a program's output sits in.
            RowKind::Pane => &self.code,
            RowKind::Event => &self.note,
            RowKind::Note => &self.note,
            RowKind::Error => &self.error,
        }
    }

    /// The pen for an inline span of `style` on a row of `kind`: the
    /// row's pen with the span's attribute laid on, the code pen for an
    /// inline code run, the link pen (carrying the url) for a link.
    #[must_use]
    pub fn span(&self, kind: RowKind, style: &SpanStyle) -> Style {
        match style {
            SpanStyle::Bold => lay(self.row(kind).clone(), Attrs::BOLD),
            SpanStyle::Italic => lay(self.row(kind).clone(), Attrs::ITALIC),
            SpanStyle::Code => self.code.clone(),
            SpanStyle::Link(url) => self.link.clone().link(url.as_str()),
            // A highlighter's colour is a third party's palette, and a
            // palette is a colour identity rather than a set of
            // guaranteed foregrounds — syntect's `InspiredGitHub` puts
            // its call-expression green at 1.97 on the code pen's light
            // ground. The floor is therefore applied here, at the
            // consumer, exactly as `magistr_theme::ink` protects the
            // GUI's stylesheet rather than retuning Catppuccin
            // (theme-latte-text-contrast): one site, and every syntax
            // theme and every user palette is covered by it.
            SpanStyle::Fg(color) => {
                let row = self.row(kind).clone();
                let floored = match (theme_color(*color), theme_color(row.bg)) {
                    (Some(fg), Some(bg)) => rgb(ensure_contrast(fg, bg, Self::MIN_CONTRAST)),
                    _ => *color,
                };
                row.fg(floored)
            }
        }
    }

    /// The pen for one cell of a program's terminal: the program's own colours — an ANSI
    /// index through the palette's sixteen, a 256-colour index through
    /// xterm's cube and greys, truecolor as itself, the terminal's
    /// default as the code pen's — with the foreground floored against
    /// its ground, so a dim grey stays legible and a red stays the
    /// program's red. Reverse video swaps the pair before the floor and
    /// is spent there; every other attribute rides along.
    #[must_use]
    pub fn term(&self, fg: Color, bg: Color, attrs: Attrs) -> Style {
        let ink = self.term_color(fg, true);
        let ground = self.term_color(bg, false);
        let (ink, ground) = if attrs.contains(Attrs::REVERSE) {
            (ground, ink)
        } else {
            (ink, ground)
        };
        Style::new()
            .fg(rgb(ensure_contrast(ink, ground, Self::MIN_CONTRAST)))
            .bg(rgb(ground))
            .attrs(attrs.without(Attrs::REVERSE))
    }

    /// A program's colour as the palette's: the default is the code
    /// pen's own, an ANSI index the palette's sixteen, the rest of the
    /// 256 xterm's table, truecolor itself.
    fn term_color(&self, color: Color, foreground: bool) -> ThemeColor {
        match color {
            Color::Default => {
                let own = if foreground {
                    self.code.fg
                } else {
                    self.code.bg
                };
                theme_color(own).unwrap_or(self.ansi[if foreground { 15 } else { 0 }])
            }
            Color::Indexed(n) if n < 16 => self.ansi[usize::from(n)],
            Color::Indexed(n) => xterm256(n),
            Color::Rgb(r, g, b) => ThemeColor::new(r, g, b),
        }
    }

    /// `style` lifted toward the bright extreme of its ground — white on
    /// a dark ground, black on a light one — by `t`'s place on a raised
    /// cosine: unchanged at `t == 0` and `t == 1`, fully lifted at the
    /// middle. The shimmer that travels down the streaming block's
    /// gutter and the flash a panel ring gives on arrival. Structural,
    /// like the accents and the ring: no contrast floor.
    #[must_use]
    pub fn shimmer(&self, style: &Style, t: f32) -> Style {
        lift(style, wave(t) * SHIMMER_LIFT, None)
    }

    /// `style` pulsed once — lifted toward the bright extreme and back
    /// over `t` in `0..=1` — for a tab switch or a dock arrival. A text
    /// pen: the result is held to [`Pen::MIN_CONTRAST`]. At `t == 0`
    /// and `t == 1` the style is exactly itself, so a settled frame is
    /// the same frame whether or not a pulse just ran.
    #[must_use]
    pub fn pulse(&self, style: &Style, t: f32) -> Style {
        lift(style, wave(t) * PULSE_LIFT, Some(Self::MIN_CONTRAST))
    }

    /// `style` faded in from the dim pen: its foreground lerped from the
    /// secondary-text colour toward its own over `t` in `0..=1`, and
    /// held to [`Pen::MIN_CONTRAST`] on the way, so an arriving row is
    /// quiet but never illegible. At `t == 1` the style is exactly
    /// itself, so a settled row is byte for byte the row it was before
    /// it arrived. The fade a new turn row and a new tool card come in
    /// on.
    #[must_use]
    pub fn fade(&self, style: &Style, t: f32) -> Style {
        toward(
            style,
            self.note.fg,
            1.0 - t.clamp(0.0, 1.0),
            Self::MIN_CONTRAST,
        )
    }

    /// `style` popped once — [`Pen::pulse`]'s single wave with BOLD laid
    /// on while the wave is at least half up. The flash a gate that
    /// settles, a plan task that completes and a roadmap chunk that
    /// completes each get; a failing gate pops on its own error pen, so
    /// the same wave reads as an error flash. At `t == 0` and `t == 1`
    /// the style is exactly itself, weight included. A pen that is bold
    /// already — the gate LEDs are — pops in colour alone.
    #[must_use]
    pub fn pop(&self, style: &Style, t: f32) -> Style {
        let pulsed = self.pulse(style, t);
        if wave(t) >= POP_BOLD_FROM {
            lay(pulsed, Attrs::BOLD)
        } else {
            pulsed
        }
    }

    /// `style` raised onto [`Pen::hover_ground`]: the second surface
    /// under it, its foreground nudged back over the contrast floor
    /// against the new ground. What a row or a card wears while the
    /// pointer is on it — a band that lifts the whole widget off the
    /// screen without moving a cell.
    #[must_use]
    pub fn raise(&self, style: &Style) -> Style {
        let (Some(fg), Some(ground)) = (theme_color(style.fg), theme_color(self.hover_ground))
        else {
            return style.clone();
        };
        style
            .clone()
            .fg(rgb(ensure_contrast(fg, ground, Self::MIN_CONTRAST)))
            .bg(self.hover_ground)
    }

    /// `style` lifted: its foreground moved toward the bright extreme of
    /// its own ground and BOLD laid on, held to [`Pen::MIN_CONTRAST`].
    /// What a chip, a tab or a button wears under the pointer: the same
    /// move [`Pen::pulse`] travels through, held at a gentler height —
    /// enough to read as raised, not so far that a signal pen washes out
    /// while the pointer rests on it.
    #[must_use]
    pub fn lift(&self, style: &Style) -> Style {
        lay(
            lift(style, HOVER_LIFT, Some(Self::MIN_CONTRAST)),
            Attrs::BOLD,
        )
    }

    /// `style` held down: its ground and its ink swapped.
    ///
    /// Composed onto whichever hover treatment the widget's
    /// [`HoverRole`](crate::cockpit::HoverRole) already gave it, so a
    /// pressed row, chip and link each read as the raised version of
    /// themselves *filled in* rather than as three separate states to
    /// design. A swap and not a third colour move for two reasons: it
    /// is unmistakable at a glance, which is what a momentary state has
    /// to be; and it carries **no contrast risk at all**, because the
    /// pair on screen is the pair the pen already cleared the floor with
    /// — the renderer swaps them and the ratio is symmetric.
    #[must_use]
    pub fn press(&self, style: &Style) -> Style {
        lay(style.clone(), Attrs::REVERSE)
    }

    /// `style` lifted and underlined: what a trace revision link wears
    /// under the pointer, so a link reads as a link rather than as one
    /// more button. The rule is dotted, the same shape [`Pen::link`]
    /// wears — a link under the pointer is still a link.
    #[must_use]
    pub fn underline(&self, style: &Style) -> Style {
        lay(self.lift(style), Attrs::UNDERLINE_DOTTED)
    }

    /// The body pen of a severity band: body text on the band's tinted
    /// ground (`docs/architecture/desktop.md#themes`). The whole card is drawn in
    /// it, so the hue is carried by the *surface* and never by a strip.
    #[must_use]
    pub fn severity(&self, severity: Severity) -> &Style {
        &self.severity[severity as usize]
    }

    /// The cap pen of a severity band: the band's own hue on that same
    /// tinted ground — the coloured icon cap the treatment's other half
    /// is made of.
    #[must_use]
    pub fn severity_cap(&self, severity: Severity) -> &Style {
        &self.severity_cap[severity as usize]
    }

    /// Every pen with its name, for the contrast measurement.
    #[must_use]
    pub fn all(&self) -> [(&'static str, &Style); 46] {
        [
            ("base", &self.base),
            ("header", &self.header),
            ("rail", &self.rail),
            ("stage", &self.stage),
            ("footer", &self.footer),
            ("thought", &self.thought),
            ("user", &self.user),
            ("heading", &self.heading),
            ("code", &self.code),
            ("fence", &self.fence),
            ("bullet", &self.bullet),
            ("link", &self.link),
            ("tool_pending", &self.tool_pending),
            ("tool_running", &self.tool_running),
            ("tool_done", &self.tool_done),
            ("tool_failed", &self.tool_failed),
            ("diff_add", &self.diff_add),
            ("diff_remove", &self.diff_remove),
            ("diff_conflict", &self.diff_conflict),
            ("note", &self.note),
            ("error", &self.error),
            ("warn", &self.warn),
            ("permission", &self.permission),
            ("allow", &self.allow),
            ("always", &self.always),
            ("deny", &self.deny),
            ("prompt", &self.prompt),
            ("prompt_mark", &self.prompt_mark),
            ("cursor", &self.cursor),
            ("phase_current", &self.phase_current),
            ("phase_other", &self.phase_other),
            ("phase_done", &self.phase_done),
            ("gate_pass", &self.gate_pass),
            ("gate_fail", &self.gate_fail),
            ("gate_running", &self.gate_running),
            ("tab_active", &self.tab_active),
            ("tab_inactive", &self.tab_inactive),
            ("chip_key", &self.chip_key),
            ("severity_alert", self.severity(Severity::Alert)),
            ("severity_alert_cap", self.severity_cap(Severity::Alert)),
            ("severity_attention", self.severity(Severity::Attention)),
            (
                "severity_attention_cap",
                self.severity_cap(Severity::Attention),
            ),
            ("severity_good", self.severity(Severity::Good)),
            ("severity_good_cap", self.severity_cap(Severity::Good)),
            ("severity_quiet", self.severity(Severity::Quiet)),
            ("severity_quiet_cap", self.severity_cap(Severity::Quiet)),
        ]
    }

    /// The pen for a permission button of `kind`.
    #[must_use]
    pub const fn button(&self, kind: PermissionKind) -> &Style {
        match kind {
            PermissionKind::AllowOnce => &self.allow,
            PermissionKind::AllowAlways => &self.always,
            PermissionKind::RejectOnce | PermissionKind::RejectAlways => &self.deny,
        }
    }
}

/// How far the shimmer lifts a colour toward the extreme at its crest.
const SHIMMER_LIFT: f32 = 0.45;
/// How far a pulse lifts a colour toward the extreme at its crest.
const PULSE_LIFT: f32 = 0.4;
/// How far a hover lifts a chip's or a button's colour. Gentler than a
/// pulse's crest ([`PULSE_LIFT`]) because a hover is *held*: a wash that
/// reads as a flash in three ticks reads as a lost hue when the pointer
/// rests, and the Allow button has to stay green while it brightens.
const HOVER_LIFT: f32 = 0.22;
/// How far the base is carried toward a severity band's hue to make
/// the band's ground. Low: severity is a *tint*, a surface that
/// has taken a colour, and a ground loud enough to compete with the
/// cap's mark would be the strip the same rule forbids, drawn wide.
const SEVERITY_TINT: f64 = 0.14;
/// How far up its wave a pop has to be to carry the extra weight — half
/// the crest, so the foot of the wave at either end is the plain pen.
const POP_BOLD_FROM: f32 = 0.5;

/// A raised cosine over one period: `0` at `t == 0` and `t == 1`, `1`
/// at the middle — a bump that starts and ends where it was.
fn wave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.5 - 0.5 * libm::cosf(std::f32::consts::TAU * t)
}

/// `style` with its foreground moved `amount` of the way toward the
/// extreme its ground contrasts with, then — when `floor` names one —
/// nudged back over the contrast floor. A style without truecolor on
/// both sides is returned as it is.
fn lift(style: &Style, amount: f32, floor: Option<f64>) -> Style {
    let (Some(fg), Some(bg)) = (theme_color(style.fg), theme_color(style.bg)) else {
        return style.clone();
    };
    let extreme = if relative_luminance(bg) < 0.5 {
        ThemeColor::new(255, 255, 255)
    } else {
        ThemeColor::new(0, 0, 0)
    };
    let mut lifted = lerp(fg, extreme, f64::from(amount));
    if let Some(floor) = floor {
        lifted = ensure_contrast(lifted, bg, floor);
    }
    style.clone().fg(rgb(lifted))
}

/// `style` with its foreground moved `amount` of the way toward `other`,
/// then nudged back over the contrast `floor`. At `amount == 0` the
/// style is returned exactly as it is — its own colour already clears
/// the floor by construction, so the fade lands byte for byte on the pen
/// it faded in. A style without truecolor on both sides, or a target
/// that is not truecolor, is returned as it is.
fn toward(style: &Style, other: Color, amount: f32, floor: f64) -> Style {
    if amount <= 0.0 {
        return style.clone();
    }
    let (Some(fg), Some(bg), Some(other)) = (
        theme_color(style.fg),
        theme_color(style.bg),
        theme_color(other),
    ) else {
        return style.clone();
    };
    let moved = lerp(fg, other, f64::from(amount.min(1.0)));
    style.clone().fg(rgb(ensure_contrast(moved, bg, floor)))
}

/// The renderer's truecolor back as a palette colour.
fn theme_color(color: Color) -> Option<ThemeColor> {
    match color {
        Color::Rgb(r, g, b) => Some(ThemeColor::new(r, g, b)),
        Color::Default | Color::Indexed(_) => None,
    }
}

/// `style` with `attr` laid on top of the attributes it has.
fn lay(style: Style, attr: Attrs) -> Style {
    let attrs = style.attrs | attr;
    style.attrs(attrs)
}

/// A palette colour as the renderer's truecolor.
fn rgb(color: ThemeColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

#[cfg(test)]
#[path = "pen.tests.rs"]
mod tests;
