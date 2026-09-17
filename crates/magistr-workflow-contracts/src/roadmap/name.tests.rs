use super::*;

/// Ids this roadmap has never held. The point of a table the live data has
/// never seen is that passing it says something about the next chunk
/// somebody writes, not about the 519 that already exist.
const FOREIGN: &[(&str, &str)] = &[
    // Leading article dropped, trailing modal trimmed.
    ("the-widget-registry-should-flush", "Widget registry"),
    // Acronym uppercased where title case would read as a typo.
    ("ftp-upload-retries-forever", "FTP upload retries"),
    ("an-ldap-bind-leaks-a-handle", "LDAP bind leaks"),
    // Trailing copula trimmed rather than kept pointing at a dropped word.
    ("quota-ceiling-is-unenforced", "Quota ceiling"),
    // Three-word cap, comfortably inside the budget.
    (
        "shipping-labels-print-blank-on-thermal",
        "Shipping labels print",
    ),
    // Budget binds before the word cap does.
    ("reconciliation-statements-duplicate", "Reconciliation"),
    // Underscores and dots are separators too.
    ("payroll_export.breaks_on_leap_day", "Payroll export breaks"),
    // A single word, already a fine label.
    ("harbourmaster", "Harbourmaster"),
];

#[test]
fn derives_readable_names_for_ids_this_roadmap_has_never_held() {
    for (id, want) in FOREIGN {
        assert_eq!(&derive(id), want, "deriving a name for {id}");
    }
}

/// The cross-language contract. A chunk written before `name` existed keeps a
/// cloud copy that is only rewritten when the chunk is mutated, so the canvas
/// derives a label for records this engine will never get to seed — and the
/// two derivations have to agree, or the same chunk wears one name locally
/// and another on the web.
///
/// The table lives outside both languages (`shared/name-derivation.cases.json`)
/// so neither side owns it and neither can be quietly relaxed to match a
/// regression. `frontend/src/tree/name.test.ts` drives the same file.
#[test]
fn agrees_with_the_typescript_derivation_on_the_shared_table() {
    const SHARED: &str =
        include_str!("../../../../subsystems/workflow/shared/name-derivation.cases.json");
    let table: serde_json::Value =
        serde_json::from_str(SHARED).expect("the shared derivation table must parse");

    assert_eq!(
        table["budget"].as_u64().expect("budget"),
        NAME_BUDGET as u64,
        "the shared table's budget must be C8's budget, or the two sides are \
         agreeing about different things"
    );

    let cases = table["cases"].as_array().expect("cases");
    assert!(
        cases.len() >= 15,
        "a contract table this small stops being evidence: {} cases",
        cases.len()
    );

    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let want = case["name"].as_str().expect("case name");
        assert_eq!(derive(id), want, "deriving a name for {id:?}");
    }
}

/// The gate the whole chunk exists to install: nothing this function can
/// emit may exceed C8's budget, whatever it is handed.
#[test]
fn every_derived_name_fits_the_budget() {
    let hostile = [
        "a",
        "the",
        "-",
        "---",
        "",
        "supercalifragilisticexpialidocious-considerations",
        "declaring-identity-requires-configuring-clients",
        "ünïcödé-wörd-thät-rüns-lóng-ènough-to-clamp",
        "ONE-TWO-THREE",
        "a-b-c-d-e-f-g-h-i-j-k",
    ];
    for id in hostile {
        let got = derive(id);
        assert!(
            got.chars().count() <= NAME_BUDGET,
            "derive({id:?}) produced {} chars: {got:?}",
            got.chars().count()
        );
    }
    for (id, _) in FOREIGN {
        assert!(fits(&derive(id)), "derive({id:?}) must satisfy fits()");
    }
}

/// An id made only of noise words still names itself. Trimming to nothing
/// would hand the canvas a blank node, which is worse than a weak label.
#[test]
fn never_trims_a_name_away_entirely() {
    for id in ["the", "a-the", "is", "of-the", "the-is-of"] {
        assert!(!derive(id).is_empty(), "derive({id:?}) must not be empty");
    }
}

/// Deterministic: positions on the canvas are learned, so a name that
/// changed between runs would move the label under a node that did not move.
#[test]
fn derivation_is_deterministic() {
    for (id, _) in FOREIGN {
        assert_eq!(derive(id), derive(id));
    }
}

/// The one input with no name to give. Stated as a test so the exception is
/// pinned rather than discovered.
#[test]
fn an_empty_id_derives_to_nothing() {
    assert_eq!(derive(""), "");
    assert!(!fits(&derive("")));
}

#[test]
fn fits_rejects_the_empty_and_the_oversized() {
    assert!(!fits(""));
    assert!(!fits("   "));
    assert!(fits("Quota ceiling"));
    assert!(fits(&"x".repeat(NAME_BUDGET)));
    assert!(!fits(&"x".repeat(NAME_BUDGET + 1)));
}

#[test]
fn why_unfit_names_the_actual_failure() {
    assert_eq!(why_unfit("").as_deref(), Some("has no name"));
    assert!(
        why_unfit(&"x".repeat(NAME_BUDGET + 1))
            .unwrap()
            .contains("over the")
    );
    assert_eq!(why_unfit("Quota ceiling"), None);
}

/// A sentence-shaped label is exactly what this chunk exists to prevent, so
/// the budget has to reject one outright.
#[test]
fn a_sentence_cannot_pass_as_a_name() {
    let sentence = "Every chunk's label is a sentence, so nothing in the roadmap can be a node";
    assert!(!fits(sentence));
}

/// An id family this roadmap has never held, seeded the way the engine
/// seeds: every tail past three words vanishes, so five distinct chunks
/// wear one word. The repair must separate them without moving the head.
fn orchard() -> Vec<(String, String)> {
    [
        "orchard-plot-water",
        "orchard-plot-water-pump",
        "orchard-plot-water-piping",
        "orchard-plot-water-meter",
        "orchard-plot-water-meter-dial",
    ]
    .iter()
    .map(|id| ((*id).to_string(), derive(id)))
    .collect()
}

fn as_refs(table: &[(String, String)]) -> Vec<(&str, &str)> {
    table
        .iter()
        .map(|(id, name)| (id.as_str(), name.as_str()))
        .collect()
}

#[test]
fn colliding_seeds_are_separated_and_the_family_head_keeps_its_label() {
    let table = orchard();
    // The premise: the seed really does tie all five.
    for (_, name) in &table {
        assert_eq!(name, "Orchard plot water");
    }
    let rewrites = repair_collisions(&as_refs(&table));

    // The head — the id that IS the shared prefix — is not rewritten.
    assert!(rewrites.iter().all(|(id, _)| id != "orchard-plot-water"));

    // Every other member now wears context + its distinguishing tail.
    let get = |id: &str| {
        rewrites
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, n)| n.as_str())
            .unwrap_or_else(|| panic!("{id} must be rewritten"))
    };
    assert_eq!(get("orchard-plot-water-pump"), "Water pump");
    assert_eq!(get("orchard-plot-water-piping"), "Water piping");
    assert_eq!(get("orchard-plot-water-meter"), "Water meter");
    assert_eq!(get("orchard-plot-water-meter-dial"), "Water meter dial");

    // The repair actually repaired: no two labels tie afterwards.
    let mut labels: Vec<&str> = table
        .iter()
        .map(|(id, name)| get_or(&rewrites, id, name))
        .collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(
        labels.len(),
        table.len(),
        "all five labels must be distinct"
    );
    for l in labels {
        assert!(fits(l), "{l:?} must satisfy fits()");
    }
}

fn get_or<'a>(rewrites: &'a [(String, String)], id: &str, current: &'a str) -> &'a str {
    rewrites
        .iter()
        .find(|(i, _)| i == id)
        .map_or(current, |(_, n)| n.as_str())
}

/// Members still tied after one pass share a longer prefix and recurse:
/// `…-north-latch-upper` and `…-north-latch-lower` both truncate to
/// "Gate north latch" until the recursion reaches the words that differ.
#[test]
fn nested_families_recurse_until_the_labels_differ() {
    let ids = [
        "mill-race-gate",
        "mill-race-gate-north",
        "mill-race-gate-north-latch-upper",
        "mill-race-gate-north-latch-lower",
    ];
    let table: Vec<(String, String)> = ids
        .iter()
        .map(|id| ((*id).to_string(), derive(id)))
        .collect();
    let rewrites = repair_collisions(&as_refs(&table));

    let finals: Vec<&str> = table
        .iter()
        .map(|(id, name)| get_or(&rewrites, id, name))
        .collect();
    assert_eq!(finals[0], "Mill race gate", "the head keeps its seed");
    assert_eq!(finals[1], "Gate north");
    assert_eq!(finals[2], "Latch upper");
    assert_eq!(finals[3], "Latch lower");
}

/// The two prohibitions, driven together: an authored name is never moved
/// even when it sits inside a colliding class, and a label nobody collides
/// with is never touched. The authored name here also squats on the label
/// a seed would want, forcing the context window to widen past it.
#[test]
fn authored_names_and_uncolliding_labels_are_never_touched() {
    let table: Vec<(String, String)> = vec![
        // Two seeds tied at "Orchard plot water".
        (
            "orchard-plot-water-pump".into(),
            derive("orchard-plot-water-pump"),
        ),
        (
            "orchard-plot-water-piping".into(),
            derive("orchard-plot-water-piping"),
        ),
        // An authored member of the same class: name != derive(id).
        ("rainbarrel-stand".into(), "Orchard plot water".into()),
        // An authored squatter on the label the pump seed would take.
        ("cistern-overflow".into(), "Water pump".into()),
        // A singleton seed: collides with nothing, must not move.
        ("harbourmaster".into(), derive("harbourmaster")),
    ];
    assert_ne!(derive("rainbarrel-stand"), "Orchard plot water");
    let rewrites = repair_collisions(&as_refs(&table));

    assert!(
        rewrites.iter().all(|(id, _)| id != "rainbarrel-stand"),
        "an authored name is never rewritten, colliding or not"
    );
    assert!(
        rewrites.iter().all(|(id, _)| id != "cistern-overflow"),
        "an authored label outside the class is never rewritten"
    );
    assert!(
        rewrites.iter().all(|(id, _)| id != "harbourmaster"),
        "a label nobody collides with is never touched"
    );
    // The pump seed could not take "Water pump" (authored squatter): the
    // context window widens until the label is free.
    let pump = rewrites
        .iter()
        .find(|(id, _)| id == "orchard-plot-water-pump")
        .map(|(_, n)| n.as_str())
        .expect("the pump seed must move");
    assert_ne!(pump, "Water pump");
    assert!(fits(pump));
}

/// Deterministic and order-independent: the store's serialization order is
/// an accident, and a label that depended on it would move between loads.
#[test]
fn repair_is_deterministic_and_order_independent() {
    let mut table = orchard();
    let forward = repair_collisions(&as_refs(&table));
    table.reverse();
    let reversed = repair_collisions(&as_refs(&table));
    assert_eq!(forward, reversed);
    assert!(!forward.is_empty());
}

/// Idempotent by construction: a rewritten name no longer equals its seed,
/// so the next pass sees an authored-looking label and leaves it alone.
#[test]
fn a_second_pass_finds_nothing_left_to_repair() {
    let mut table = orchard();
    let rewrites = repair_collisions(&as_refs(&table));
    assert!(!rewrites.is_empty());
    for (id, name) in &mut table {
        if let Some((_, n)) = rewrites.iter().find(|(i, _)| i == id) {
            *name = n.clone();
        }
    }
    assert_eq!(repair_collisions(&as_refs(&table)), vec![]);
}

/// Nameless rows belong to the backfill, not to a label class — an empty
/// string tying with another empty string is not a collision.
#[test]
fn empty_names_are_not_a_collision_class() {
    let table: Vec<(String, String)> = vec![
        ("first-nameless".into(), String::new()),
        ("second-nameless".into(), String::new()),
    ];
    assert_eq!(repair_collisions(&as_refs(&table)), vec![]);
}
