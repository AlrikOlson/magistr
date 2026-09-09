use magistr_theme::{ThemeColor, catppuccin_latte, catppuccin_mocha, math};

use super::*;
use crate::render::{Attrs, Underline};

fn theme_color(color: Color) -> ThemeColor {
    match color {
        Color::Rgb(r, g, b) => ThemeColor::new(r, g, b),
        other => panic!("{other:?} is not a palette colour"),
    }
}

#[test]
fn the_dark_pen_is_drawn_from_mocha() {
    let pen = Pen::of(&catppuccin_mocha());
    assert_eq!(pen.base.bg, Color::Rgb(30, 30, 46));
    assert_eq!(pen.base.fg, Color::Rgb(205, 214, 244));
    assert_eq!(pen.header.fg, Color::Rgb(137, 180, 250));
    assert_eq!(pen.header.bg, Color::Rgb(49, 50, 68));
    assert!(pen.header.attrs.contains(Attrs::BOLD));
    assert_eq!(pen.rail.fg, Color::Rgb(203, 166, 247));
    assert_eq!(pen.rail.bg, pen.base.bg);
    assert_eq!(pen.stage, pen.base);
    assert_eq!(pen.footer.fg, Color::Rgb(166, 173, 200));
    assert_eq!(pen.footer.bg, pen.header.bg);
}

#[test]
fn the_light_pen_differs_and_every_pen_has_a_palette_colour_on_both_sides() {
    let dark = Pen::of(&catppuccin_mocha());
    let light = Pen::of(&catppuccin_latte());
    assert_ne!(dark, light);
    assert_eq!(light.base.bg, Color::Rgb(239, 241, 245));
    for pen in [&dark, &light] {
        for (_, style) in pen.all() {
            assert!(matches!(style.fg, Color::Rgb(..)), "{style:?}");
            assert!(matches!(style.bg, Color::Rgb(..)), "{style:?}");
            assert!(style.link.is_none());
        }
    }
}

#[test]
fn every_pen_reads_at_wcag_aa_large_text_contrast_in_both_palettes() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        for (name, style) in pen.all() {
            let ratio = math::contrast_ratio(theme_color(style.fg), theme_color(style.bg));
            assert!(
                ratio >= 3.0,
                "{}: {name} contrast {ratio:.2} < 3.0 ({:?} on {:?})",
                palette.name,
                style.fg,
                style.bg
            );
        }
    }
}

#[test]
fn the_panel_pens_are_structure_border_quiet_focus_accent() {
    // The border ring is structure, not text: it deliberately sits below
    // the AA text-contrast floor, so it stays out of `Pen::all` — this
    // test pins its palette roles instead.
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        let border = &pen.panel_border;
        let focus = &pen.panel_focus;
        assert_eq!(theme_color(border.fg), palette.surface1);
        assert_eq!(theme_color(border.bg), palette.base);
        assert_eq!(theme_color(focus.fg), palette.mauve);
        assert_eq!(theme_color(focus.bg), palette.base);
        // Focus must read louder than the quiet ring.
        let quiet = math::contrast_ratio(theme_color(border.fg), theme_color(border.bg));
        let loud = math::contrast_ratio(theme_color(focus.fg), theme_color(focus.bg));
        assert!(
            loud > quiet,
            "{}: focus {loud:.2} <= border {quiet:.2}",
            palette.name
        );
        assert_eq!(pen.panel(false), border);
        assert_eq!(pen.panel(true), focus);
    }
}

#[test]
fn every_row_kind_has_its_pen_and_the_kinds_that_differ_read_differently() {
    use crate::agent::ToolStatus;
    use crate::turn::{RowKind, Voice};
    let pen = Pen::of(&catppuccin_mocha());
    assert_eq!(pen.row(RowKind::Prose(Voice::Agent)), &pen.stage);
    assert_eq!(pen.row(RowKind::Prose(Voice::Thought)), &pen.thought);
    assert_eq!(pen.row(RowKind::Prose(Voice::User)), &pen.user);
    assert_eq!(pen.row(RowKind::Heading), &pen.heading);
    assert_eq!(pen.row(RowKind::Code), &pen.code);
    assert_eq!(pen.row(RowKind::Fence), &pen.fence);
    assert_eq!(pen.row(RowKind::Bullet), &pen.bullet);
    assert_eq!(
        pen.row(RowKind::Tool(ToolStatus::Pending)),
        &pen.tool_pending
    );
    assert_eq!(
        pen.row(RowKind::Tool(ToolStatus::InProgress)),
        &pen.tool_running
    );
    assert_eq!(
        pen.row(RowKind::Tool(ToolStatus::Completed)),
        &pen.tool_done
    );
    assert_eq!(pen.row(RowKind::Tool(ToolStatus::Failed)), &pen.tool_failed);
    assert_eq!(pen.row(RowKind::Note), &pen.note);
    assert_eq!(pen.row(RowKind::Error), &pen.error);
    assert_eq!(pen.row(RowKind::DiffAdd), &pen.diff_add);
    assert_eq!(pen.row(RowKind::DiffRemove), &pen.diff_remove);
    assert_eq!(pen.row(RowKind::Conflict), &pen.diff_conflict);
    assert_eq!(pen.row(RowKind::EditButtons), &pen.code);
    assert_ne!(pen.diff_add.fg, pen.diff_remove.fg);
    assert_eq!(
        pen.diff_add.bg, pen.code.bg,
        "the diff sits on the code surface"
    );
    assert_ne!(pen.thought, pen.stage, "thinking reads dimmer than speech");
    assert_ne!(pen.code.bg, pen.stage.bg, "code sits on a raised surface");
    assert_eq!(
        pen.fence.bg, pen.code.bg,
        "the fence frames the same surface"
    );
    assert_ne!(
        pen.fence.fg, pen.code.fg,
        "the frame is quieter than the code"
    );
    assert!(pen.heading.attrs.contains(Attrs::BOLD));
    assert!(pen.error.attrs.contains(Attrs::BOLD));
    assert_ne!(pen.tool_failed.fg, pen.tool_done.fg);
    assert_ne!(pen.tool_running.fg, pen.tool_pending.fg);
    assert_eq!(pen.all().len(), 46);
}

#[test]
fn the_gutter_accents_speak_one_role_each_in_both_palettes() {
    // The gutter bar is structure like the panel ring — a block glyph,
    // not text — so its pens stay out of `Pen::all`'s AA sweep; this
    // test pins their palette roles and that the roles read apart.
    use crate::turn::Accent;
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(theme_color(pen.accent_user.fg), palette.mauve);
        assert_eq!(theme_color(pen.accent_agent.fg), palette.blue);
        assert_eq!(theme_color(pen.accent_thought.fg), palette.subtext0);
        assert_eq!(theme_color(pen.accent_tool.fg), palette.peach);
        let bars = [
            &pen.accent_user,
            &pen.accent_agent,
            &pen.accent_thought,
            &pen.accent_tool,
        ];
        for bar in bars {
            assert_eq!(theme_color(bar.bg), palette.base, "bars sit on the base");
            assert_ne!(bar.fg, pen.base.bg, "a bar never vanishes");
        }
        for (i, a) in bars.iter().enumerate() {
            for b in &bars[i + 1..] {
                assert_ne!(a.fg, b.fg, "{}: two roles share a colour", palette.name);
            }
        }
        assert_eq!(pen.accent(Accent::User), &pen.accent_user);
        assert_eq!(pen.accent(Accent::Agent), &pen.accent_agent);
        assert_eq!(pen.accent(Accent::Thought), &pen.accent_thought);
        assert_eq!(pen.accent(Accent::Tool), &pen.accent_tool);
        assert_eq!(pen.accent(Accent::Meta), &pen.stage, "meta rows go bare");
    }
}

/// A colour one step off `ground` — illegible against it by
/// construction, whichever palette the ground came from.
fn ground_ish(ground: ThemeColor) -> crate::render::Color {
    crate::render::Color::Rgb(
        ground.r.wrapping_add(1),
        ground.g.wrapping_add(1),
        ground.b.wrapping_add(1),
    )
}

#[test]
fn inline_spans_compose_over_the_row_pen() {
    use crate::render::Color;
    use crate::turn::{RowKind, SpanStyle, Voice};
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        let kind = RowKind::Prose(Voice::Agent);
        let bold = pen.span(kind, &SpanStyle::Bold);
        assert!(bold.attrs.contains(Attrs::BOLD));
        assert_eq!(bold.fg, pen.stage.fg, "bold keeps the row's colour");
        let italic = pen.span(kind, &SpanStyle::Italic);
        assert!(italic.attrs.contains(Attrs::ITALIC));
        assert_eq!(
            pen.span(kind, &SpanStyle::Code),
            pen.code,
            "an inline code run is the code pen"
        );
        let link = pen.span(kind, &SpanStyle::Link("https://x".to_owned()));
        assert!(link.attrs.contains(Attrs::UNDERLINE));
        assert_eq!(link.link.as_deref(), Some("https://x"));
        // A highlighter's colour wins wherever it is legible on the
        // code surface, and is nudged where it is not — the floor
        // belongs to the consumer, because a syntax theme is a colour
        // identity and makes no promise about our ground.
        let ground = match pen.code.bg {
            Color::Rgb(r, g, b) => ThemeColor::new(r, g, b),
            _ => panic!("the code pen has a truecolor ground"),
        };
        let legible = rgb(magistr_theme::math::ensure_contrast(
            ThemeColor::new(1, 2, 3),
            ground,
            Pen::MIN_CONTRAST,
        ));
        let kept = pen.span(RowKind::Code, &SpanStyle::Fg(legible));
        assert_eq!(kept.fg, legible, "a legible syntax colour is untouched");
        assert_eq!(kept.bg, pen.code.bg, "the code surface stays");
        let coloured = pen.span(RowKind::Code, &SpanStyle::Fg(ground_ish(ground)));
        let Color::Rgb(r, g, b) = coloured.fg else {
            panic!("the span keeps a truecolor foreground");
        };
        assert!(
            magistr_theme::math::contrast_ratio(ThemeColor::new(r, g, b), ground)
                >= Pen::MIN_CONTRAST,
            "an illegible syntax colour is floored, not painted"
        );
    }
}

#[test]
fn the_status_strip_has_its_pens() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(pen.phase(true), &pen.phase_current);
        assert_eq!(pen.phase(false), &pen.phase_other);
        assert!(pen.phase_current.attrs.contains(Attrs::BOLD));
        assert_ne!(
            pen.phase_current.fg, pen.phase_other.fg,
            "the current phase reads brighter"
        );
        assert_eq!(
            pen.phase_current.bg, pen.header.bg,
            "the strip sits on the chrome surface"
        );
        assert_eq!(pen.phase_other.bg, pen.header.bg);
        assert_eq!(pen.gate(Some(true)), &pen.gate_pass);
        assert_eq!(pen.gate(Some(false)), &pen.gate_fail);
        assert_eq!(pen.gate(None), &pen.gate_running);
        assert_ne!(
            pen.gate_pass.fg, pen.gate_fail.fg,
            "pass and fail read apart"
        );
        assert!(pen.gate_pass.attrs.contains(Attrs::BOLD));
        assert!(pen.gate_fail.attrs.contains(Attrs::BOLD));
        assert!(pen.gate_running.attrs.contains(Attrs::ITALIC));
        assert_eq!(pen.gate_fail.fg, pen.error.fg, "a failed gate is an error");
        assert_eq!(pen.gate_pass.bg, pen.header.bg, "the LEDs ride the bar");
        assert_eq!(pen.gate_fail.bg, pen.header.bg);
        assert_eq!(pen.gate_running.bg, pen.header.bg);
    }
}

#[test]
fn the_tab_bar_keeps_its_own_pens_on_the_panel_base() {
    // The tab bar sits inside the stage panel on the base colour; the
    // phase pens moved onto the header's chrome surface, so the tabs
    // carry their own pair.
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(pen.tab(true), &pen.tab_active);
        assert_eq!(pen.tab(false), &pen.tab_inactive);
        assert!(pen.tab_active.attrs.contains(Attrs::BOLD));
        assert_eq!(pen.tab_active.bg, pen.base.bg, "tabs stay on the base");
        assert_eq!(pen.tab_inactive.bg, pen.base.bg);
        assert_ne!(
            pen.tab_active.fg, pen.tab_inactive.fg,
            "the active tab reads brighter"
        );
    }
}

#[test]
fn the_keycap_chip_rises_off_the_footer_bar() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(theme_color(pen.chip_key.fg), palette.text);
        assert_eq!(theme_color(pen.chip_key.bg), palette.surface1);
        assert!(pen.chip_key.attrs.contains(Attrs::BOLD));
        assert_ne!(
            pen.chip_key.bg, pen.footer.bg,
            "the keycap rises off the bar"
        );
    }
}

#[test]
fn the_strip_the_buttons_the_prompt_and_the_cursor_have_their_pens() {
    use crate::agent::PermissionKind;
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(pen.permission.bg, pen.header.bg, "the strip is chrome");
        for kind in PermissionKind::ALL {
            assert_eq!(pen.button(kind).bg, pen.permission.bg);
            assert!(pen.button(kind).attrs.contains(Attrs::BOLD));
        }
        assert_eq!(pen.button(PermissionKind::AllowOnce), &pen.allow);
        assert_eq!(pen.button(PermissionKind::AllowAlways), &pen.always);
        assert_eq!(pen.button(PermissionKind::RejectOnce), &pen.deny);
        assert_eq!(pen.button(PermissionKind::RejectAlways), &pen.deny);
        assert_ne!(pen.allow.fg, pen.deny.fg, "allow and deny read apart");
        assert_ne!(pen.always.fg, pen.allow.fg, "always is its own colour");
        assert_eq!(pen.prompt, pen.base);
        assert!(pen.prompt_mark.attrs.contains(Attrs::BOLD));
        assert_eq!(pen.cursor.fg, pen.base.bg, "the cursor inverts the base");
        assert_eq!(pen.cursor.bg, pen.base.fg);
    }
}

#[test]
fn every_pen_reads_at_the_floor_in_every_palette_and_every_cvd_mode() {
    use magistr_theme::{CvdMode, all_builtin_palettes};
    let modes = [
        None,
        Some(CvdMode::Deuteranopia),
        Some(CvdMode::Protanopia),
        Some(CvdMode::Tritanopia),
    ];
    for palette in all_builtin_palettes() {
        for cvd in modes {
            let pen = Pen::with(&palette, cvd);
            for (name, style) in pen.all() {
                let ratio = math::contrast_ratio(theme_color(style.fg), theme_color(style.bg));
                assert!(
                    ratio >= Pen::MIN_CONTRAST,
                    "{} {cvd:?}: {name} contrast {ratio:.2}",
                    palette.name
                );
            }
        }
    }
}

#[test]
fn a_cvd_mode_moves_every_pen_that_means_failed_or_passed() {
    use magistr_theme::CvdMode;
    let palette = catppuccin_mocha();
    let plain = Pen::of(&palette);
    let deutan = Pen::with(&palette, Some(CvdMode::Deuteranopia));
    // Deuteranopia takes error off red and success off green.
    for (name, before, after) in [
        ("error", &plain.error, &deutan.error),
        ("tool_failed", &plain.tool_failed, &deutan.tool_failed),
        ("deny", &plain.deny, &deutan.deny),
        ("gate_fail", &plain.gate_fail, &deutan.gate_fail),
        ("diff_remove", &plain.diff_remove, &deutan.diff_remove),
        ("gate_pass", &plain.gate_pass, &deutan.gate_pass),
        ("allow", &plain.allow, &deutan.allow),
        ("diff_add", &plain.diff_add, &deutan.diff_add),
    ] {
        assert_ne!(before.fg, after.fg, "{name} did not follow the remapping");
    }
    // Body text is not a signal: it never moves.
    assert_eq!(plain.base, deutan.base);
    assert_eq!(plain.heading, deutan.heading);
}

#[test]
fn the_signal_pens_agree_with_each_other() {
    use magistr_theme::CvdMode;
    for cvd in [None, Some(CvdMode::Deuteranopia), Some(CvdMode::Tritanopia)] {
        for palette in [catppuccin_mocha(), catppuccin_latte()] {
            let pen = Pen::with(&palette, cvd);
            assert_eq!(pen.error.fg, pen.tool_failed.fg, "{}", palette.name);
            assert_eq!(pen.deny.fg, pen.gate_fail.fg, "{}", palette.name);
            assert_eq!(pen.allow.fg, pen.gate_pass.fg, "{}", palette.name);
            assert_ne!(pen.gate_pass.fg, pen.gate_fail.fg, "{}", palette.name);
            assert_ne!(pen.warn.fg, pen.gate_fail.fg, "{}", palette.name);
        }
    }
}

#[test]
fn the_pens_express_hierarchy_not_just_colour() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        let name = palette.name;
        // Headings and decisions are bold.
        for (label, style) in [
            ("heading", &pen.heading),
            ("header", &pen.header),
            ("user", &pen.user),
            ("error", &pen.error),
            ("warn", &pen.warn),
            ("allow", &pen.allow),
            ("deny", &pen.deny),
            ("always", &pen.always),
            ("permission", &pen.permission),
            ("phase_current", &pen.phase_current),
            ("tab_active", &pen.tab_active),
            ("chip_key", &pen.chip_key),
        ] {
            assert!(
                style.attrs.contains(Attrs::BOLD),
                "{name}: {label} is not bold"
            );
        }
        // Metadata is quieter than body text: secondary ink, and never
        // bold.
        for (label, style) in [
            ("note", &pen.note),
            ("thought", &pen.thought),
            ("footer", &pen.footer),
            ("tool_pending", &pen.tool_pending),
            ("tab_inactive", &pen.tab_inactive),
            ("phase_other", &pen.phase_other),
            ("gate_running", &pen.gate_running),
            ("fence", &pen.fence),
        ] {
            assert!(
                !style.attrs.contains(Attrs::BOLD),
                "{name}: {label} is bold"
            );
            let quiet = math::contrast_ratio(theme_color(style.fg), theme_color(style.bg));
            let body = math::contrast_ratio(theme_color(pen.base.fg), theme_color(pen.base.bg));
            assert!(
                quiet < body,
                "{name}: {label} is not quieter than body text"
            );
        }
        // Interactivity is accented, never plain body text.
        for (label, style) in [
            ("link", &pen.link),
            ("tab_active", &pen.tab_active),
            ("allow", &pen.allow),
            ("deny", &pen.deny),
            ("always", &pen.always),
            ("prompt_mark", &pen.prompt_mark),
        ] {
            assert_ne!(style.fg, pen.base.fg, "{name}: {label} reads as body text");
        }
        assert!(pen.link.attrs.contains(Attrs::UNDERLINE), "{name}: link");
        assert!(pen.note.attrs.contains(Attrs::ITALIC), "{name}: note");
    }
}

/// The motion pens: a shimmer or a pulse at the start or the
/// end of its period is exactly the pen it moves, so a settled frame is
/// the same frame; at the crest it visibly lifts; and a pulsed text pen
/// still clears the floor in both palettes.
#[test]
fn a_shimmer_and_a_pulse_leave_the_settled_frame_alone_and_lift_at_the_crest() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(pen.shimmer(&pen.accent_agent, 0.0), pen.accent_agent);
        assert_eq!(pen.shimmer(&pen.accent_agent, 1.0), pen.accent_agent);
        assert_ne!(pen.shimmer(&pen.accent_agent, 0.5), pen.accent_agent);
        assert_eq!(pen.pulse(&pen.tab_active, 0.0), pen.tab_active);
        assert_eq!(pen.pulse(&pen.tab_active, 1.0), pen.tab_active);
        let crest = pen.pulse(&pen.tab_active, 0.5);
        assert_ne!(crest, pen.tab_active);
        assert_eq!(crest.attrs, pen.tab_active.attrs, "only the colour moves");
        assert_eq!(crest.bg, pen.tab_active.bg);
        for t in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let pulsed = pen.pulse(&pen.tab_inactive, t);
            let ratio = math::contrast_ratio(theme_color(pulsed.fg), theme_color(pulsed.bg));
            assert!(
                ratio >= Pen::MIN_CONTRAST,
                "{} pulse at {t}: {ratio:.2}",
                palette.name
            );
        }
        // A pen without truecolor is left alone.
        let plain = Style::new();
        assert_eq!(pen.pulse(&plain, 0.5), plain);
    }
}

/// The arrival pens: a fade lands exactly on the pen it fades
/// in, starts visibly dimmer than it, never dips below the floor on the
/// way, and reaches the pen monotonically; a pop is the pulse with BOLD
/// laid on only while the wave is up, so both ends are the pen itself.
#[test]
fn a_fade_lands_on_its_pen_and_a_pop_is_bold_only_while_it_is_up() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let name = palette.name;
        let pen = Pen::of(&palette);
        // Lands on the pen, byte for byte — a settled row is today's row.
        assert_eq!(pen.fade(&pen.stage, 1.0), pen.stage, "{name}");
        assert_eq!(pen.fade(&pen.stage, 1.5), pen.stage, "{name}: clamped");
        // Starts at the dim pen's colour, keeping the row's own ground
        // and attributes.
        let start = pen.fade(&pen.heading, 0.0);
        assert_eq!(start.fg, pen.note.fg, "{name}: fades in from the dim pen");
        assert_eq!(start.bg, pen.heading.bg, "{name}");
        assert_eq!(
            start.attrs, pen.heading.attrs,
            "{name}: only the colour moves"
        );
        // Every step of the way clears the text floor.
        for t in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0] {
            let faded = pen.fade(&pen.heading, t);
            let ratio = math::contrast_ratio(theme_color(faded.fg), theme_color(faded.bg));
            assert!(ratio >= Pen::MIN_CONTRAST, "{name} fade at {t}: {ratio:.2}");
        }
        // A pen without truecolor is left alone.
        let plain = Style::new();
        assert_eq!(pen.fade(&plain, 0.25), plain, "{name}");

        // The pop: exactly the pen at both ends, raised AND bold in the
        // middle, and a failing gate's pop stays on the error pen.
        assert_eq!(pen.pop(&pen.gate_pass, 0.0), pen.gate_pass, "{name}");
        assert_eq!(pen.pop(&pen.gate_pass, 1.0), pen.gate_pass, "{name}");
        let crest = pen.pop(&pen.gate_pass, 0.5);
        assert_ne!(crest.fg, pen.gate_pass.fg, "{name}: raised at the crest");
        assert_eq!(crest.bg, pen.gate_pass.bg, "{name}");
        // The weight only shows on a pen that had none — the gate LEDs
        // are bold already, so on those a pop is colour alone.
        assert!(pen.gate_pass.attrs.contains(Attrs::BOLD), "{name}");
        let plain_crest = pen.pop(&pen.stage, 0.5);
        assert!(
            plain_crest.attrs.contains(Attrs::BOLD),
            "{name}: bold at the crest"
        );
        // A quarter in the wave is already half up: bold is on.
        assert!(
            pen.pop(&pen.stage, 0.25).attrs.contains(Attrs::BOLD),
            "{name}: bold from half the crest"
        );
        // An eighth in it is not.
        assert!(
            !pen.pop(&pen.stage, 0.05).attrs.contains(Attrs::BOLD),
            "{name}: not bold at the foot"
        );
        assert_eq!(pen.pop(&pen.stage, 0.0), pen.stage, "{name}");
        assert_eq!(pen.pop(&pen.stage, 1.0), pen.stage, "{name}");
        assert_eq!(
            pen.pop(&pen.gate_fail, 0.5).bg,
            pen.gate_fail.bg,
            "{name}: a failing gate pops on its own error pen"
        );
    }
}

// ── the three hover treatments ────────────────────────────

/// A raised pen keeps its glyph's legibility on the new ground: the
/// second surface under it in both palettes, the foreground re-floored
/// against that ground, and the attributes it already carried untouched.
#[test]
fn the_raised_pen_swaps_the_ground_and_holds_the_floor() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        for (name, style) in pen.all() {
            let raised = pen.raise(style);
            assert_eq!(
                raised.bg, pen.hover_ground,
                "{name}: raised onto the hover ground"
            );
            assert_eq!(raised.attrs, style.attrs, "{name}: weight is untouched");
            let ratio = math::contrast_ratio(theme_color(raised.fg), theme_color(raised.bg));
            assert!(
                ratio >= Pen::MIN_CONTRAST,
                "{name}: raised to {ratio:.2} on the hover ground"
            );
        }
        assert_eq!(
            pen.hover_ground,
            Color::Rgb(palette.surface1.r, palette.surface1.g, palette.surface1.b),
            "the hover ground is the palette's second surface"
        );
    }
}

/// A lifted pen is the pen's own colour moved toward the bright extreme
/// of its ground and bolded — visibly different from the pen it came
/// from, still over the floor, still on the same ground, so a lift never
/// moves a cell.
#[test]
fn the_lifted_pen_differs_from_its_own_and_stays_legible() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        for (name, style) in pen.all() {
            let lifted = pen.lift(style);
            assert_eq!(lifted.bg, style.bg, "{name}: the ground is untouched");
            assert!(lifted.attrs.contains(Attrs::BOLD), "{name}: lifted is bold");
            assert_ne!(lifted.fg, style.fg, "{name}: the lift is visible");
            let ratio = math::contrast_ratio(theme_color(lifted.fg), theme_color(lifted.bg));
            assert!(ratio >= Pen::MIN_CONTRAST, "{name}: lifted to {ratio:.2}");
        }
    }
}

/// The link treatment is the lift with a rule under it — what tells a
/// hovered revision link from a hovered button.
#[test]
fn the_underlined_pen_is_the_lift_plus_a_rule() {
    let pen = Pen::of(&catppuccin_mocha());
    let lifted = pen.lift(&pen.stage);
    let underlined = pen.underline(&pen.stage);
    assert_eq!(underlined.fg, lifted.fg);
    assert_eq!(underlined.bg, lifted.bg);
    assert!(underlined.attrs.contains(Attrs::UNDERLINE));
    assert!(underlined.attrs.contains(Attrs::BOLD));
    assert!(!lifted.attrs.contains(Attrs::UNDERLINE));
}

/// A held lift is gentler than a pulse's crest: the same direction, less
/// far, so a signal pen keeps its hue while the pointer rests on it.
#[test]
fn a_held_lift_goes_less_far_than_a_pulse_crest() {
    let pen = Pen::of(&catppuccin_mocha());
    let allow = &pen.allow;
    let lifted = theme_color(pen.lift(allow).fg);
    let crest = theme_color(pen.pulse(allow, 0.5).fg);
    let plain = theme_color(allow.fg);
    let distance = |a: ThemeColor, b: ThemeColor| {
        f64::from(a.r).mul_add(0.0, 0.0)
            + (f64::from(a.r) - f64::from(b.r)).abs()
            + (f64::from(a.g) - f64::from(b.g)).abs()
            + (f64::from(a.b) - f64::from(b.b)).abs()
    };
    assert!(distance(lifted, plain) > 0.0, "the lift is visible");
    assert!(
        distance(lifted, plain) < distance(crest, plain),
        "and gentler than the flash"
    );
}

#[test]
fn the_four_rules_carry_the_shapes_their_meanings_ask_for() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        assert_eq!(
            pen.gate_fail.attrs.underline(),
            Some(Underline::Curly),
            "a failed gate is marked the way an editor marks a diagnostic"
        );
        assert_eq!(
            pen.diff_conflict.attrs.underline(),
            Some(Underline::Curly),
            "so is the one diff line the human has to resolve"
        );
        assert_eq!(
            pen.link.attrs.underline(),
            Some(Underline::Dotted),
            "a link is dotted"
        );
        assert_eq!(
            pen.underline(&pen.link).attrs.underline(),
            Some(Underline::Dotted),
            "a link under the pointer is still a link"
        );
        assert_eq!(
            pen.tab_active.attrs.underline(),
            Some(Underline::Single),
            "the active tab wears a plain rule"
        );
        assert_eq!(
            pen.tab_inactive.attrs.underline(),
            None,
            "and the others wear none — the rule is what marks it active"
        );
        assert_eq!(pen.row(RowKind::Conflict), &pen.diff_conflict);
    }
}

#[test]
fn a_rule_that_means_something_carries_its_own_colour() {
    let palette = catppuccin_mocha();
    let pen = Pen::of(&palette);
    let error = theme_color(pen.error.fg);
    for (name, style) in [
        ("gate_fail", &pen.gate_fail),
        ("diff_conflict", &pen.diff_conflict),
    ] {
        let rule = theme_color(style.underline_color);
        assert!(
            math::contrast_ratio(rule, theme_color(style.bg)) > 1.0,
            "{name}'s rule is a colour, not the ground"
        );
        assert_ne!(
            style.underline_color,
            Color::Default,
            "{name} names its rule colour rather than borrowing the text's"
        );
    }
    assert_eq!(
        theme_color(pen.tab_active.underline_color),
        ThemeColor::new(palette.mauve.r, palette.mauve.g, palette.mauve.b),
        "the active tab's rule is the raw accent — structural, like the gutter bars"
    );
    // The error pens' text is floored for contrast; the rule beside it
    // is the role's own colour, so the two need not be equal.
    assert!(error.r > 0 || error.g > 0 || error.b > 0);
}

#[test]
fn every_pen_that_carries_a_rule_still_carries_the_underline_bit() {
    let pen = Pen::of(&catppuccin_mocha());
    for (name, style) in [
        ("gate_fail", &pen.gate_fail),
        ("diff_conflict", &pen.diff_conflict),
        ("link", &pen.link),
        ("tab_active", &pen.tab_active),
    ] {
        assert!(
            style.attrs.contains(Attrs::UNDERLINE),
            "{name} falls back to the plain rule on a terminal that predates 4:x"
        );
    }
}

/// The terminal pane's pen: hue is the program's — an
/// ANSI index the palette's own colour, truecolor itself — and
/// legibility the theme's, every foreground floored against its ground.
#[test]
fn the_term_pen_keeps_the_programs_hue_and_floors_it_on_its_ground() {
    for palette in [catppuccin_mocha(), catppuccin_latte()] {
        let pen = Pen::of(&palette);
        let ansi = magistr_theme::ansi16(&palette);
        // Default colours are the code pen's: the pane is a well of
        // output on the code ground.
        let plain = pen.term(Color::Default, Color::Default, Attrs::NONE);
        assert_eq!(plain.bg, pen.code.bg, "{}", palette.name);
        assert_eq!(plain.fg, pen.code.fg, "{}", palette.name);
        // ANSI red is the palette's red — the program's hue, kept.
        let red = pen.term(Color::Indexed(1), Color::Default, Attrs::BOLD);
        let ground = theme_color(pen.code.bg);
        let expect = math::ensure_contrast(ansi[1], ground, Pen::MIN_CONTRAST);
        assert_eq!(theme_color(red.fg), expect, "{}", palette.name);
        assert_eq!(red.attrs, Attrs::BOLD);
        // Bright black — a program's dim grey — clears the floor.
        let dim = pen.term(Color::Indexed(8), Color::Default, Attrs::DIM);
        assert!(
            math::contrast_ratio(theme_color(dim.fg), ground) >= Pen::MIN_CONTRAST,
            "{}: dim grey reads",
            palette.name
        );
        // Truecolor is itself unless it fails the floor; a cube index
        // is xterm's; a grey ramp entry likewise.
        let peach = pen.term(Color::Rgb(250, 179, 135), Color::Default, Attrs::NONE);
        let floored =
            math::ensure_contrast(ThemeColor::new(250, 179, 135), ground, Pen::MIN_CONTRAST);
        assert_eq!(theme_color(peach.fg), floored, "{}", palette.name);
        let cube = pen.term(Color::Indexed(196), Color::Default, Attrs::NONE);
        let floored = math::ensure_contrast(ThemeColor::new(255, 0, 0), ground, Pen::MIN_CONTRAST);
        assert_eq!(
            theme_color(cube.fg),
            floored,
            "{}: 196 is pure red",
            palette.name
        );
        let grey = pen.term(Color::Default, Color::Indexed(244), Attrs::NONE);
        assert_eq!(
            grey.bg,
            Color::Rgb(128, 128, 128),
            "{}: grey 244",
            palette.name
        );
        // Reverse swaps the pair and is spent doing it.
        let reversed = pen.term(
            Color::Indexed(1),
            Color::Default,
            Attrs::REVERSE | Attrs::BOLD,
        );
        assert_eq!(theme_color(reversed.bg), ansi[1], "{}", palette.name);
        assert_eq!(reversed.attrs, Attrs::BOLD);
        assert!(
            math::contrast_ratio(theme_color(reversed.fg), ansi[1]) >= Pen::MIN_CONTRAST,
            "{}: the ink on a red ground reads",
            palette.name
        );
    }
}
