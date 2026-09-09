use super::*;
use crate::math::contrast_ratio;
use crate::palette::{all_builtin_palettes, catppuccin_latte, catppuccin_mocha};

/// The four slots a GUI paints *behind* things with.
const GROUNDS: [PaletteField; 4] = [
    PaletteField::Base,
    PaletteField::Surface0,
    PaletteField::Surface1,
    PaletteField::Surface2,
];

#[test]
fn a_ground_is_the_palette_s_own_colour() {
    // A background must be exactly the colour the palette names — a
    // nudged base is a different theme, not a more legible one.
    for palette in all_builtin_palettes() {
        for field in GROUNDS {
            assert_eq!(
                ink(field, &palette),
                field.resolve(&palette),
                "{}: {} was moved",
                palette.name,
                field.as_str()
            );
        }
    }
}

#[test]
fn every_ink_slot_clears_its_floor_on_every_builtin() {
    for palette in all_builtin_palettes() {
        for field in PaletteField::ALL {
            let Contract::Ink { ratio, on_panels } = contract(field) else {
                continue;
            };
            let c = ink(field, &palette);
            let got = contrast_ratio(c, palette.base);
            assert!(
                got >= ratio,
                "{}: {} is {got:.2} on base, needs {ratio}",
                palette.name,
                field.as_str()
            );
            if on_panels {
                let got = contrast_ratio(c, palette.surface0);
                assert!(
                    got >= ratio,
                    "{}: {} is {got:.2} on surface0, needs {ratio}",
                    palette.name,
                    field.as_str()
                );
            }
        }
    }
}

#[test]
fn a_slot_that_already_clears_is_returned_untouched() {
    // Mocha measures clean at every floor this contract declares, so the
    // whole palette must come back byte-identical — which is what keeps
    // the emitted CSS of the default theme unmoved.
    let mocha = catppuccin_mocha();
    for field in PaletteField::ALL {
        assert_eq!(
            ink(field, &mocha),
            field.resolve(&mocha),
            "mocha: {} was moved",
            field.as_str()
        );
    }
}

#[test]
fn latte_s_five_failing_pairs_are_corrected() {
    // The five Palette::validate reports on Catppuccin Latte:
    // subtext0 4.37, blue 4.34 (need 4.5); green 2.96, yellow 2.31,
    // overlay0 2.30 (need 3.0). The palette itself is untouched.
    let latte = catppuccin_latte();
    for (field, floor) in [
        (PaletteField::Subtext0, 4.5),
        (PaletteField::Blue, 4.5),
        (PaletteField::Green, 4.5),
        (PaletteField::Yellow, 4.5),
        (PaletteField::Overlay0, 3.0),
    ] {
        let raw = contrast_ratio(field.resolve(&latte), latte.base);
        assert!(raw < floor, "{} no longer fails raw", field.as_str());
        let fixed = contrast_ratio(ink(field, &latte), latte.base);
        assert!(
            fixed >= floor,
            "{} is {fixed:.2} on Latte base, needs {floor}",
            field.as_str()
        );
    }
}

#[test]
fn the_palette_is_left_reporting_its_own_failures() {
    // ink() protects the consumer; it does not launder the palette.
    // Latte still fails validate(), and the ledger is unchanged.
    assert!(!catppuccin_latte().validate().is_empty());
}

#[test]
fn every_field_has_a_contract() {
    // A slot added to the palette has to declare which side of the
    // ground/ink line it lands on, rather than defaulting to silence.
    for field in PaletteField::ALL {
        let is_ground = GROUNDS.contains(&field);
        assert_eq!(
            matches!(contract(field), Contract::Ground),
            is_ground,
            "{} disagrees with the ground set",
            field.as_str()
        );
    }
}
