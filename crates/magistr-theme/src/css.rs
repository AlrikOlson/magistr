//! CSS custom property generation from palettes.
//!
//! Produces CSS custom properties (variables) from a `Palette` or `OwnedPalette`,
//! enabling the GUI frontend to use the same theme definitions as the TUI.

use crate::color::ThemeColor;
use crate::ink::ink;
use crate::mapping::{PaletteField, SemanticRole, ThemeMapping};
use crate::palette::{OwnedPalette, Palette};

/// Generate CSS custom properties from a `Palette`.
///
/// Returns a string of CSS custom property declarations (without the `:root {}` wrapper)
/// that can be injected into a `<style>` tag or CSS file.
///
/// Produces two groups of properties:
/// 1. **Palette colors** — the color slots: `--magistr-base`, `--magistr-blue`, etc.
/// 2. **Semantic tokens** — resolved via `ThemeMapping`: `--magistr-primary`, `--magistr-error`, etc.
///
/// Both groups go through [`crate::ink`], which is what makes this a
/// *token* pipeline rather than a colour dump. A ground — `base` and the
/// three surfaces, and any token pointing at one — is emitted exactly as
/// the palette names it; every other slot is nudged until it clears the
/// floor [`Palette::validate`] declares for it on the palette's base.
/// The contract hangs off the palette field rather than the semantic
/// role, so a mapping that points `error` at `mauve` gets mauve's rule
/// and the two families can never disagree. A palette that already
/// passes its own validator — Catppuccin Mocha does — is emitted
/// unchanged.
///
/// # Example output
///
/// ```css
/// --magistr-base: #1e1e2e;
/// --magistr-surface0: #313244;
/// ...
/// --magistr-primary: #89b4fa;
/// --magistr-error: #f38ba8;
/// ```
pub fn palette_to_css(palette: &Palette, mapping: &ThemeMapping) -> String {
    let mut out = String::with_capacity(1024);

    // Palette color slots
    for field in PaletteField::ALL {
        let color = ink(field, palette);
        write_property(&mut out, &format!("magistr-{}", field.as_str()), color);
    }

    out.push('\n');

    // Semantic tokens
    for role in SemanticRole::ALL {
        let color = ink(mapping.field_for(role), palette);
        write_property(&mut out, &format!("magistr-{}", role.as_str()), color);
    }

    out
}

/// Generate CSS custom properties from an `OwnedPalette`.
pub fn owned_palette_to_css(palette: &OwnedPalette, mapping: &ThemeMapping) -> String {
    palette_to_css(&palette.as_ref_palette(), mapping)
}

/// Generate a complete CSS `:root` block with custom properties.
pub fn palette_to_css_root(palette: &Palette, mapping: &ThemeMapping) -> String {
    let props = palette_to_css(palette, mapping);
    format!(":root {{\n{props}}}\n")
}

/// Write a single CSS custom property line.
fn write_property(out: &mut String, name: &str, color: ThemeColor) {
    out.push_str(&format!("  --{name}: {};\n", color.to_hex()));
}

#[cfg(test)]
#[path = "css.tests.rs"]
mod tests;
