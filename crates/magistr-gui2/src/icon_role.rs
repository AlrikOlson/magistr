/// Where an [`Icon`](crate::icon::Icon) is allowed to appear.
///
/// `docs/architecture/desktop.md#icons` states the placement law in one sentence:
/// "icons mark **kinds, navigation and empty states**; buttons stay
/// text verbs; icon-only buttons for utilities only." This enum is that
/// sentence, and the law is enforced by what it *does not* name rather
/// than by a check somebody has to remember to run — there is no
/// `Action` role, so "an icon inside a text-verb button" cannot be
/// spelled. A caller with a marked-up button has nothing to pass.
///
/// The two questions the law actually turns on are
/// [`IconRole::is_control`] — is this placement something a pointer
/// acts on? — and [`IconRole::takes_label`] — does a word sit beside
/// the mark? The law is then a property of the set: no role answers yes
/// to both, which is exactly "no icon appears inside a text-verb
/// button", and `no_role_is_both_a_control_and_labelled` is that
/// sentence as a failing test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IconRole {
    /// A mark naming what a thing *is* — a tool call's verb, a row's
    /// kind, a receipt's type. Sits beside the label that names it and
    /// is not itself clickable.
    Kind,
    /// A mark naming a *place* — a tab, a breadcrumb rung, a palette
    /// entry. Clicking the row navigates, and the place's name is
    /// beside the mark, never replaced by it.
    Nav,
    /// The mark of an empty state: the one drawing on a surface that
    /// has nothing to show yet, over the sentence that says so.
    Empty,
    /// A utility control with no room for a verb — the only icon-only button the design
    /// permits. Copy, close, expand: a thing whose meaning is conventional and whose
    /// label would be one word nobody reads twice.
    Utility,
}

impl IconRole {
    /// Every place the law names, in the order the design names them.
    pub const ALL: [Self; 4] = [Self::Kind, Self::Nav, Self::Empty, Self::Utility];

    /// Whether a pointer acts on the mark itself.
    ///
    /// A `Nav` mark is inside something clickable — a tab, a
    /// breadcrumb — but the *row* is the control and the mark is one of
    /// the things in it; only a `Utility` placement is a control that
    /// consists of the icon.
    #[must_use]
    pub const fn is_control(self) -> bool {
        matches!(self, Self::Utility)
    }

    /// Whether a word is drawn beside the mark.
    #[must_use]
    pub const fn takes_label(self) -> bool {
        !matches!(self, Self::Utility)
    }

    /// The word this role is called in the design language.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Nav => "navigation",
            Self::Empty => "empty state",
            Self::Utility => "utility",
        }
    }
}

#[cfg(test)]
#[path = "icon_role.tests.rs"]
mod tests;
