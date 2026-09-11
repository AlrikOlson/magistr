//! Native edit/files/undo acceptance through the real parser worker, derived
//! index, rustfmt and cargo: the retained edit-verbs, files-tool, undo and
//! rust-move fixtures, plus stale previews, rename ambiguity, mixed batches,
//! partial failure with compensation and undo after external edits.
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use magistr_workspace::{
    commands::{
        EditInput, EditOp, FilesInput, FilesOp, UndoInput, UndoRow, UndoScope, WorkspaceCommands,
        WriteAction, WriteReport,
    },
    config::ResolveOptions,
    confirmation::ConfirmationSeal,
    contracts::{ErrorKind, WorkspaceResponse},
    file_content::FileContent,
    index::{IndexOptions, WorkspaceIndex, watch::WatchMode},
    jail::WorkspaceKeyStore,
    journal::{JournalDb, JournalDbOptions, JournalWriteOutcome, UndoCandidateQuery},
    transaction::FormatterOptions,
    undo::UndoConflictPolicy,
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    policy: WorkspacePolicy,
    worker: Arc<WorkerSupervisor>,
    journal: JournalDb,
    seal: ConfirmationSeal,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("editing fixture");
        let root = directory.path().join("workspace");
        fs::create_dir_all(root.join(".git")).expect("repository marker");
        let key = WorkspaceKeyStore::new(root.join(".workbench"))
            .ensure()
            .expect("workspace key");
        let journal = JournalDb::open(JournalDbOptions::new(root.join(".workbench/journal.db")))
            .expect("sole test journal writer");
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(&root)).expect("policy");
        let worker = Arc::new(
            WorkerSupervisor::new(
                PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
                WorkerOptions::default(),
            )
            .expect("native parser worker"),
        );
        Self {
            directory,
            root,
            policy,
            worker,
            journal,
            seal: ConfirmationSeal::new(&key, 1),
        }
    }

    fn seed(&self, files: &[(&str, &str)]) {
        for (path, content) in files {
            self.write(path, content);
        }
    }

    fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("directory");
        fs::write(file, content).expect("fixture source");
    }

    fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path)).ok()
    }

    fn index(&self) -> WorkspaceIndex {
        WorkspaceIndex::start(
            self.root.clone(),
            self.directory.path().join("state"),
            Arc::clone(&self.worker),
            IndexOptions {
                watch_mode: WatchMode::Off,
                poll_interval: Duration::from_millis(50),
                ..IndexOptions::default()
            },
        )
        .expect("native index")
    }

    async fn warm_index(&self) -> WorkspaceIndex {
        let index = self.index();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let status = index.status();
            if !status.building && status.generation > 0 {
                return index;
            }
            assert!(Instant::now() < deadline, "index did not warm: {status:?}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn commands<'a>(
        &'a mut self,
        index: Option<&'a WorkspaceIndex>,
        session: &str,
    ) -> WorkspaceCommands<'a> {
        WorkspaceCommands::new(
            &self.policy,
            &self.worker,
            &mut self.journal,
            &self.seal,
            index,
            FormatterOptions::default(),
            session,
        )
        .expect("native commands")
    }

    async fn shutdown(self, mut index: Option<WorkspaceIndex>) {
        if let Some(index) = index.as_mut() {
            index.shutdown().await.expect("index shutdown");
        }
        self.worker.shutdown().await.expect("reap parser child");
    }
}

fn ok<R: Clone + std::fmt::Debug>(response: WorkspaceResponse<R>) -> (Vec<R>, Vec<String>) {
    match response {
        WorkspaceResponse::Success { results, notes, .. } => {
            (results.rows().to_vec(), notes.unwrap_or_default())
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("expected success, got {error} {notes:?}")
        }
    }
}

fn refused<R: std::fmt::Debug>(response: WorkspaceResponse<R>) -> (ErrorKind, Vec<String>) {
    match response {
        WorkspaceResponse::Failure { error, notes, .. } => (error.kind, notes.unwrap_or_default()),
        WorkspaceResponse::Success { results, notes, .. } => {
            panic!("expected refusal, got {:?} {notes:?}", results.rows())
        }
    }
}

/// Word-boundary occurrence check without a regex dependency.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn hash(text: &str) -> String {
    FileContent::Text(text.into()).content_hash()
}

fn edit(op: EditOp, file: &str) -> EditInput {
    let mut input = EditInput::new(op);
    input.file = Some(file.into());
    input
}

fn symbol_edit(op: EditOp, file: &str, symbol: &str, content: &str) -> EditInput {
    let mut input = edit(op, file);
    input.symbol = Some(symbol.into());
    input.content = Some(content.into());
    input
}

fn files(op: FilesOp, from: Option<&str>, to: Option<&str>) -> FilesInput {
    let mut input = FilesInput::new(op);
    input.from = from.map(str::to_owned);
    input.to = to.map(str::to_owned);
    input
}

fn moved(from: &str, to: &str) -> FilesInput {
    let mut input = files(FilesOp::Move, Some(from), Some(to));
    input.auto_apply = true;
    input
}

/// One language fixture from the retained edit-verbs suite: a two-function
/// module, a replacement body that still parses, and a line to insert.
struct VerbFixture {
    lang: &'static str,
    path: &'static str,
    content: &'static str,
    new_body: &'static str,
    inserted: &'static str,
}

const VERB_FIXTURES: [VerbFixture; 6] = [
    VerbFixture {
        lang: "typescript",
        path: "src/alpha.ts",
        content: "function alpha(): number {\n  return 1;\n}\n\nfunction beta(): number {\n  return alpha() + 1;\n}\n",
        new_body: "function alpha(): number {\n  return 42;\n}",
        inserted: "// inserted-line",
    },
    VerbFixture {
        lang: "javascript",
        path: "src/alpha.js",
        content: "function alpha() {\n  return 1;\n}\n\nfunction beta() {\n  return alpha() + 1;\n}\n",
        new_body: "function alpha() {\n  return 42;\n}",
        inserted: "// inserted-line",
    },
    VerbFixture {
        lang: "python",
        path: "src/alpha.py",
        content: "def alpha():\n    return 1\n\n\ndef beta():\n    return alpha() + 1\n",
        new_body: "def alpha():\n    return 42",
        inserted: "# inserted-line",
    },
    VerbFixture {
        lang: "rust",
        path: "src/alpha.rs",
        content: "fn alpha() -> u32 {\n    1\n}\n\nfn beta() -> u32 {\n    alpha() + 1\n}\n",
        new_body: "fn alpha() -> u32 {\n    42\n}",
        inserted: "// inserted-line",
    },
    VerbFixture {
        lang: "go",
        path: "src/alpha.go",
        content: "package main\n\nfunc alpha() int {\n\treturn 1\n}\n\nfunc beta() int {\n\treturn alpha() + 1\n}\n",
        new_body: "func alpha() int {\n\treturn 42\n}",
        inserted: "// inserted-line",
    },
    VerbFixture {
        lang: "java",
        path: "src/Alpha.java",
        content: "class Alpha {\n  static int alpha() {\n    return 1;\n  }\n\n  static int beta() {\n    return alpha() + 1;\n  }\n}\n",
        new_body: "  static int alpha() {\n    return 42;\n  }",
        inserted: "  // inserted-line",
    },
];

#[tokio::test]
async fn every_edit_verb_against_fixture_repos_in_six_languages() {
    for fixture in &VERB_FIXTURES {
        let mut f = Fixture::new();
        // The retained rig seeds every language fixture into one workspace.
        for other in &VERB_FIXTURES {
            f.write(other.path, other.content);
        }
        let index = f.warm_index().await;
        let lang = fixture.lang;

        let (rows, _) = ok(f
            .commands(Some(&index), "s")
            .edit(&symbol_edit(
                EditOp::ReplaceBody,
                fixture.path,
                "alpha",
                fixture.new_body,
            ))
            .await);
        assert_eq!(rows.len(), 1, "{lang}");
        assert!(rows[0].applied && rows[0].diff.contains('+'), "{lang}");
        let after = f.read(fixture.path).expect("source");
        assert!(
            after.contains("42") && !after.contains("return 1") && after.contains("beta"),
            "{lang}: {after}"
        );

        f.write(fixture.path, fixture.content);
        let index = f.warm_index_after(index).await;
        ok(f.commands(Some(&index), "s")
            .edit(&symbol_edit(
                EditOp::InsertAfter,
                fixture.path,
                "alpha",
                fixture.inserted,
            ))
            .await);
        let after = f.read(fixture.path).expect("source");
        let inserted = after.find("inserted-line").expect("inserted after alpha");
        assert!(
            inserted > after.find("alpha").expect("alpha")
                && inserted < after.find("beta").expect("beta"),
            "{lang}: {after}"
        );

        f.write(fixture.path, fixture.content);
        let index = f.warm_index_after(index).await;
        ok(f.commands(Some(&index), "s")
            .edit(&symbol_edit(
                EditOp::InsertBefore,
                fixture.path,
                "beta",
                fixture.inserted,
            ))
            .await);
        let after = f.read(fixture.path).expect("source");
        let inserted = after.find("inserted-line").expect("inserted before beta");
        let beta = after.find("beta").expect("beta");
        assert!(
            inserted < beta && after[inserted..beta].matches('\n').count() <= 2,
            "{lang}: {after}"
        );

        f.write(fixture.path, fixture.content);
        let index = f.warm_index_after(index).await;
        let mut rename = edit(EditOp::Rename, fixture.path);
        rename.symbol = Some("alpha".into());
        rename.new_name = Some("gamma".into());
        let (kind, _) = refused(f.commands(Some(&index), "s").edit(&rename).await);
        assert!(matches!(kind, ErrorKind::ConfirmRequired { .. }), "{lang}");
        rename.auto_apply = true;
        ok(f.commands(Some(&index), "s").edit(&rename).await);
        let after = f.read(fixture.path).expect("source");
        assert!(!has_word(&after, "alpha"), "{lang}: {after}");
        assert!(after.contains("gamma"), "{lang}");
        if lang == "java" {
            assert!(
                after.contains("class Alpha"),
                "word boundary keeps the class"
            );
        }

        f.write(fixture.path, fixture.content);
        let index = f.warm_index_after(index).await;
        let mut rewrite = EditInput::new(EditOp::Rewrite);
        rewrite.pattern = Some("return 1".into());
        rewrite.rewrite_pattern = Some("return 7".into());
        rewrite.path = Some("src/**".into());
        rewrite.auto_apply = true;
        let (rows, _) = ok(f.commands(Some(&index), "s").edit(&rewrite).await);
        assert!(!rows.is_empty(), "{lang}");
        assert!(
            !f.read(fixture.path).expect("source").contains("return 1;"),
            "{lang}"
        );

        f.write(fixture.path, fixture.content);
        let mut replace = edit(EditOp::ReplaceText, fixture.path);
        replace.old = Some("1".into());
        replace.new = Some("9".into());
        replace.occurrence = Some(1);
        let (rows, _) = ok(f.commands(Some(&index), "s").edit(&replace).await);
        assert!(rows[0].applied, "{lang}");
        assert!(
            f.read(fixture.path).expect("source").contains('9'),
            "{lang}"
        );
        f.shutdown(Some(index)).await;
    }
}

impl Fixture {
    /// Refresh the derived index after a direct fixture write.
    async fn warm_index_after(&self, index: WorkspaceIndex) -> WorkspaceIndex {
        let before = index.status().generation;
        index.request_refresh();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let status = index.status();
            if !status.building && status.generation > before {
                return index;
            }
            assert!(
                Instant::now() < deadline,
                "index did not refresh: {status:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

#[tokio::test]
async fn parse_rejection_stale_anchors_and_taxonomy_edges_answer_in_the_same_call() {
    let mut f = Fixture::new();
    let fixture = &VERB_FIXTURES[0];
    f.write(fixture.path, fixture.content);
    f.write(
        "src/caller.ts",
        "import { alpha } from './alpha';\nexport const x = alpha();\n",
    );
    let index = f.warm_index().await;

    let (kind, _) = refused(
        f.commands(Some(&index), "s")
            .edit(&symbol_edit(
                EditOp::ReplaceBody,
                fixture.path,
                "alpha",
                "function alpha() {\n  return (1;\n}",
            ))
            .await,
    );
    let ErrorKind::ParseRejected {
        parser_message,
        rejected_diff,
    } = kind
    else {
        panic!("parse rejection: {kind:?}");
    };
    assert!(!parser_message.is_empty());
    assert!(rejected_diff.contains("+  return (1;"), "{rejected_diff}");
    assert_eq!(f.read(fixture.path).as_deref(), Some(fixture.content));

    let read_hash = hash(fixture.content);
    let changed = format!("// drifted\n{}", fixture.content);
    f.write(fixture.path, &changed);
    let mut stale = edit(EditOp::ReplaceText, fixture.path);
    stale.old = Some("return 1;".into());
    stale.new = Some("return 2;".into());
    stale.expected_hash = Some(read_hash);
    let (kind, _) = refused(f.commands(Some(&index), "s").edit(&stale).await);
    assert_eq!(
        kind,
        ErrorKind::StaleAnchor {
            content_hash: hash(&changed)
        }
    );
    f.write(fixture.path, fixture.content);
    let index = f.warm_index_after(index).await;

    let (kind, _) = refused(
        f.commands(None, "s")
            .edit(&symbol_edit(
                EditOp::ReplaceBody,
                fixture.path,
                "alpha",
                "x",
            ))
            .await,
    );
    assert_eq!(kind, ErrorKind::IndexUnavailable);
    let (kind, _) = refused(
        f.commands(Some(&index), "s")
            .edit(&symbol_edit(EditOp::ReplaceBody, fixture.path, "nope", "x"))
            .await,
    );
    assert_eq!(kind, ErrorKind::SymbolNotFound);
    let mut bad_name = edit(EditOp::Rename, fixture.path);
    bad_name.symbol = Some("alpha".into());
    bad_name.new_name = Some("not a name".into());
    let (kind, _) = refused(f.commands(Some(&index), "s").edit(&bad_name).await);
    assert_eq!(kind, ErrorKind::PatternInvalid);
    let mut nothing = EditInput::new(EditOp::Rewrite);
    nothing.pattern = Some("no_such_text_anywhere".into());
    nothing.rewrite_pattern = Some("x".into());
    nothing.auto_apply = true;
    let (rows, notes) = ok(f.commands(Some(&index), "s").edit(&nothing).await);
    assert!(rows.is_empty());
    assert!(notes.join("\n").contains("nothing to change"));

    // Ambiguity: a second `alpha` in the same file is a real ambiguity.
    f.write(
        fixture.path,
        format!(
            "{}\nfunction alpha(): number {{\n  return 3;\n}}\n",
            fixture.content
        ),
    );
    let index = f.warm_index_after(index).await;
    let (kind, _) = refused(
        f.commands(Some(&index), "s")
            .edit(&symbol_edit(
                EditOp::ReplaceBody,
                fixture.path,
                "alpha",
                "x",
            ))
            .await,
    );
    assert!(
        matches!(kind, ErrorKind::AmbiguousSymbol { .. }),
        "{kind:?}"
    );
    f.write(fixture.path, fixture.content);
    let index = f.warm_index_after(index).await;

    // Cross-file rename through the choke point: one journal seq for both files.
    let mut rename = edit(EditOp::Rename, fixture.path);
    rename.symbol = Some("alpha".into());
    rename.new_name = Some("gamma".into());
    rename.auto_apply = true;
    let (rows, notes) = ok(f.commands(Some(&index), "session-a").edit(&rename).await);
    assert!(rows.len() >= 2, "{rows:?}");
    assert!(f.read("src/caller.ts").expect("caller").contains("gamma()"));
    let seqs: std::collections::BTreeSet<_> = rows.iter().map(|r| r.journal_seq).collect();
    assert_eq!(seqs.len(), 1);
    assert!(notes.join("\n").contains("name-resolution-free"));
    f.shutdown(Some(index)).await;
}

struct MoveFixture {
    lang: &'static str,
    seed: &'static [(&'static str, &'static str)],
    from: &'static str,
    to: &'static str,
    expect_after: &'static [(&'static str, &'static str)],
    expect_gone: &'static [(&'static str, &'static str)],
}

const MOVE_FIXTURES: [MoveFixture; 6] = [
    MoveFixture {
        lang: "typescript",
        seed: &[
            (
                "src/util/helper.ts",
                "export function helper(): number {\n  return 1;\n}\n",
            ),
            (
                "src/app.ts",
                "import { helper } from \"./util/helper\";\n\nexport function run(): number {\n  return helper();\n}\n",
            ),
            (
                "src/feature/main.ts",
                "import { helper } from \"../util/helper\";\n\nexport const value = helper();\n",
            ),
        ],
        from: "src/util/helper.ts",
        to: "src/core/helper.ts",
        expect_after: &[
            ("src/app.ts", "from \"./core/helper\""),
            ("src/feature/main.ts", "from \"../core/helper\""),
        ],
        expect_gone: &[
            ("src/app.ts", "util/helper"),
            ("src/feature/main.ts", "util/helper"),
        ],
    },
    MoveFixture {
        lang: "javascript",
        seed: &[
            ("lib/util.js", "module.exports = { util: () => 1 };\n"),
            (
                "lib/index.js",
                "const { util } = require(\"./util.js\");\n\nmodule.exports = { util };\n",
            ),
            (
                "app.js",
                "const { util } = require(\"./lib/util.js\");\n\nconsole.log(util());\n",
            ),
        ],
        from: "lib/util.js",
        to: "lib/nested/util.js",
        expect_after: &[
            ("lib/index.js", "require(\"./nested/util.js\")"),
            ("app.js", "require(\"./lib/nested/util.js\")"),
        ],
        expect_gone: &[
            ("lib/index.js", "\"./util.js\""),
            ("app.js", "\"./lib/util.js\""),
        ],
    },
    MoveFixture {
        lang: "python",
        seed: &[
            ("pkg/helper.py", "def helper():\n    return 1\n"),
            (
                "app.py",
                "from pkg.helper import helper\n\nprint(helper())\n",
            ),
            (
                "pkg/other.py",
                "import pkg.helper\n\nvalue = pkg.helper.helper()\n",
            ),
        ],
        from: "pkg/helper.py",
        to: "pkg/core/helper.py",
        expect_after: &[
            ("app.py", "from pkg.core.helper import helper"),
            ("pkg/other.py", "import pkg.core.helper"),
        ],
        expect_gone: &[
            ("app.py", "from pkg.helper import"),
            ("pkg/other.py", "import pkg.helper\n"),
        ],
    },
    MoveFixture {
        lang: "java",
        seed: &[
            (
                "com/example/Util.java",
                "package com.example;\n\npublic class Util {\n  public static int max() {\n    return 1;\n  }\n}\n",
            ),
            (
                "com/app/App.java",
                "package com.app;\n\nimport com.example.Util;\n\nclass App {\n  int run() {\n    return Util.max();\n  }\n}\n",
            ),
            (
                "com/app/Calc.java",
                "package com.app;\n\nimport static com.example.Util.max;\n\nclass Calc {\n  int c() {\n    return max();\n  }\n}\n",
            ),
        ],
        from: "com/example/Util.java",
        to: "com/core/Util.java",
        expect_after: &[
            ("com/app/App.java", "import com.core.Util;"),
            ("com/app/Calc.java", "import static com.core.Util.max;"),
        ],
        expect_gone: &[
            ("com/app/App.java", "com.example.Util"),
            ("com/app/Calc.java", "com.example.Util"),
        ],
    },
    MoveFixture {
        lang: "c",
        seed: &[
            ("include/util.h", "int util(void);\n"),
            (
                "src/main.c",
                "#include \"../include/util.h\"\n\nint main(void) {\n  return util();\n}\n",
            ),
            (
                "src/other.c",
                "#include \"include/util.h\"\n\nint other(void) {\n  return util();\n}\n",
            ),
        ],
        from: "include/util.h",
        to: "include/core/util.h",
        expect_after: &[
            ("src/main.c", "#include \"../include/core/util.h\""),
            ("src/other.c", "#include \"include/core/util.h\""),
        ],
        expect_gone: &[
            ("src/main.c", "\"../include/util.h\""),
            ("src/other.c", "\"include/util.h\""),
        ],
    },
    MoveFixture {
        lang: "cpp",
        seed: &[
            ("lib/vec.hpp", "struct Vec {\n  int x;\n};\n"),
            (
                "app/main.cpp",
                "#include \"../lib/vec.hpp\"\n\nint main() {\n  return Vec{1}.x;\n}\n",
            ),
        ],
        from: "lib/vec.hpp",
        to: "lib/math/vec.hpp",
        expect_after: &[("app/main.cpp", "#include \"../lib/math/vec.hpp\"")],
        expect_gone: &[("app/main.cpp", "\"../lib/vec.hpp\"")],
    },
];

#[tokio::test]
async fn import_aware_moves_across_six_fixture_languages() {
    for fixture in &MOVE_FIXTURES {
        let mut f = Fixture::new();
        f.seed(fixture.seed);
        let lang = fixture.lang;
        let (kind, _) = refused(
            f.commands(None, "suite")
                .files(&files(FilesOp::Move, Some(fixture.from), Some(fixture.to)))
                .await,
        );
        let ErrorKind::ConfirmRequired { confirm_token } = kind else {
            panic!("{lang}: gated");
        };
        assert!(
            f.read(fixture.from).is_some() && f.read(fixture.to).is_none(),
            "{lang}: nothing moved"
        );

        let before = f.read(fixture.from).expect("source");
        let (rows, _) = ok(f
            .commands(None, "suite")
            .files(&moved(fixture.from, fixture.to))
            .await);
        assert!(f.read(fixture.from).is_none(), "{lang}");
        assert_eq!(
            f.read(fixture.to).as_deref(),
            Some(before.as_str()),
            "{lang}"
        );
        for (path, needle) in fixture.expect_after {
            assert!(
                f.read(path).expect(path).contains(needle),
                "{lang}: {path} lacks {needle}"
            );
        }
        for (path, needle) in fixture.expect_gone {
            assert!(
                !f.read(path).expect(path).contains(needle),
                "{lang}: {path} still has {needle}"
            );
        }
        let seqs: std::collections::BTreeSet<_> = rows.iter().map(|r| r.journal_seq).collect();
        assert_eq!(seqs.len(), 1, "{lang}: one journal op for the batch");
        assert!(seqs.iter().next().expect("seq").is_some());

        // A token minted before the move no longer describes the workspace.
        let mut replay = files(FilesOp::Move, Some(fixture.from), Some(fixture.to));
        replay.confirm = Some(confirm_token);
        let (kind, _) = refused(f.commands(None, "suite").files(&replay).await);
        assert_eq!(kind, ErrorKind::FileNotFound, "{lang}");
        f.shutdown(None).await;
    }
}

#[tokio::test]
async fn move_flows_two_call_confirm_own_imports_broken_source_and_stale_anchor() {
    let fixture = &MOVE_FIXTURES[0];
    let mut f = Fixture::new();
    f.seed(fixture.seed);
    let (kind, notes) = refused(
        f.commands(None, "suite")
            .files(&files(FilesOp::Move, Some(fixture.from), Some(fixture.to)))
            .await,
    );
    let ErrorKind::ConfirmRequired { confirm_token } = kind else {
        panic!("gated");
    };
    assert!(
        notes.join("\n").contains("rewrote 2 import specifier(s)"),
        "{notes:?}"
    );
    let mut confirmed = files(FilesOp::Move, Some(fixture.from), Some(fixture.to));
    confirmed.confirm = Some(confirm_token);
    ok(f.commands(None, "suite").files(&confirmed).await);
    assert!(f.read(fixture.from).is_none() && f.read(fixture.to).is_some());

    f.seed(&[
        ("src/util/shared.ts", "export const shared = 1;\n"),
        (
            "src/util/helper.ts",
            "import { shared } from \"./shared\";\n\nexport const h = shared;\n",
        ),
    ]);
    ok(f.commands(None, "suite")
        .files(&moved("src/util/helper.ts", "src/core/helper2.ts"))
        .await);
    assert!(
        f.read("src/core/helper2.ts")
            .expect("moved")
            .contains("from \"../util/shared\"")
    );

    // Relocating an already-broken file applies with diagnostics, not PARSE_REJECTED.
    f.write(
        "src/broken.ts",
        "export function broken() {\n  return (1;\n}\n",
    );
    let (rows, _) = ok(f
        .commands(None, "suite")
        .files(&moved("src/broken.ts", "src/moved-broken.ts"))
        .await);
    let created = rows
        .iter()
        .find(|r| r.path == "src/moved-broken.ts")
        .expect("created row");
    assert!(!created.diagnostics.is_empty(), "{created:?}");
    assert!(created.applied);

    let mut stale = moved("src/core/helper.ts", "src/elsewhere/helper.ts");
    stale.expected_hash = Some(hash("what the agent read earlier"));
    let (kind, _) = refused(f.commands(None, "suite").files(&stale).await);
    assert_eq!(
        kind,
        ErrorKind::StaleAnchor {
            content_hash: hash(fixture.seed[0].1)
        }
    );
    f.shutdown(None).await;
}

#[tokio::test]
async fn deletes_are_recoverable_and_every_files_operation_is_journaled() {
    let mut f = Fixture::new();
    let content = "export const keep = 42;\n";
    f.write("src/keep.ts", content);
    let (rows, _) = ok(f
        .commands(None, "suite")
        .files(&files(FilesOp::Delete, Some("src/keep.ts"), None))
        .await);
    assert_eq!(rows[0].action, WriteAction::Deleted);
    assert!(f.read("src/keep.ts").is_none());
    let trash: Vec<_> = fs::read_dir(f.root.join(".workbench/trash"))
        .expect("trash")
        .collect();
    assert_eq!(trash.len(), 1, "nothing is unlinked");
    ok(f.commands(None, "suite").undo(&UndoInput::default()).await);
    assert_eq!(f.read("src/keep.ts").as_deref(), Some(content));

    f.write("src/a.ts", "export const a = 1;\n");
    ok(f.commands(None, "suite")
        .files(&files(FilesOp::Create, None, Some("src/new.ts")))
        .await);
    ok(f.commands(None, "suite")
        .files(&files(FilesOp::Mkdir, None, Some("src/dir")))
        .await);
    ok(f.commands(None, "suite")
        .files(&files(FilesOp::Copy, Some("src/a.ts"), Some("src/b.ts")))
        .await);
    ok(f.commands(None, "suite")
        .files(&files(FilesOp::Delete, Some("src/b.ts"), None))
        .await);
    ok(f.commands(None, "suite")
        .files(&moved("src/a.ts", "src/moved.ts"))
        .await);
    let ops = f.journal.list(0).expect("journal");
    let files_ops: Vec<_> = ops.iter().filter(|op| op.tool == "files").collect();
    assert_eq!(files_ops.len(), 6);
    assert!(files_ops.iter().all(|op| op.session_id == "suite"));
    let kinds: Vec<Vec<String>> = files_ops[1..]
        .iter()
        .map(|op| {
            let mut kinds: Vec<String> = op
                .files
                .iter()
                .map(|file| format!("{:?}:{}", file.kind, file.path).to_lowercase())
                .collect();
            kinds.sort();
            kinds
        })
        .collect();
    assert_eq!(kinds[0], vec!["create:src/new.ts"]);
    assert_eq!(kinds[1], vec!["create:src/dir/.gitkeep"]);
    assert_eq!(kinds[2], vec!["create:src/b.ts"]);
    assert_eq!(kinds[3], vec!["delete:src/b.ts"]);
    assert_eq!(kinds[4], vec!["create:src/moved.ts", "delete:src/a.ts"]);
    f.shutdown(None).await;
}

const RUST_FIXTURE: [(&str, &str); 8] = [
    (
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    ),
    (
        "src/lib.rs",
        "//! Fixture crate.\n\npub mod a;\npub mod c;\npub mod util;\n\npub use a::b::helper as root_helper;\n\npub fn run() -> u32 {\n    crate::a::b::helper() + a::b::helper() + c::from_c()\n}\n",
    ),
    (
        "src/a.rs",
        "pub mod b;\npub mod other;\n\npub fn sibling() -> u32 {\n    1\n}\n\npub fn via_self() -> u32 {\n    self::b::helper()\n}\n\npub fn via_uniform() -> u32 {\n    b::helper()\n}\n",
    ),
    (
        "src/a/b.rs",
        "use super::sibling;\n\n/// Helper.\npub fn helper() -> u32 {\n    crate::util::one() + sibling() + super::sibling()\n}\n",
    ),
    (
        "src/a/other.rs",
        "pub fn twice() -> u32 {\n    super::b::helper() * 2\n}\n",
    ),
    ("src/util.rs", "pub fn one() -> u32 {\n    1\n}\n"),
    (
        "src/c.rs",
        "pub mod d;\n\npub fn from_c() -> u32 {\n    crate::a::b::helper() + d::dd()\n}\n",
    ),
    (
        "src/c/d.rs",
        "pub fn dd() -> u32 {\n    super::super::a::b::helper()\n}\n",
    ),
];

fn cargo_check(root: &std::path::Path) -> std::process::Output {
    std::process::Command::new("cargo")
        .args(["check", "--offline", "--quiet"])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("cargo on PATH")
}

#[tokio::test]
async fn rust_moves_rewrite_module_paths_through_the_files_verb_and_compile() {
    let mut f = Fixture::new();
    f.seed(&RUST_FIXTURE);
    let (kind, notes) = refused(
        f.commands(None, "suite")
            .files(&files(
                FilesOp::Move,
                Some("src/a/b.rs"),
                Some("src/a/bee.rs"),
            ))
            .await,
    );
    let ErrorKind::ConfirmRequired { confirm_token } = kind else {
        panic!("gated");
    };
    let joined = notes.join("\n");
    assert!(
        joined.contains("rust: module crate::a::b → crate::a::bee"),
        "{joined}"
    );
    assert!(
        joined.contains("src/a.rs") && joined.contains("src/c/d.rs"),
        "{joined}"
    );
    assert_eq!(f.read("src/a.rs").as_deref(), Some(RUST_FIXTURE[2].1));

    let mut preview = files(FilesOp::Move, Some("src/a/b.rs"), Some("src/c/b.rs"));
    preview.dry_run = true;
    let (rows, _) = ok(f.commands(None, "suite").files(&preview).await);
    assert!(rows.len() >= 7, "{rows:?}");
    assert!(rows.iter().all(|r| !r.applied));
    assert_eq!(f.read("src/c.rs").as_deref(), Some(RUST_FIXTURE[6].1));
    assert!(f.journal.list(0).expect("journal").is_empty());

    let mut confirmed = files(FilesOp::Move, Some("src/a/b.rs"), Some("src/a/bee.rs"));
    confirmed.confirm = Some(confirm_token);
    ok(f.commands(None, "suite").files(&confirmed).await);
    assert!(f.read("src/a/b.rs").is_none());
    assert_eq!(f.read("src/a/bee.rs").as_deref(), Some(RUST_FIXTURE[3].1));
    assert!(f.read("src/a.rs").expect("a").contains("pub mod bee;"));
    assert!(
        f.read("src/lib.rs")
            .expect("lib")
            .contains("crate::a::bee::helper()")
    );
    assert!(
        f.read("src/c/d.rs")
            .expect("d")
            .contains("super::super::a::bee::helper()")
    );
    let check = cargo_check(&f.root);
    assert!(
        check.status.success(),
        "rename in place must compile: {}",
        String::from_utf8_lossy(&check.stderr)
    );

    // Move under another parent: declaration moves, super:: re-roots, manifest follows.
    let mut g = Fixture::new();
    g.seed(&RUST_FIXTURE);
    g.write("Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"cli\"\npath = \"src/tools/cli.rs\"\n");
    g.write("src/tools/cli.rs", "fn main() {}\n");
    ok(g.commands(None, "suite")
        .files(&moved("src/a/b.rs", "src/c/b.rs"))
        .await);
    assert!(!g.read("src/a.rs").expect("a").contains("mod b;"));
    // rustfmt (reorder_modules) formats the parent after the declaration lands.
    let c = g.read("src/c.rs").expect("c");
    assert!(
        c.contains("pub mod b;\n") && c.contains("pub mod d;\n"),
        "{c}"
    );
    assert!(
        g.read("src/c/b.rs")
            .expect("b")
            .contains("use crate::a::sibling;")
    );
    let check = cargo_check(&g.root);
    assert!(
        check.status.success(),
        "move under another parent must compile: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    ok(g.commands(None, "suite")
        .files(&moved("src/tools/cli.rs", "src/bin/cli.rs"))
        .await);
    let manifest = g.read("Cargo.toml").expect("manifest");
    assert!(manifest.contains("path = \"src/bin/cli.rs\""));
    assert!(toml::from_str::<toml::Value>(&manifest).is_ok());

    // Negative shapes surface their notes and the move still proceeds as a plain move.
    let mut h = Fixture::new();
    h.seed(&RUST_FIXTURE);
    h.write(
        "src/lib.rs",
        format!("{}use crate::a::{{b, other}};\n", RUST_FIXTURE[1].1),
    );
    let (_, notes) = refused(
        h.commands(None, "suite")
            .files(&files(
                FilesOp::Move,
                Some("src/a/b.rs"),
                Some("src/a/bee.rs"),
            ))
            .await,
    );
    assert!(notes.join("\n").contains("not rewritten: src/lib.rs:12 — use-tree member names the moved module (use crate::a::{b, other}); split it by hand"), "{notes:?}");
    let (_, notes) = ok(h
        .commands(None, "suite")
        .files(&moved("src/a.rs", "src/aa.rs"))
        .await);
    assert!(
        notes
            .join("\n")
            .contains("has child modules (b.rs, other.rs)")
    );
    assert!(h.read("src/aa.rs").is_some());

    // The lightweight retained shape: lib.rs + core.rs + helper.rs, go untouched.
    let mut k = Fixture::new();
    k.seed(&[
        ("src/helper.rs", "pub fn helper() -> u32 {\n    1\n}\n"),
        ("src/core.rs", "pub fn core() -> u32 {\n    2\n}\n"),
        (
            "src/lib.rs",
            "mod core;\nmod helper;\n\nuse crate::helper::helper;\n",
        ),
        (
            "pkg/util.go",
            "package util\n\nfunc Util() int {\n\treturn 1\n}\n",
        ),
        (
            "cmd/main.go",
            "package main\n\nimport \"example.com/pkg\"\n\nfunc main() {}\n",
        ),
    ]);
    let (_, notes) = ok(k
        .commands(None, "suite")
        .files(&moved("src/helper.rs", "src/core/helper.rs"))
        .await);
    assert_eq!(
        k.read("src/lib.rs").as_deref(),
        Some("mod core;\n\nuse crate::core::helper::helper;\n")
    );
    assert_eq!(
        k.read("src/core.rs").as_deref(),
        Some("mod helper;\n\npub fn core() -> u32 {\n    2\n}\n")
    );
    assert!(
        notes
            .join("\n")
            .contains("rust: module crate::helper → crate::core::helper")
    );
    let (_, notes) = ok(k
        .commands(None, "suite")
        .files(&moved("pkg/util.go", "pkg/other/util.go"))
        .await);
    assert!(
        k.read("cmd/main.go")
            .expect("go")
            .contains("import \"example.com/pkg\"")
    );
    let go_notes = notes.join("\n");
    assert!(go_notes.contains("go imports name packages, not files, and are not rewritten"));
    assert!(!go_notes.to_lowercase().contains("rust"));
    for fixture in [f, g, h, k] {
        fixture.shutdown(None).await;
    }
}

#[tokio::test]
async fn rustfmt_formats_edits_and_exact_undo_restores_the_pre_edit_bytes() {
    let mut f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"fmt-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    f.write("rustfmt.toml", "tab_spaces = 2\n");
    let before = "pub fn value() -> usize { 1 }\n";
    f.write("src/lib.rs", before);
    let mut replace = edit(EditOp::ReplaceText, "src/lib.rs");
    replace.old = Some("{ 1 }".into());
    replace.new = Some("{let value=2;value}".into());
    let (rows, _) = ok(f.commands(None, "s").edit(&replace).await);
    assert_eq!(rows[0].formatting, "formatted and reverified");
    assert_eq!(
        f.read("src/lib.rs").as_deref(),
        Some("pub fn value() -> usize {\n  let value = 2;\n  value\n}\n")
    );
    let (rows, _) = ok(f.commands(None, "s").undo(&UndoInput::default()).await);
    let UndoRow::Write(report) = &rows[0] else {
        panic!("write row: {rows:?}");
    };
    assert_eq!(report.formatting, "disabled: exact bytes");
    assert_eq!(f.read("src/lib.rs").as_deref(), Some(before));
    f.shutdown(None).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_partially_failing_batch_compensates_and_reports_every_file_effect() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = Fixture::new();
    f.write("src/a.ts", "export const a = 1;\n");
    f.write("zz_locked/b.ts", "export const b = 1;\n");
    let index = f.warm_index().await;
    fs::set_permissions(f.root.join("zz_locked"), fs::Permissions::from_mode(0o500))
        .expect("read-only directory");
    let mut rewrite = EditInput::new(EditOp::Rewrite);
    rewrite.pattern = Some("1".into());
    rewrite.rewrite_pattern = Some("2".into());
    rewrite.auto_apply = true;
    let (kind, notes) = refused(f.commands(Some(&index), "s").edit(&rewrite).await);
    fs::set_permissions(f.root.join("zz_locked"), fs::Permissions::from_mode(0o700))
        .expect("restore mode");
    assert_eq!(kind, ErrorKind::EngineUnavailable, "{notes:?}");
    let joined = notes.join("\n");
    assert!(joined.contains("src/a.ts: rolled_back"), "{joined}");
    assert!(joined.contains("zz_locked/b.ts: not_applied"), "{joined}");
    assert!(joined.contains("rolled back"), "{joined}");
    assert_eq!(f.read("src/a.ts").as_deref(), Some("export const a = 1;\n"));
    assert_eq!(
        f.read("zz_locked/b.ts").as_deref(),
        Some("export const b = 1;\n")
    );
    let ops = f.journal.list(0).expect("journal");
    assert_eq!(ops.len(), 1);
    assert_eq!(
        f.journal.write_outcome(ops[0].seq).expect("outcome"),
        Some(JournalWriteOutcome::RolledBack)
    );
    assert!(
        f.journal
            .undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
            .is_empty()
    );
    f.shutdown(Some(index)).await;
}

#[tokio::test]
async fn undo_after_external_edits_surfaces_conflicts_forces_and_refuses_broken_restorations() {
    let mut f = Fixture::new();
    f.write("src/a.ts", "const a = 1;\n");
    let mut replace = edit(EditOp::ReplaceText, "src/a.ts");
    replace.old = Some("1".into());
    replace.new = Some("2".into());
    ok(f.commands(None, "s").edit(&replace).await);
    f.write("src/a.ts", "const a = 3;\n");
    let (rows, _) = ok(f.commands(None, "s").undo(&UndoInput::default()).await);
    let UndoRow::Conflict {
        expected_hash,
        current_hash,
        restore_hash,
        ..
    } = &rows[0]
    else {
        panic!("conflict: {rows:?}");
    };
    assert_eq!(expected_hash, &hash("const a = 2;\n"));
    assert_eq!(current_hash, &hash("const a = 3;\n"));
    assert_eq!(restore_hash, &hash("const a = 1;\n"));
    let force = UndoInput {
        scope: UndoScope::Session,
        on_conflict: UndoConflictPolicy::Force,
        ..UndoInput::default()
    };
    ok(f.commands(None, "s").undo(&force).await);
    assert_eq!(f.read("src/a.ts").as_deref(), Some("const a = 1;\n"));

    // Restoring broken bytes into a now-valid file is refused; the op stays undoable.
    f.write(
        "src/broken.ts",
        "export function broken() {\n  return (1;\n}\n",
    );
    let mut fix = edit(EditOp::ReplaceText, "src/broken.ts");
    fix.old = Some("(1;".into());
    fix.new = Some("1;".into());
    let (rows, _) = ok(f.commands(None, "s").edit(&fix).await);
    assert!(rows[0].applied);
    let (kind, notes) = refused(f.commands(None, "s").undo(&UndoInput::default()).await);
    assert!(matches!(kind, ErrorKind::ParseRejected { .. }), "{kind:?}");
    assert!(notes.join("\n").contains("remain undoable"));
    assert_eq!(
        f.read("src/broken.ts").as_deref(),
        Some("export function broken() {\n  return 1;\n}\n")
    );
    let candidates = f
        .journal
        .undo_candidates(&UndoCandidateQuery {
            session_id: Some("s".into()),
            ..Default::default()
        })
        .expect("candidates");
    assert_eq!(candidates.len(), 1);
    f.shutdown(None).await;
}

#[tokio::test]
async fn mixed_create_delete_write_batches_ride_one_journal_op_and_undo_together() {
    let mut f = Fixture::new();
    f.seed(&[
        (
            "src/util/helper.ts",
            "import { shared } from \"./shared\";\n\nexport const h = shared;\n",
        ),
        ("src/util/shared.ts", "export const shared = 1;\n"),
        (
            "src/app.ts",
            "import { h } from \"./util/helper\";\n\nexport const app = h;\n",
        ),
    ]);
    let (rows, _) = ok(f
        .commands(None, "s")
        .files(&moved("src/util/helper.ts", "src/core/helper.ts"))
        .await);
    let mut actions: BTreeMap<&str, WriteAction> = BTreeMap::new();
    for row in &rows {
        actions.insert(row.path.as_str(), row.action);
    }
    assert_eq!(
        actions.get("src/core/helper.ts"),
        Some(&WriteAction::Created)
    );
    assert_eq!(
        actions.get("src/util/helper.ts"),
        Some(&WriteAction::Deleted)
    );
    assert_eq!(actions.get("src/app.ts"), Some(&WriteAction::Modified));
    let seqs: std::collections::BTreeSet<_> = rows.iter().map(|r| r.journal_seq).collect();
    assert_eq!(seqs.len(), 1);
    let (rows, _) = ok(f.commands(None, "s").undo(&UndoInput::default()).await);
    let restored: Vec<&WriteReport> = rows
        .iter()
        .filter_map(|row| match row {
            UndoRow::Write(report) => Some(report),
            _ => None,
        })
        .collect();
    assert_eq!(restored.len(), 3);
    assert!(f.read("src/core/helper.ts").is_none());
    assert!(
        f.read("src/util/helper.ts")
            .expect("restored")
            .contains("./shared")
    );
    assert!(
        f.read("src/app.ts")
            .expect("restored")
            .contains("./util/helper")
    );
    f.shutdown(None).await;
}
