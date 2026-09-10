#!/usr/bin/env bash
# native-write-path-gate.sh — one native write path (arch-12)
#
# Every supported native workflow mutation runs one implementation: the
# think-and-ship tool handler behind the MCP verb, reached by the agent over
# the wire and by Magistr's own surfaces through `NativeEngines`. This gate
# keeps that executable rather than documented:
#
#   1. Engines and family services are composed only in magistr-engine's
#      `tas` module. No production source elsewhere names a think-and-ship
#      engine or service type, `UnifiedService::new` or `open_engines`.
#   2. Inside `tas`, an engine lock is taken only by the writer
#      (native_engines.rs) and the reader (native_reader.rs).
#   3. The direct engine mutations — the documented exceptions — are exactly
#      `record_check_full` (the recording half of `ship_check`, whose
#      handler executes the command itself) and `propose_title` /
#      `propose_status` (tracker-mirror verbs with no MCP tool), each once,
#      in native_engines.rs.
#   4. The MCP transport never writes through its own wire: nothing under
#      mcp_tools constructs a tool call to itself.
#   5. Only magistr-engine and magistr-cli depend on think-and-ship directly
#      (the CLI for wire types and the served UnifiedService, never engines).
#
# Doc comments are not code: rule 1 skips comment lines.
#
# LINT_PROJECT_ROOT overrides the project root (rule 5 needs a real
# workspace and is skipped under an override).

set -euo pipefail

PROJECT_ROOT="${LINT_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "$PROJECT_ROOT"

errors=0
fail() {
    echo "ERROR: $*"
    errors=$((errors + 1))
}

production_sources() {
    find crates -path '*/src/*' -name '*.rs' -not -name '*.tests.rs' | sort
}

# --- Rule 1: composition only in magistr-engine/src/tas -----------------------
names='\b(RoadmapEngine|ShipEngine|SignalEngine|ReasoningServer|ThinkService|ShipService|RoadmapService|SignalService)\b|UnifiedService::new|open_engines\('
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/tas/*) continue ;;
    esac
    hits=$(grep -nE "$names" "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f composes or names a think-and-ship engine or service outside magistr-engine/src/tas:"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 2: engine locks only in the writer and the reader -------------------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/tas/native_engines.rs | crates/magistr-engine/src/tas/native_reader.rs) continue ;;
        crates/magistr-engine/src/tas/*) ;;
        *) continue ;;
    esac
    hits=$(grep -nE 'roadmap_engine\(\)|ship_engine\(\)|think_engine\(\)|signal_engine\(\)|lock\(&self\.(roadmap|ship|think|signal)\)' "$f" || true)
    if [[ -n "$hits" ]]; then
        fail "$f takes an engine lock outside the writer and the reader:"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 3: the documented direct mutations, once each, in the writer --------
for verb in record_check_full propose_title propose_status; do
    sites=$(production_sources | xargs grep -lE "\.${verb}\(" || true)
    if [[ "$sites" != "crates/magistr-engine/src/tas/native_engines.rs" ]]; then
        fail "\`.${verb}(\` must be called only from native_engines.rs; found in: ${sites:-nowhere}"
    fi
    count=$(grep -cE "\.${verb}\(" crates/magistr-engine/src/tas/native_engines.rs || true)
    if [[ "$count" -ne 1 ]]; then
        fail "\`.${verb}(\` must appear exactly once in native_engines.rs (found $count)"
    fi
done

# --- Rule 4: the transport never calls its own wire ---------------------------
hits=$(grep -nE 'CallToolRequestParams::new\(' crates/magistr-cli/src/mcp_tools/*.rs 2>/dev/null | grep -v '\.tests\.rs' || true)
if [[ -n "$hits" ]]; then
    fail "mcp_tools constructs a tool call to its own wire — act through NativeEngines instead:"
    echo "$hits"
fi

# --- Rule 5: direct dependents of think-and-ship ------------------------------
if [[ -z "${LINT_PROJECT_ROOT:-}" ]]; then
    dependents=$(cargo tree -i think-and-ship --depth 1 --prefix none 2>/dev/null \
        | awk 'NR > 1 { print $1 }' | sort | tr '\n' ' ')
    if [[ "$dependents" != "magistr-cli magistr-engine " ]]; then
        fail "think-and-ship direct dependents must be exactly magistr-cli and magistr-engine; found: ${dependents:-none}"
    fi
fi

if [[ "$errors" -eq 0 ]]; then
    echo "OK: one native write path — engines composed in magistr-engine/tas, verbs through the library handlers, the transport never writes through its own wire."
    exit 0
fi
echo "native-write-path-gate: $errors error(s)"
exit 1
