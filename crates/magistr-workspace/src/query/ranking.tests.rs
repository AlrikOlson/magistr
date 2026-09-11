use super::*;
use crate::contracts::{Freshness, WorkspaceResponse};
use crate::query::SearchConfidence;
use crate::query::SearchHit;
use serde_json::json;

fn candidate(file: &str, test: bool, junk: bool, hits: Vec<SearchHit>) -> RankingCandidate {
    RankingCandidate {
        file: file.into(),
        test,
        junk,
        hits,
        test_ranges: Vec::new(),
    }
}

fn hit(line: u64, column: u32, excerpt: &str, kind: Option<&str>) -> SearchHit {
    let mut hit = SearchHit::new(line, column, excerpt.into());
    hit.kind = kind.map(str::to_owned);
    hit
}

#[test]
fn retained_full_pipeline_golden_is_identical() {
    let boilerplate = "module.exports = { needle: true };";
    let candidates = vec![
        candidate(
            "test/core.test.ts",
            true,
            false,
            vec![
                hit(1, 1, "needle(); needle();", None),
                hit(2, 1, "expect(needle).toBe(1);", None),
            ],
        ),
        candidate(
            "src/core.ts",
            false,
            false,
            vec![
                hit(10, 3, "needle();", None),
                hit(1, 17, "export function needle() {}", Some("function")),
            ],
        ),
        candidate(
            "src/docs.ts",
            false,
            false,
            vec![hit(4, 4, "// needle explains this", None)],
        ),
        candidate(
            "vendor/a.js",
            false,
            true,
            vec![hit(1, 1, boilerplate, None)],
        ),
        candidate(
            "vendor/b.js",
            false,
            true,
            vec![hit(1, 1, boilerplate, None)],
        ),
        candidate(
            "vendor/c.js",
            false,
            true,
            vec![hit(1, 1, boilerplate, None)],
        ),
    ];
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../subsystems/workspace/reference/test/support/golden/ranking/full-pipeline.json"
    ))
    .expect("retained ranking fixture");
    assert_eq!(
        serde_json::to_value(Ranking::rank(candidates, "needle")).expect("ranked rows"),
        expected
    );
}

fn golden(name: &str, actual: serde_json::Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../subsystems/workspace/reference/test/support/golden/ranking")
        .join(format!("{name}.json"));
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("retained golden"))
            .expect("golden JSON");
    assert_eq!(actual, expected);
}

#[test]
fn retained_stage1_classification_golden_is_identical() {
    let cases = [
        (
            "symbol tag function",
            1,
            "export function needle() {}",
            Some("function"),
        ),
        (
            "structural call node",
            1,
            "needle(1);",
            Some("call_expression"),
        ),
        (
            "structural import node",
            1,
            "import needle",
            Some("import_statement"),
        ),
        ("structural comment node", 1, "// needle", Some("comment")),
        (
            "structural string node",
            1,
            "\"needle\"",
            Some("string_fragment"),
        ),
        (
            "references definition",
            1,
            "fn needle()",
            Some("definition"),
        ),
        (
            "references use falls back to excerpt",
            1,
            "needle(v)",
            Some("use"),
        ),
        ("cold comment", 4, "// needle does things", None),
        ("cold hash comment", 3, "# needle in python", None),
        ("cold import", 10, "import { needle } from \"./mod\"", None),
        ("cold rust use", 5, "use crate::needle::Thing;", None),
        ("cold fn declaration", 4, "fn needle(x: u32) -> u32 {", None),
        ("cold const declaration", 7, "const needle = 42;", None),
        (
            "cold string span",
            14,
            "console.log(\"needle here\");",
            None,
        ),
        ("cold call", 1, "needle(argument);", None),
        ("cold bare mention", 5, "x + needle + y", None),
    ];
    golden("stage1-classify", json!(cases.into_iter().map(|(name, column, text, tag)| json!({"name": name, "class": classify(&hit(1, column, text, tag), &Classifier::new("needle")).name()})).collect::<Vec<_>>()));
}

#[test]
fn retained_stage2_file_class_golden_is_identical() {
    golden("stage2-file-class", json!([
        ("plain source", false, false), ("test path", true, false),
        ("vendor", false, true), ("vendored test is vendor", true, true),
    ].into_iter().map(|(name, test, junk)| json!({"name": name, "class": match class_of(&candidate("x", test, junk, vec![])) { 2 => "source", 1 => "test", _ => "vendor" }})).collect::<Vec<_>>()));
}

fn classified(
    line: u64,
    text: &str,
    class: KindClass,
    confidence: Option<SearchConfidence>,
) -> ClassifiedHit {
    let mut hit = hit(line, 1, text, None);
    hit.confidence = confidence;
    ClassifiedHit { hit, class }
}

fn classified_group(file: &str, hits: Vec<ClassifiedHit>, class: u8) -> ClassifiedGroup {
    ClassifiedGroup {
        file: file.into(),
        junk: class == 0,
        file_class: class,
        total_hits: hits.len(),
        hits,
    }
}

#[test]
fn retained_stage3_hit_order_golden_is_identical() {
    let mut hits = vec![
        classified(2, "// mentions needle", KindClass::Comment, None),
        classified(8, "needle();", KindClass::Call, None),
        classified(5, "function needle() {}", KindClass::Declaration, None),
        classified(
            3,
            "needle(x);",
            KindClass::Call,
            Some(SearchConfidence::Low),
        ),
        classified(
            9,
            "needle(y);",
            KindClass::Call,
            Some(SearchConfidence::High),
        ),
        classified(1, "x = needle", KindClass::Other, None),
    ];
    order_hits(&mut hits);
    golden(
        "stage3-order-hits",
        json!(
            hits.iter()
                .map(|hit| {
                    let mut row = json!({"line": hit.hit.line, "kindClass": hit.class.name()});
                    if let Some(confidence) = hit.hit.confidence {
                        row["confidence"] = json!(confidence);
                    }
                    row
                })
                .collect::<Vec<_>>()
        ),
    );
}

#[test]
fn retained_stage4_dedup_golden_is_identical() {
    let repeated = "export const GENERATED = \"needle\";";
    let mut groups = vec![
        classified_group(
            "vendor/gen-a.ts",
            vec![classified(1, repeated, KindClass::Declaration, None)],
            0,
        ),
        classified_group(
            "src/real.ts",
            vec![
                classified(3, repeated, KindClass::Declaration, None),
                classified(9, "needle();", KindClass::Call, None),
            ],
            2,
        ),
        classified_group(
            "vendor/gen-b.ts",
            vec![classified(1, repeated, KindClass::Declaration, None)],
            0,
        ),
        classified_group(
            "src/pair-one.ts",
            vec![classified(2, "twice(needle);", KindClass::Call, None)],
            2,
        ),
        classified_group(
            "src/pair-two.ts",
            vec![classified(4, "twice(needle);", KindClass::Call, None)],
            2,
        ),
    ];
    dedup(&mut groups);
    golden(
        "stage4-dedup",
        json!(groups.iter().map(|group| json!({
        "file": group.file, "totalHits": group.total_hits,
        "hits": group.hits.iter().map(|hit| {
            let mut row = json!({"line": hit.hit.line, "excerpt": hit.hit.excerpt});
            if let Some(count) = hit.hit.duplicates { row["duplicates"] = json!(count); }
            row
        }).collect::<Vec<_>>()
    })).collect::<Vec<_>>()),
    );
}

#[test]
fn retained_stage5_group_order_golden_is_identical() {
    let mut groups = vec![
        classified_group(
            "src/deep/nested/mention.ts",
            vec![classified(1, "needle + 1", KindClass::Other, None)],
            2,
        ),
        classified_group(
            "test/needle.test.ts",
            vec![
                classified(1, "needle();", KindClass::Call, None),
                classified(2, "needle();", KindClass::Call, None),
            ],
            1,
        ),
        classified_group(
            "test/def-here.test.ts",
            vec![classified(
                1,
                "function needle() {}",
                KindClass::Declaration,
                None,
            )],
            1,
        ),
        classified_group(
            "vendor/lock.json",
            vec![classified(
                1,
                "\"needle\": \"1.0.0\"",
                KindClass::String,
                None,
            )],
            0,
        ),
        classified_group(
            "src/needle/core.ts",
            vec![classified(1, "needle + 2", KindClass::Other, None)],
            2,
        ),
        classified_group(
            "src/refs-high.ts",
            vec![classified(
                1,
                "needle(a)",
                KindClass::Call,
                Some(SearchConfidence::High),
            )],
            2,
        ),
        classified_group(
            "src/refs-low.ts",
            vec![classified(
                1,
                "needle(b)",
                KindClass::Call,
                Some(SearchConfidence::Low),
            )],
            2,
        ),
    ];
    order_groups(&mut groups, &["needle".into()]);
    golden(
        "stage5-order-groups",
        json!(groups.iter().map(|group| &group.file).collect::<Vec<_>>()),
    );
}

fn wire(file: &str, text: &str) -> SearchGroup {
    SearchGroup {
        file: file.into(),
        junk: false,
        total_hits: 1,
        hits: vec![hit(1, 1, text, None)],
    }
}

fn trimmed(rows: Vec<SearchGroup>, budget: u64) -> serde_json::Value {
    let response = crate::query::budget::success(
        rows,
        budget,
        Freshness::Cold,
        vec![],
        |group| format!("file:{}", group.file),
        vec![],
    );
    match response {
        WorkspaceResponse::Success {
            results, budget, ..
        } => json!({"keep": results.rows().len(), "used": budget.used}),
        WorkspaceResponse::Failure { error, .. } => {
            assert_eq!(error.kind, crate::contracts::ErrorKind::BudgetTooSmall);
            json!({"keep":0,"used":0})
        }
    }
}

#[test]
fn retained_stage6_budget_golden_is_identical() {
    let groups = vec![
        wire("a.ts", "short"),
        wire("b.ts", &"x".repeat(200)),
        wire("c.ts", &"y".repeat(200)),
    ];
    golden(
        "stage6-trim",
        json!({
            "allFit": trimmed(groups.clone(), 4000), "partial": trimmed(groups, 100),
            "firstTooBig": trimmed(vec![wire("big.ts", &"z".repeat(4000))], 100), "empty": trimmed(vec![], 100),
        }),
    );
}

#[test]
fn rust_test_regions_demote_hits_and_test_only_files() {
    let mut first = candidate(
        "src/lib.rs",
        false,
        false,
        vec![
            hit(25, 9, "let c = compute(2);", None),
            hit(5, 5, "compute(input)", None),
        ],
    );
    first.test_ranges = vec![crate::worker::WorkerTestRange {
        start_line: 20,
        end_line: 40,
    }];
    let groups = Ranking::rank(vec![first.clone()], "compute");
    assert_eq!(
        groups[0]
            .hits
            .iter()
            .map(|hit| hit.line)
            .collect::<Vec<_>>(),
        [5, 25]
    );
    assert_eq!(groups[0].hits[1].in_test_range, Some(true));
    first.hits.remove(1);
    let groups = Ranking::rank(
        vec![
            first,
            candidate(
                "src/other.rs",
                false,
                false,
                vec![hit(3, 1, "compute(input)", None)],
            ),
        ],
        "compute",
    );
    assert_eq!(groups[0].file, "src/other.rs");
}
