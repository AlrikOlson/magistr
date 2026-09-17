use crate::element_id::ElementId;

/// One action a screen offers: a sans-serif **verb** on a raised rect (`.btn7`),
/// optionally carrying its key cap (`.kbd7`) and the negative accent (`.btn7.neg` —
/// `Deny`).
///
/// Deliberately *not* able to say "primary": the design allows one filled primary per
/// screen — the one lit exit — so primary-ness lives on
/// [`ActionRow`](crate::action_row::ActionRow)'s single slot, where the type can count
/// it, never on the action itself where a list could carry two.
pub struct Action {
    pub(crate) verb: String,
    pub(crate) cap: Option<String>,
    pub(crate) negative: bool,
    pub(crate) accent: bool,
    pub(crate) disabled: bool,
    pub(crate) id: ElementId,
}

impl Action {
    /// An action saying `verb`, answering on `id`.
    #[must_use]
    pub fn new(verb: impl Into<String>, id: ElementId) -> Self {
        Self {
            verb: verb.into(),
            cap: None,
            negative: false,
            accent: false,
            disabled: false,
            id,
        }
    }

    /// The key that fires this action, shown as a data-pill cap inside
    /// the button's own rect.
    #[must_use]
    pub fn cap(mut self, key: impl Into<String>) -> Self {
        self.cap = Some(key.into());
        self
    }

    /// The destructive voice — `Deny`.
    #[must_use]
    pub fn negative(mut self) -> Self {
        self.negative = true;
        self
    }

    /// The accent voice — `.btn7.acc`, the mauve raise a signal card's `Promote` wears.
    /// Not the primary: one lit exit is counted in the success hue, and this is the
    /// attention accent beside it.
    #[must_use]
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }

    /// This action cannot currently be taken: it answers no pointer,
    /// takes no focus, and wears the flat disabled face.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}
