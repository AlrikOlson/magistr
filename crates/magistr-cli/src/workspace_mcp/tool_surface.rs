//! The declared tool surface: names, titles and descriptions as the retained
//! server declared them, input schemas verbatim from the native copies of
//! the reference schemas, and the envelope as every tool's output schema.

use std::sync::Arc;

use magistr_workspace::{extensions::PluginToolListing, service::ToolSchemas};
use rmcp::model::{JsonObject, Tool};

/// The instructions a client reads at `initialize`.
pub(crate) const INSTRUCTIONS: &str = "magistr workspace — the native workspace tools for one \
workspace. search, map and read answer over the live tree and the derived index; edit, files, \
undo and script mutate through the verified write pipeline behind the confirm gate; run \
executes one declared command with no shell. Every result is a versioned ResponseEnvelope \
(text and structuredContent carry the same sealed JSON); degradations ride its `notes`.";

const STATUS: &str = "Workspace health: root, SQLite journal mode (WAL, or TRUNCATE when the \
workspace sits on a network filesystem), and the supported envelope versions. Every result is a \
versioned ResponseEnvelope; degradations ride its `notes`.";

const MAP: &str = "Workspace orientation — replaces ls -R and orientation greps (SDD §9.2). A \
pruned directory tree (Tier-0 capable, no index required), per-file symbol outlines when the \
Tier-1 index is up (detail: skeleton | outline | full), index health, a language census, and \
the installed-plugin inventory with trust labels (sideloads are always untrusted). The overview \
row carries a short ttlMs keyed to the index generation for client caching.";

const READ: &str = "Budgeted, enveloped file reading — replaces cat, head, tail and sed -n (SDD \
§9.3). Reads the whole file, an explicit lines: {start, end} range, or a symbol resolved via the \
Tier-1 index (surround: none | enclosing | imports). The unit header carries precise line \
anchors and a sha256 content hash for edit's staleness anchor; content is mtime+size-verified \
before return and truncation always carries a manifest with a line cursor.";

const SEARCH: &str = "Ranked, grouped search — replaces grep, glob and find (SDD §9.1). Modes: \
auto (runs lexical), lexical (regex over live file content), structural (ast-grep pattern with \
$VAR metavariables, hits carry node kinds), references (symbol edges from the warm index, each \
with confidence provenance), files (glob/filename). semantic is reserved and rejected with a \
typed error. Results are grouped by file, definitions first, junk and tests demoted; truncation \
always carries a manifest.";

const EDIT: &str = "Verified structural editing — replaces sed, awk and patch (SDD §9.4). Symbol \
verbs (replace_body | insert_after | insert_before | rename) address code via the Tier-1 index; \
rewrite applies an ast-grep pattern across a glob scope; replace_text is the anchored literal \
fallback. Every verb runs the §7.5 write engine: a new syntax error rejects the whole call \
(PARSE_REJECTED with the parser message and rejected diff), applied diffs are the receipt, and \
read's contentHash passed as expectedHash turns a changed file into STALE_ANCHOR with the fresh \
hash. Single-file symbol/text edits auto-apply; rewrite, rename and anything touching >1 file or \
>200 changed lines is confirm-gated: MRTR clients approve the aggregated diff in-call via \
elicitation, everyone else gets CONFIRM_REQUIRED carrying a confirmToken and re-invokes the \
identical call with confirm to apply. A stale confirm answers STALE_ANCHOR, a forged or \
mismatched one CONFIRM_MISMATCH.";

const FILES: &str = "Typed file operations — replaces mv, cp, mkdir, touch and rm (SDD §9.5). \
move is import-aware: import specifiers that resolve to the moved file are rewritten across the \
workspace (typescript, javascript, python, java, c, c++ — specifier-based, name-resolution-free, \
the notes say exactly what was rewritten) and the moved file's own relative imports are \
recomputed; inherently multi-file, so always confirm-gated (MRTR elicitation in-call, or \
CONFIRM_REQUIRED + confirmToken as two calls). delete is a journal-backed move to \
.workbench/trash/ — recoverable with undo, nothing is unlinked destructively. create is touch \
(never overwrites); mkdir materializes the directory as a journaled .gitkeep. Every verb runs \
the §7.5 write engine: dry-run diff, parse-verify, format, journal, atomic swap; read's \
contentHash passed as expectedHash turns a changed source into STALE_ANCHOR.";

const UNDO: &str = "The journal, read and rewound (SDD §9.8, §7.6). action \"list\" reads the \
durable per-workspace op log — newest op first, one row per file an op touched, each carrying \
the op seq, its added/removed counts and its unified diff — and writes nothing. action \
\"rewind\" (the default) rewinds whole ops (write batches): the newest `steps`, or exactly the \
ops named by `ops` (the seqs list reports). Rewind is session-scoped by default — only the \
calling session's ops (identity via _meta workbench/sessionId) unless scope is \"workspace\". \
Conflicts are detected by content hash and surfaced in the envelope as a three-way choice \
(keepCurrent | force | merge by hand), never silently clobbered. Restorations run through the \
verified write pipeline with staleness anchors.";

const SCRIPT: &str = "Typed Code Mode for the long tail — replaces ad-hoc bash pipelines (SDD \
§9.7). The source runs as a JavaScript function body in a QuickJS-ng-on-WASM guest under the \
Wasmtime host: fuel-metered, memory-capped, no ambient filesystem, network, or shell. The one \
global is `workspace`: search(args) and read(args) answer the same envelopes as the tools, \
glob(pattern) lists matching workspace paths — all live and jail-checked — while \
workspace.edit.queue({file, old, new, occurrence?}) and workspace.files.queue({op: \
create|mkdir|delete, ...}) only QUEUE writes. Use `return` for the JSON-serializable result. \
Queued writes resolve through the §7.5 write engine and are always confirm-gated: dryRun:true \
(default) returns the value plus every dry-run diff and never applies; dryRun:false answers \
CONFIRM_REQUIRED with a confirmToken and re-invoking the identical call with confirm applies. \
Abuse dies typed: SANDBOX_FUEL_EXCEEDED, SANDBOX_MEMORY_EXCEEDED, SANDBOX_CAP_DENIED.";

const RUN: &str = "The declared-command escape hatch (SDD §9.6). Executes one command from the \
merged config's [commands] table as a single argv — no shell, ever. The allowlist is seeded at \
init from lockfile/manifest detection, amendable only by a human (workbench.toml is a protected \
path), and governable by org policy (§7.2). Known runners (bun/jest/vitest/pytest/cargo \
test/cargo nextest/go) answer structured failures — test name, file, assertion, trimmed trace — \
via bundled runner adapters; cargo build/check/clippy (and a test run that failed to compile) \
answer a diagnostics row — level, code, message, file:line:column, help — errors first, trimmed \
to the budget with a manifest; unknown commands answer head+tail with a truncation manifest. An \
undeclared command answers COMMAND_NOT_DECLARED (declare it in your config layer); text/file \
munging or shell attempts answer RUN_REDIRECTED — use search/read to inspect and edit to modify.";

/// The retained envelope, every tool's output schema.
pub(crate) fn envelope_schema() -> Arc<JsonObject> {
    Arc::new(
        ToolSchemas::envelope_value()
            .as_object()
            .cloned()
            .unwrap_or_default(),
    )
}

fn schema_of(tool: &str) -> Arc<JsonObject> {
    Arc::new(
        ToolSchemas::input_value(tool)
            .and_then(|schema| schema.as_object().cloned())
            .unwrap_or_default(),
    )
}

fn tool(
    name: &'static str,
    title: &str,
    description: &'static str,
    input: Arc<JsonObject>,
) -> Tool {
    let mut tool = Tool::new(name, description, input);
    tool.title = Some(title.to_owned());
    tool.output_schema = Some(envelope_schema());
    tool
}

/// The core surface in the retained order: status, map, read, search, the
/// write path, script when an interpreter is composed, then run.
pub(crate) fn core_tools(offers_script: bool) -> Vec<Tool> {
    let mut status = Tool::new(
        "workbench_status",
        STATUS,
        Arc::new(
            serde_json::json!({"type": "object", "properties": {}})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        ),
    );
    status.title = Some("Workbench status".to_owned());
    status.output_schema = Some(envelope_schema());
    let mut tools = vec![
        status,
        tool("map", "Map the workspace", MAP, schema_of("map")),
        tool("read", "Read a file", READ, schema_of("read")),
        tool(
            "search",
            "Search the workspace",
            SEARCH,
            schema_of("search"),
        ),
        tool("edit", "Edit the workspace", EDIT, schema_of("edit")),
        tool("files", "File operations", FILES, schema_of("files")),
        tool(
            "undo",
            "Read and rewind journaled ops",
            UNDO,
            schema_of("undo"),
        ),
    ];
    if offers_script {
        tools.push(tool(
            "script",
            "Run a sandboxed script",
            SCRIPT,
            schema_of("script"),
        ));
    }
    tools.push(tool(
        "run",
        "Run a declared project command",
        RUN,
        schema_of("run"),
    ));
    tools
}

/// A plugin tool under its derived namespaced name. Plugin payloads are
/// envelope-versioned JSON by decision: the whole argument object is the
/// payload, so the wire schema is deliberately open and the host wraps
/// every output in the envelope.
pub(crate) fn plugin_tool(listing: PluginToolListing) -> Tool {
    let mut tool = Tool::new(
        listing.name,
        listing.description,
        Arc::new(
            serde_json::json!({"type": "object", "additionalProperties": true})
                .as_object()
                .cloned()
                .unwrap_or_default(),
        ),
    );
    tool.title = Some(listing.title);
    tool.output_schema = Some(envelope_schema());
    tool
}
