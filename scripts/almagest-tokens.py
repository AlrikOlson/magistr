#!/usr/bin/env python3
"""Generate JSON and Rust design tokens from the reference HTML stylesheet.

Run: python3 scripts/almagest-tokens.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DESIGN = ROOT / "crates" / "magistr-gui2" / "design"
VOLUME = DESIGN / "almagest-seventh-edition.html"



def rules(css: str) -> dict[str, str]:
    """Every selector -> declaration body in the volume's one stylesheet."""
    out: dict[str, str] = {}
    for m in re.finditer(r"([^{}]+)\{([^{}]*)\}", css):
        sel = re.sub(r"/\*.*?\*/", "", m.group(1), flags=re.S).strip()
        out[sel] = out.get(sel, "") + ";" + m.group(2)
    return out


def prop(body: str, name: str) -> str | None:
    m = re.search(rf"(?:^|;)\s*{name}\s*:\s*([^;]+)", body)
    return m.group(1).strip() if m else None


def px(value: str | None) -> float | None:
    if value is None:
        return None
    m = re.match(r"([\d.]+)px$", value)
    return float(m.group(1)) if m else None


def shorthand(value: str | None, index: int) -> float | None:
    """One side of a CSS shorthand (`margin:18px 0 8px`), in px."""
    if value is None:
        return None
    parts = value.split()
    if index >= len(parts):
        return None
    m = re.match(r"([\d.]+)px$", parts[index])
    return float(m.group(1)) if m else 0.0 if parts[index] == "0" else None


def rule_px(css_rules: dict[str, str], selector: str, name: str) -> float:
    """One px declaration off a class rule, or an error naming it."""
    body = css_rules.get(selector)
    if body is None:
        raise SystemExit(f"selector {selector!r} is gone from the volume")
    value = px(prop(body, name))
    if value is None:
        raise SystemExit(f"{selector} no longer declares {name}")
    return value


def em(value: str | None) -> float:
    if value is None:
        return 0.0
    m = re.match(r"\.?[\d.]*em$", value) and re.match(r"([\d.]+|\.[\d]+)em$", value)
    return float(m.group(1)) if m else 0.0


def voice(
    *,
    family: str,
    size: float | None,
    weight: int = 400,
    tracking: float = 0.0,
    line_height: float = 0.0,
    upper: bool = False,
) -> dict:
    if size is None:
        raise SystemExit("a voice the volume no longer declares — re-derive by hand")
    return {
        "family": family,
        "px": size,
        "weight": weight,
        "tracking_em": tracking,
        "line_height": line_height,
        "transform": "uppercase" if upper else "none",
    }


def class_voice(css_rules: dict[str, str], selector: str, family: str) -> dict:
    body = css_rules.get(selector)
    if body is None:
        raise SystemExit(f"selector {selector!r} is gone from the volume")
    fam = prop(body, "font-family") or ""
    declared = (
        "mono" if "--mono" in fam else "serif" if "--serif" in fam else "sans" if "--sans" in fam else family
    )
    weight = prop(body, "font-weight")
    lh = prop(body, "line-height")
    return voice(
        family=declared if fam else family,
        size=px(prop(body, "font-size")),
        weight=int(weight) if weight else 400,
        tracking=em(prop(body, "letter-spacing")),
        line_height=float(lh) if lh else 0.0,
        upper=prop(body, "text-transform") == "uppercase",
    )


# The custom properties both `.app7` blocks declare, in the order the
# generated palette file lists them. Every name here is required in
# BOTH media — a var the volume drops fails the run by name.
APP7_VARS = [
    "base",
    "deep",
    "mantle",
    "s0",
    "s1",
    "ov0",
    "tx",
    "sb0",
    "sb1",
    "blue",
    "mauve",
    "green",
    "yellow",
    "red",
    "peach",
    "teal",
    "glow-m",
    "glow-g",
    "glow-p",
    "edge",
]


def hex_rgb(value: str) -> list[int]:
    m = re.match(r"#([0-9a-fA-F]{6})$", value.strip())
    if m is None:
        raise SystemExit(f"expected a 6-digit hex color, got {value!r}")
    h = m.group(1)
    return [int(h[i : i + 2], 16) for i in (0, 2, 4)]


def app7_palette(css_rules: dict[str, str], selector: str) -> dict:
    """One `.app7` block's custom properties: hex vars + the hairc mix."""
    body = css_rules.get(selector)
    if body is None:
        raise SystemExit(f"selector {selector!r} is gone from the volume")
    out: dict = {}
    for var in APP7_VARS:
        value = prop(body, f"--{var}")
        if value is None:
            raise SystemExit(f"{selector} no longer declares --{var}")
        out[var.replace("-", "_")] = hex_rgb(value)
    hairc = prop(body, "--hairc")
    m = hairc and re.match(
        r"color-mix\(in oklab,\s*(#[0-9a-fA-F]{6})\s+([\d.]+)%,\s*transparent\)", hairc
    )
    if not m:
        raise SystemExit(f"{selector}'s --hairc is no longer a color-mix with transparent")
    out["hairc"] = {"rgb": hex_rgb(m.group(1)), "alpha": float(m.group(2)) / 100.0}
    return out


def inline(markup: str, pattern: str, what: str) -> str:
    m = re.search(pattern, markup, re.S)
    if m is None:
        raise SystemExit(f"the volume no longer carries {what}")
    return m.group(1)


def main() -> None:
    html = VOLUME.read_text(encoding="utf-8")
    css = "\n".join(re.findall(r"<style[^>]*>(.*?)</style>", html, re.S))
    r = rules(css)

    # The Session view (05) markup, for the voices set inline.
    session = html[html.rfind("<figure", 0, html.find("Steer")) : html.find("Steer") + 4000]
    # The Moment view (06), whose overlay title is the heading exemplar.
    moment = html[html.find("06 &middot;") :][:6000]
    # The review view (07): its own column, card and note spreads.
    review = html[html.find("07 &middot;") :][:6000]

    tokens = {
        # ── class-declared voices, read off the stylesheet ────────────
        "cap": class_voice(r, ".cap7", "mono"),
        "crumb": class_voice(r, ".crumb", "mono"),
        "chip": class_voice(r, ".chip7", "mono"),
        "diffline": class_voice(r, ".diffline", "mono"),
        "badge": class_voice(r, ".mode7", "mono"),
        "value": class_voice(r, ".rr .rv", "sans") | {"family": "mono"},
        "meter_value": class_voice(r, ".meter .mv", "mono"),
        "ledger_name": class_voice(r, ".srow .sn", "sans"),
        "ledger_meta": class_voice(r, ".srow .sm", "mono"),
        "hint": class_voice(r, ".khint7", "sans"),
        "receipt_key": class_voice(r, ".rr .rk", "mono"),
        # ── the control atoms' own voices ───────────────────
        "button": class_voice(r, ".btn7", "sans"),
        "kbd": class_voice(r, ".kbd7", "mono"),
        "theme_label": class_voice(r, ".tcard .tl2", "sans"),
        "slot": voice(
            family="sans",
            size=px(
                inline(
                    html,
                    r"border:1\.5px dashed var\(--s1\);border-radius:10px;"
                    r"padding:12px 22px;font-size:(12px)",
                    "the dashed slot's copy at 12px",
                )
            ),
        ),
        # ── voices the views set inline, anchored in the markup ───────
        "prose": voice(
            family="sans",
            size=px(
                inline(
                    session,
                    r'<p style="[^"]*font-size:(13px);line-height:1\.65"',
                    "the turn's prose at 13px/1.65",
                )
            ),
            line_height=1.65,
        ),
        "steer": voice(
            family="sans",
            size=px(
                inline(
                    session,
                    r'flex:1;color:var\(--sb0\);font-size:([\d.]+px)">Steer',
                    "the Steer placeholder",
                )
            ),
        ),
        "phase": voice(
            family="mono",
            size=px(
                inline(
                    session,
                    r'class="hair-t mono">\s*<span style="font-size:([\d.]+px)',
                    "the phases bar",
                )
            ),
        ),
        "heading": voice(
            family="sans",
            size=px(
                inline(
                    moment,
                    r'<div style="font-size:([\d.]+px);font-weight:600;letter-spacing:-\.01em',
                    "the moment title at 15/600",
                )
            ),
            weight=600,
        ),
        "stat": voice(
            family="mono",
            size=px(
                inline(
                    html,
                    r'<span class="mono num" style="font-size:([\d.]+px);'
                    r'color:var\(--sb1\)"><span class="cnt">',
                    "the counter's stat voice",
                )
            ),
        ),
        # Display-face sizing for the chapter title (the serif appears
        # only at >=20px on report titles and the project cover): the
        # chapter's title in view 09, set inline in the volume's --serif.
        "chapter_title": voice(
            family="serif",
            size=px(
                inline(
                    html,
                    r'<div style="font-family:var\(--serif\);font-size:([\d.]+px);'
                    r'letter-spacing:-\.01em;margin-bottom:12px">',
                    "the chapter title in the serif display face",
                )
            ),
            tracking=-0.01,
        ),
    }

    # The leading is the APPLICATION WINDOW's, not the page's. `body`
    # sets 1.6 for the volume's prose; `.app7` — every one of the
    # nineteen views — sets its own 1.5, and every voice inside a view
    # that declares no leading of its own inherits that one. Reading
    # `body` here put a pixel of air under every unleaded row and
    # compounded it down each column.
    app = r.get(".app7")
    if app is None or prop(app, "line-height") is None:
        raise SystemExit("the volume's .app7 no longer declares a line-height")

    geometry = {
        "rail_width": px(inline(session, r'style="width:(248px);flex:none', "the 248px rail")),
        "card_radius": px(inline(session, r'border-radius:(11px)"', "the card's radius")),
        "turn_max_width": px(inline(session, r'max-width:(600px)', "the turn's measure")),
        # The window's own reading rhythm — the leading every voice that
        # declares none of its own is set on.
        "leading": float(prop(app, "line-height")),
        # The window's hairlines: `.hair-b`/`.hair-t`/`.hair-l` are all
        # `1px solid`, and a header's or a footer's own height includes
        # the rule under it.
        "hairline": 1.0,
    }

    # ── the rail's own spreads, as the volume's markup declares them ──
    rail_pad = inline(
        session,
        r'style="width:248px;flex:none;padding:([\d.]+px [\d.]+px)"',
        "the rail's padding",
    )
    charter_cap = inline(
        session,
        r'<div class="cap7" style="margin:([\d.]+px 0 [\d.]+px)"',
        "the charter cap's margins",
    )
    spacing = {
        "rail_pad_top": shorthand(rail_pad, 0),
        "rail_pad_side": shorthand(rail_pad, 1),
        # Two caps, two gaps: the turn column's caption and the rail's
        # first section cap are both `.cap7` with a margin-bottom and
        # they are not the same number, so each is anchored on what
        # follows it rather than on the class alone.
        "stage_cap_gap": px(
            inline(
                session,
                r'<div class="cap7" style="margin-bottom:([\d.]+px)">Agent',
                "the turn caption's gap",
            )
        ),
        "rail_cap_gap": px(
            inline(
                session,
                r'<div class="cap7" style="margin-bottom:([\d.]+px)">'
                r'<svg class="ic"><use href="#ic-checkpoint"',
                "the ledger cap's gap",
            )
        ),
        "rail_section_gap": shorthand(charter_cap, 0),
        "rail_section_cap_gap": shorthand(charter_cap, 2),
        "rail_ledger_pitch": px(
            inline(session, r'<div style="margin-bottom:([\d.]+px);position:relative"', "a ledger row's pitch")
        ),
        "rail_ledger_indent": px(
            inline(session, r'border-left:2px solid var\(--s0\);padding-left:([\d.]+px)', "the spine's indent")
        ),
        "rail_row_pad": shorthand(prop(r[".rr"], "padding"), 0),
        "meter_label_gap": rule_px(r, ".meter .ml", "margin-bottom"),
        "meter_note_gap": rule_px(r, ".meter .mf", "margin-top"),
        "meter_track_height": rule_px(r, ".meter .mb", "height"),
        "meter_tick_width": rule_px(r, ".meter .mb b", "width"),
        # ── the review view's own spreads (07) ───────────────────────
        "review_files_pad_top": shorthand(
            inline(
                review,
                r'style="width:230px;flex:none;padding:([\d.]+px [\d.]+)"',
                "the reviewed-file column's padding",
            ),
            0,
        ),
        "review_files_pad_side": px(
            inline(
                review,
                r'<div class="cap7" style="padding:0 ([\d.]+px) [\d.]+px">',
                "the scope cap's side padding",
            )
        ),
        "review_scope_cap_gap": px(
            inline(
                review,
                r'<div class="cap7" style="padding:0 [\d.]+px ([\d.]+px)">',
                "the scope cap's gap",
            )
        ),
        # A file row's pitch is its own line height at its own size —
        # the volume sets the list as one block, so the rhythm is the
        # leading rather than a margin.
        "review_file_pitch": round(
            float(
                inline(
                    review,
                    r'<div class="mono" style="font-size:11px;line-height:([\d.]+)">',
                    "the file list's leading",
                )
            )
            * 11.0,
            2,
        ),
        "review_agent_cap_gap": shorthand(
            inline(
                review,
                r'<div class="cap7" style="padding:([\d.]+px [\d.]+px [\d.]+px)">the agent',
                "the agent cap's padding",
            ),
            0,
        ),
        "review_agent_pitch": round(
            float(
                inline(
                    review,
                    r'padding:0 [\d.]+px;font-size:10px;color:var\(--ov0\);line-height:([\d.]+)"',
                    "the agent block's leading",
                )
            )
            * 10.0,
            2,
        ),
        "review_pane_pad_top": shorthand(
            inline(
                review,
                r'style="flex:1;min-width:0;padding:([\d.]+px [\d.]+px);overflow:hidden"',
                "the diff pane's padding",
            ),
            0,
        ),
        "review_pane_pad_side": shorthand(
            inline(
                review,
                r'style="flex:1;min-width:0;padding:([\d.]+px [\d.]+px);overflow:hidden"',
                "the diff pane's padding",
            ),
            1,
        ),
        "review_card_pad_top": shorthand(
            inline(
                review,
                r'<div class="mono hair-b" style="padding:([\d.]+px [\d.]+px);font-size:10.5px',
                "the diff card head's padding",
            ),
            0,
        ),
        "review_card_pad_side": shorthand(
            inline(
                review,
                r'<div class="mono hair-b" style="padding:([\d.]+px [\d.]+px);font-size:10.5px',
                "the diff card head's padding",
            ),
            1,
        ),
        "review_note_gap": px(
            inline(
                review,
                r'<div style="margin-top:([\d.]+px);max-width:640px;font-size:12.5px',
                "the agent note's gap",
            )
        ),
    }

    # ── the session header, as the volume's markup declares it ──
    #
    # `padding:15px 20px 12px 92px` on a `hair-b` row. The left inset is
    # the room the window's own lights take; the top and bottom decide
    # where everything under the header begins, and the band between
    # them is the tallest thing in the row — the clock chip, whose own
    # box is its padding, its border and the chip voice's line box.
    header_pad = inline(
        session,
        r'style="display:flex;align-items:center;gap:\d+px;'
        r'padding:([\d.]+px [\d.]+px [\d.]+px [\d.]+px)" class="hair-b"',
        "the session header's padding",
    )
    # The header's three gaps, and they are three different numbers:
    # the row's own `gap` between the crumb group, the spark and the
    # clock chip; `.crumb`'s gap between one crumb and the separator
    # after it; and `.crumb .c`'s gap between a crumb's own mark and
    # its label. Spreading all three on one cell-derived constant is
    # what put the altitude path 57px past where the volume ends it.
    header_gap = px(
        inline(
            session,
            r'style="display:flex;align-items:center;gap:([\d.]+px);'
            r'padding:[\d.]+px [\d.]+px [\d.]+px [\d.]+px" class="hair-b"',
            "the session header's gap",
        )
    )
    chip_pad = prop(r.get(".chip7", ""), "padding")
    chip = tokens["chip"]
    stage_pad = inline(
        session,
        r'<div style="flex:1;overflow:hidden;padding:([\d.]+px [\d.]+px)">',
        "the turn column's padding",
    )
    composer_pad = inline(
        session,
        r'<div style="padding:(0 [\d.]+px [\d.]+px)">\s*<div style="display:flex;'
        r'align-items:center;gap:[\d.]+px;border:1px solid var\(--hairc\);'
        r"border-radius:999px",
        "the composer's padding",
    )
    pill_pad = inline(
        session,
        r"border-radius:999px;padding:([\d.]+px [\d.]+px);"
        r"background:color-mix\(in oklab, var\(--mantle\)",
        "the composer pill's padding",
    )
    footer_pad = inline(
        session,
        r'<div style="display:flex;gap:[\d.]+px;align-items:center;'
        r'padding:([\d.]+px [\d.]+px)" class="hair-t mono">',
        "the bottom bar's padding",
    )
    footer_gap = inline(
        session,
        r'<div style="display:flex;gap:([\d.]+px);align-items:center;'
        r'padding:[\d.]+px [\d.]+px" class="hair-t mono">',
        "the bottom bar's gap",
    )
    spacing.update(
        {
            "stage_header_gap": header_gap,
            "crumb_gap": rule_px(r, ".crumb", "gap"),
            "crumb_mark_gap": rule_px(r, ".crumb .c", "gap"),
            "stage_header_pad_top": shorthand(header_pad, 0),
            "stage_header_pad_bottom": shorthand(header_pad, 2),
            "stage_header_pad_left": shorthand(header_pad, 3),
            "chip_pad_y": shorthand(chip_pad, 0),
            "chip_pad_x": shorthand(chip_pad, 1),
            # Derived, because a header's height is not declared
            # anywhere: it is its own padding around the tallest thing
            # in the row — the clock chip, whose box is its padding, its
            # border either side and the chip voice's line box — plus
            # the hairline under it. One expression rather than a
            # measured constant, so a volume that re-pads its header or
            # re-sizes its chip moves the cockpit's header with it.
            "stage_header_height": shorthand(header_pad, 0)
            + 2 * shorthand(chip_pad, 0)
            + 2 * geometry["hairline"]
            + chip["px"] * geometry["leading"]
            + shorthand(header_pad, 2)
            + geometry["hairline"],
            "stage_pad_top": shorthand(stage_pad, 0),
            "stage_pad_side": shorthand(stage_pad, 1),
            # The composer's own wrapper carries the column's side
            # padding again and its bottom air; the pill inside it is a
            # stadium whose height is its padding, its border and the
            # steer voice's line box.
            "composer_pad_bottom": shorthand(composer_pad, 2),
            "composer_pill_height": 2 * shorthand(pill_pad, 0)
            + 2 * geometry["hairline"]
            + tokens["steer"]["px"] * geometry["leading"],
            "composer_pill_pad_side": shorthand(pill_pad, 1),
            "composer_pill_gap": px(
                inline(
                    session,
                    r"align-items:center;gap:([\d.]+px);border:1px solid "
                    r"var\(--hairc\);border-radius:999px",
                    "the composer pill's gap",
                )
            ),
            # The bottom bar: a hairline, its padding and the phase
            # voice's line box.
            "footer_height": geometry["hairline"]
            + 2 * shorthand(footer_pad, 0)
            + tokens["phase"]["px"] * geometry["leading"],
            "footer_pad_side": shorthand(footer_pad, 1),
            "footer_gap": px(footer_gap),
            # The rhythm of the feed: every block the turn column stacks
            # — a paragraph, a card — carries the same margin under it.
            "stage_block_gap": px(
                inline(
                    session,
                    r'<p style="margin:0 0 ([\d.]+px);font-size:13px;line-height:1\.65">',
                    "the turn paragraph's gap",
                )
            ),
        }
    )

    missing = [name for name, value in spacing.items() if value is None]
    if missing:
        raise SystemExit(f"the volume no longer declares: {', '.join(missing)}")

    # ── the window's two palettes, the volume's own hex ──────────────
    # `.app7` is the night medium and `.app7.latte` the day one; the
    # component layer paints in these, never in a Catppuccin
    # slot re-typed by hand. `--hairc` is the one non-hex var: a
    # color-mix with transparent, which is its color at that alpha.
    palette = {
        "night": app7_palette(r, ".app7"),
        "day": app7_palette(r, ".app7.latte"),
    }

    table = {
        "source": VOLUME.name,
        "generated_by": "scripts/almagest-tokens.py",
        "voices": tokens,
        "geometry": geometry,
        "spacing": spacing,
        "palette": palette,
    }
    (DESIGN / "almagest7.tokens.json").write_text(
        json.dumps(table, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )

    lines = [
        "// GENERATED by scripts/almagest-tokens.py from",
        f"// design/{VOLUME.name} — the canonical CSS token oracle.",
        "// Do not edit; re-run the script when the reference stylesheet changes.",
        "",
    ]
    for name, v in sorted(tokens.items()):
        lines.append(
            f'pub const {name.upper()}: Voice = Voice {{ family: "{v["family"]}", '
            f'px: {v["px"]}, weight: {v["weight"]}, tracking_em: {v["tracking_em"]}, '
            f'line_height: {v["line_height"]}, upper: {str(v["transform"] == "uppercase").lower()} }};'
        )
    for name, value in sorted(geometry.items()):
        lines.append(f"pub const {name.upper()}: f32 = {value};")
    for name, value in sorted(spacing.items()):
        lines.append(f"pub const {name.upper()}: f32 = {value};")
    (DESIGN / "almagest7_tokens.rs").write_text("\n".join(lines) + "\n", encoding="utf-8")

    # The palettes get their own generated file, included by
    # `src/lexicon.rs` alone — the voices file is included by `voice.rs`
    # and a second consumer of the same file would duplicate its consts.
    pal_lines = [
        "// GENERATED by scripts/almagest-tokens.py from",
        f"// design/{VOLUME.name} — the canonical CSS token oracle.",
        "// Do not edit; re-run the script when the reference stylesheet changes.",
        "",
    ]
    for medium, vars_ in sorted(palette.items()):
        for name, value in vars_.items():
            const = f"{medium.upper()}_{name.upper()}"
            if name == "hairc":
                rgb = ", ".join(str(c) for c in value["rgb"])
                pal_lines.append(f"pub const {const}: [u8; 3] = [{rgb}];")
                pal_lines.append(
                    f"pub const {const}_ALPHA: f32 = {value['alpha']};"
                )
            else:
                rgb = ", ".join(str(c) for c in value)
                pal_lines.append(f"pub const {const}: [u8; 3] = [{rgb}];")
    (DESIGN / "almagest7_palette.rs").write_text("\n".join(pal_lines) + "\n", encoding="utf-8")

    # Since the a6 cutover the cockpit carries no px geometry at all:
    # the native window is composed in magistr-gui2 from this table, so
    # nothing is written into magistr-cockpit any more.
    print(
        f"derived {len(tokens)} voices + {len(geometry)} geometry facts "
        f"+ {len(spacing)} spreads from {VOLUME.name}"
    )


if __name__ == "__main__":
    sys.exit(main())
