use super::*;

#[test]
fn the_four_receipts_wear_their_keys_and_verdicts() {
    let keys: Vec<&str> = ScaffoldKind::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(keys, ["config", "rules", "hooks", "store"]);
    let verdicts: Vec<&str> = ScaffoldKind::ALL.iter().map(|k| k.verdict()).collect();
    assert_eq!(verdicts, ["Written", "Written", "Armed", "Opened"]);
}
