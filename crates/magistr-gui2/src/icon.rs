use crate::shape::Shape;

/// The Almagest icon set: the 27 celestial-technical glyphs the blessed
/// volume ships as an SVG sprite (`docs/architecture/desktop.md#icons` — 16-unit
/// grid, 1.5-unit stroke, round caps, "every stroke terminal ends in a
/// star-dot").
///
/// # The geometry is the volume's, copied
///
/// Each variant's [`Icon::shapes`] is the sprite's own elements,
/// element for element and number for number, taken from the canonical
/// volume's `<symbol id="ic-…">` definitions. Nothing here is a
/// re-drawing: the volume is the design's fixed point, so this file is
/// a *port* whose correctness a human can check by putting it beside
/// the page. [`Icon::sprite_id`] names the symbol each variant came
/// from so that comparison needs no guessing, and
/// `every_glyph_is_the_sprites_own_geometry` refuses a variant with no
/// shapes.
///
/// # Component names and sprite identifiers
///
/// Eight glyphs are called something different in the sprite than in
/// the design's prose — `ic-orbit` is the session, `ic-rec` the
/// recording, `ic-film` the filmstrip, `ic-galaxy` the projects,
/// `ic-map` the roadmap, `ic-check` done, `ic-kbd` the keys and
/// `ic-box` the stack. The variant carries the *word the cockpit uses*,
/// because a call site names a kind and not a picture; the sprite id
/// carries the drawing's own name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Icon {
    Edit,
    Run,
    Read,
    Commit,
    Gate,
    Checkpoint,
    Session,
    Trace,
    Report,
    Recording,
    Filmstrip,
    Image,
    Signal,
    Search,
    Settings,
    Projects,
    Roadmap,
    Folder,
    Clone,
    Done,
    Alert,
    Time,
    Keys,
    Pin,
    Cost,
    Agent,
    Stack,
}

impl Icon {
    /// The design grid every number in this file is on, in its own
    /// units. `docs/architecture/desktop.md#icons`.
    pub const GRID: f32 = 16.0;

    /// Stroke width in the icon grid's units.
    pub const STROKE: f32 = 1.5;

    /// Icons in the same order as the reference sprite.
    pub const ALL: [Self; 27] = [
        Self::Edit,
        Self::Run,
        Self::Read,
        Self::Commit,
        Self::Gate,
        Self::Checkpoint,
        Self::Session,
        Self::Trace,
        Self::Report,
        Self::Recording,
        Self::Filmstrip,
        Self::Image,
        Self::Signal,
        Self::Search,
        Self::Settings,
        Self::Projects,
        Self::Roadmap,
        Self::Folder,
        Self::Clone,
        Self::Done,
        Self::Alert,
        Self::Time,
        Self::Keys,
        Self::Pin,
        Self::Cost,
        Self::Agent,
        Self::Stack,
    ];

    /// The word the cockpit calls this kind.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Run => "run",
            Self::Read => "read",
            Self::Commit => "commit",
            Self::Gate => "gate",
            Self::Checkpoint => "checkpoint",
            Self::Session => "session",
            Self::Trace => "trace",
            Self::Report => "report",
            Self::Recording => "recording",
            Self::Filmstrip => "filmstrip",
            Self::Image => "image",
            Self::Signal => "signal",
            Self::Search => "search",
            Self::Settings => "settings",
            Self::Projects => "projects",
            Self::Roadmap => "roadmap",
            Self::Folder => "folder",
            Self::Clone => "clone",
            Self::Done => "done",
            Self::Alert => "alert",
            Self::Time => "time",
            Self::Keys => "keys",
            Self::Pin => "pin",
            Self::Cost => "cost",
            Self::Agent => "agent",
            Self::Stack => "stack",
        }
    }

    /// The `<symbol>` in the volume's sprite this glyph was copied
    /// from.
    #[must_use]
    pub const fn sprite_id(self) -> &'static str {
        match self {
            Self::Edit => "ic-edit",
            Self::Run => "ic-run",
            Self::Read => "ic-read",
            Self::Commit => "ic-commit",
            Self::Gate => "ic-gate",
            Self::Checkpoint => "ic-checkpoint",
            Self::Session => "ic-orbit",
            Self::Trace => "ic-trace",
            Self::Report => "ic-report",
            Self::Recording => "ic-rec",
            Self::Filmstrip => "ic-film",
            Self::Image => "ic-image",
            Self::Signal => "ic-signal",
            Self::Search => "ic-search",
            Self::Settings => "ic-settings",
            Self::Projects => "ic-galaxy",
            Self::Roadmap => "ic-map",
            Self::Folder => "ic-folder",
            Self::Clone => "ic-clone",
            Self::Done => "ic-check",
            Self::Alert => "ic-alert",
            Self::Time => "ic-time",
            Self::Keys => "ic-kbd",
            Self::Pin => "ic-pin",
            Self::Cost => "ic-cost",
            Self::Agent => "ic-agent",
            Self::Stack => "ic-box",
        }
    }

    /// The glyph's elements on the 16-unit grid.
    #[must_use]
    pub const fn shapes(self) -> &'static [Shape] {
        match self {
            Self::Edit => EDIT,
            Self::Run => RUN,
            Self::Read => READ,
            Self::Commit => COMMIT,
            Self::Gate => GATE,
            Self::Checkpoint => CHECKPOINT,
            Self::Session => SESSION,
            Self::Trace => TRACE,
            Self::Report => REPORT,
            Self::Recording => RECORDING,
            Self::Filmstrip => FILMSTRIP,
            Self::Image => IMAGE,
            Self::Signal => SIGNAL,
            Self::Search => SEARCH,
            Self::Settings => SETTINGS,
            Self::Projects => PROJECTS,
            Self::Roadmap => ROADMAP,
            Self::Folder => FOLDER,
            Self::Clone => CLONE,
            Self::Done => DONE,
            Self::Alert => ALERT,
            Self::Time => TIME,
            Self::Keys => KEYS,
            Self::Pin => PIN,
            Self::Cost => COST,
            Self::Agent => AGENT,
            Self::Stack => STACK,
        }
    }
}

/// The volume's `ic-edit`.
const EDIT: &[Shape] = &[
    Shape::Stroke("M3.5 12.5 L11 5"),
    Shape::Stroke("M3.5 12.5 H6"),
    Shape::Star {
        cx: 12.0,
        cy: 4.0,
        r: 1.2,
    },
];

/// The volume's `ic-run`.
const RUN: &[Shape] = &[
    Shape::Stroke("M4 5 l3.5 3 L4 11 M9.5 11.5 h2.6"),
    Shape::Star {
        cx: 13.2,
        cy: 11.5,
        r: 1.1,
    },
];

/// The volume's `ic-read`.
const READ: &[Shape] = &[
    Shape::Ring {
        cx: 6.5,
        cy: 6.5,
        r: 3.5,
    },
    Shape::Stroke("M9.2 9.2 L12 12"),
    Shape::Star {
        cx: 12.6,
        cy: 12.6,
        r: 1.1,
    },
];

/// The volume's `ic-commit`.
const COMMIT: &[Shape] = &[
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 2.2,
    },
    Shape::Stroke("M8 10.2 V12 M8 4 V5.8"),
    Shape::Star {
        cx: 8.0,
        cy: 13.1,
        r: 1.1,
    },
    Shape::Star {
        cx: 8.0,
        cy: 2.9,
        r: 1.1,
    },
];

/// The volume's `ic-gate`.
const GATE: &[Shape] = &[
    Shape::Stroke("M3 11 A5 5 0 0 1 13 11"),
    Shape::Stroke("M8 11 L10.6 7"),
    Shape::Star {
        cx: 8.0,
        cy: 11.0,
        r: 1.2,
    },
];

/// The volume's `ic-checkpoint`.
const CHECKPOINT: &[Shape] = &[
    Shape::Stroke("M8 3 L12.5 12 H3.5 Z"),
    Shape::Star {
        cx: 8.0,
        cy: 9.4,
        r: 1.1,
    },
];

/// The volume's `ic-orbit`.
const SESSION: &[Shape] = &[
    Shape::Ellipse {
        cx: 8.0,
        cy: 8.0,
        rx: 5.6,
        ry: 2.6,
        deg: -18.0,
    },
    Shape::Star {
        cx: 12.7,
        cy: 5.6,
        r: 1.3,
    },
];

/// The volume's `ic-trace`.
const TRACE: &[Shape] = &[
    Shape::Stroke("M4.5 13 V7 A2.5 2.5 0 0 1 7 4.5 H10"),
    Shape::Star {
        cx: 11.8,
        cy: 4.5,
        r: 1.2,
    },
    Shape::Star {
        cx: 4.5,
        cy: 13.0,
        r: 1.2,
    },
];

/// The volume's `ic-report`.
const REPORT: &[Shape] = &[
    Shape::Rect {
        x: 4.0,
        y: 2.5,
        w: 8.0,
        h: 11.0,
        r: 1.0,
    },
    Shape::Stroke("M6 6.2 h4 M6 8.7 h4"),
];

/// The volume's `ic-rec`.
const RECORDING: &[Shape] = &[
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 5.0,
    },
    Shape::Fill("M6.8 5.9 L10.3 8 L6.8 10.1 Z"),
];

/// The volume's `ic-film`.
const FILMSTRIP: &[Shape] = &[
    Shape::Rect {
        x: 3.0,
        y: 4.0,
        w: 10.0,
        h: 8.0,
        r: 1.0,
    },
    Shape::Star {
        cx: 5.0,
        cy: 6.0,
        r: 0.7,
    },
    Shape::Star {
        cx: 5.0,
        cy: 10.0,
        r: 0.7,
    },
    Shape::Star {
        cx: 11.0,
        cy: 6.0,
        r: 0.7,
    },
    Shape::Star {
        cx: 11.0,
        cy: 10.0,
        r: 0.7,
    },
];

/// The volume's `ic-image`.
const IMAGE: &[Shape] = &[
    Shape::Rect {
        x: 3.0,
        y: 3.5,
        w: 10.0,
        h: 9.0,
        r: 1.0,
    },
    Shape::Stroke("M3.5 10.2 L6.5 7.6 L9 9.8 L12.5 7.2"),
    Shape::Star {
        cx: 10.6,
        cy: 5.7,
        r: 1.0,
    },
];

/// The volume's `ic-signal`.
const SIGNAL: &[Shape] = &[
    Shape::Star {
        cx: 8.0,
        cy: 11.0,
        r: 1.3,
    },
    Shape::Stroke("M5.2 8.4 a4 4 0 0 1 5.6 0 M3.4 6.2 a6.5 6.5 0 0 1 9.2 0"),
];

/// The volume's `ic-search`.
const SEARCH: &[Shape] = &[
    Shape::Ring {
        cx: 6.5,
        cy: 6.5,
        r: 3.5,
    },
    Shape::Stroke("M9.2 9.2 L12 12"),
    Shape::Star {
        cx: 12.6,
        cy: 12.6,
        r: 1.1,
    },
];

/// The volume's `ic-settings`.
const SETTINGS: &[Shape] = &[
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 4.8,
    },
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 1.7,
    },
    Shape::Stroke("M8 3.2 V1.9 M8 14.1 V12.8 M3.2 8 H1.9 M14.1 8 H12.8"),
];

/// The volume's `ic-galaxy`.
const PROJECTS: &[Shape] = &[
    Shape::Stroke("M4.3 10.8 L7.8 5.2 M8.2 5.2 L11.8 9.8"),
    Shape::Star {
        cx: 4.0,
        cy: 11.3,
        r: 1.2,
    },
    Shape::Star {
        cx: 8.0,
        cy: 4.7,
        r: 1.2,
    },
    Shape::Star {
        cx: 12.0,
        cy: 10.3,
        r: 1.2,
    },
];

/// The volume's `ic-map`.
const ROADMAP: &[Shape] = &[
    Shape::Stroke("M3.8 11.6 L7.6 8.2 M8.4 7.8 L12 5"),
    Shape::Stroke("M8.3 8.4 L11 11"),
    Shape::Star {
        cx: 3.5,
        cy: 12.0,
        r: 1.2,
    },
    Shape::Star {
        cx: 8.0,
        cy: 8.0,
        r: 1.2,
    },
    Shape::Star {
        cx: 12.5,
        cy: 4.5,
        r: 1.2,
    },
    Shape::Star {
        cx: 11.5,
        cy: 11.5,
        r: 1.2,
    },
];

/// The volume's `ic-folder`.
const FOLDER: &[Shape] = &[Shape::Stroke(
    "M2.5 5.2 a1 1 0 0 1 1 -1 h3 l1.5 1.5 h4.5 a1 1 0 0 1 1 1 V11 a1 1 0 0 1 -1 1 h-9 a1 1 0 0 1 -1 -1 Z",
)];

/// The volume's `ic-clone`.
const CLONE: &[Shape] = &[
    Shape::Ring {
        cx: 4.5,
        cy: 4.5,
        r: 1.6,
    },
    Shape::Ring {
        cx: 4.5,
        cy: 11.5,
        r: 1.6,
    },
    Shape::Ring {
        cx: 11.5,
        cy: 8.0,
        r: 1.6,
    },
    Shape::Stroke("M4.5 6.1 V9.9 M6.1 5.1 L9.9 7.3"),
];

/// The volume's `ic-check`.
const DONE: &[Shape] = &[
    Shape::Stroke("M3.5 8.6 L6.8 11.6 L12 4.8"),
    Shape::Star {
        cx: 12.5,
        cy: 4.0,
        r: 1.1,
    },
];

/// The volume's `ic-alert`.
const ALERT: &[Shape] = &[
    Shape::Stroke("M8 3 L13.5 12.5 H2.5 Z"),
    Shape::Stroke("M8 6.8 v2.4"),
    Shape::Star {
        cx: 8.0,
        cy: 11.0,
        r: 1.0,
    },
];

/// The volume's `ic-time`.
const TIME: &[Shape] = &[
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 5.0,
    },
    Shape::Stroke("M8 5.4 V8 l2 1.6"),
    Shape::Star {
        cx: 8.0,
        cy: 8.0,
        r: 0.9,
    },
];

/// The volume's `ic-kbd`.
const KEYS: &[Shape] = &[
    Shape::Rect {
        x: 2.5,
        y: 5.0,
        w: 11.0,
        h: 6.5,
        r: 1.0,
    },
    Shape::Stroke("M5 9.3 h6"),
    Shape::Star {
        cx: 4.8,
        cy: 7.1,
        r: 0.6,
    },
    Shape::Star {
        cx: 7.0,
        cy: 7.1,
        r: 0.6,
    },
    Shape::Star {
        cx: 9.2,
        cy: 7.1,
        r: 0.6,
    },
    Shape::Star {
        cx: 11.2,
        cy: 7.1,
        r: 0.6,
    },
];

/// The volume's `ic-pin`.
const PIN: &[Shape] = &[
    Shape::Stroke("M6.2 2.8 h3.6 l-.7 3.6 2.2 2.4 H4.7 L6.9 6.4 Z"),
    Shape::Stroke("M8 8.8 V12"),
    Shape::Star {
        cx: 8.0,
        cy: 13.1,
        r: 1.1,
    },
];

/// The volume's `ic-cost`.
const COST: &[Shape] = &[
    Shape::Ring {
        cx: 8.0,
        cy: 8.0,
        r: 5.0,
    },
    Shape::Stroke("M9.8 5.6 L6.2 10.4"),
];

/// The volume's `ic-agent`.
const AGENT: &[Shape] = &[
    Shape::Stroke("M8 2.6 V5.6 M8 10.4 V13.4 M2.6 8 H5.6 M10.4 8 H13.4"),
    Shape::Star {
        cx: 8.0,
        cy: 8.0,
        r: 1.4,
    },
];

/// The volume's `ic-box`.
const STACK: &[Shape] = &[Shape::Stroke(
    "M3 5.5 L8 3 L13 5.5 V10.5 L8 13 L3 10.5 Z M3 5.5 L8 8 L13 5.5 M8 8 V13",
)];

#[cfg(test)]
#[path = "icon.tests.rs"]
mod tests;
