#!/usr/bin/env bash
# native-write-path-gate.sh — one native write path (arch-12)
#
# Native mutations run typed first-party application use cases under Magistr's
# admitted project capability. MCP and owner protocols serialize at their edge.
#
# 1. Magistr composes workflow engines/services only in magistr-engine/tas.
# 2. Native readers alone inspect engine locks; writers use application methods.
# 3. Native writes contain no MCP roundtrip, one-poll future or raw check append.
# 4. MCP never constructs a call to its own wire.
# 5. Only magistr-engine and magistr-cli depend directly on the workflow crate.
# 6. First-party application modules contain no MCP dependency.
# 7. Check admission and settlement run only through the composition
#    boundary (magistr-cli/src/application) or the writer itself (arch-14f).
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
names='\b(RoadmapEngine|ShipEngine|SignalEngine|ReasoningServer|ThinkService|ShipService|RoadmapService|SignalService|ThinkUseCases|ShipUseCases|RoadmapUseCases|SignalUseCases)\b|UnifiedService::new|open_engines\('
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

# --- Rule 2: only native readers inspect raw engines ------------------------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/tas/native_reader.rs) continue ;;
        crates/magistr-engine/src/tas/*) ;;
        *) continue ;;
    esac
    hits=$(grep -nE 'roadmap_engine\(\)|ship_engine\(\)|think_engine\(\)|signal_engine\(\)|lock\(&self\.(roadmap|ship|think|signal)\)' "$f" || true)
    if [[ -n "$hits" ]]; then
        fail "$f takes an engine lock outside the native reader:"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 3: typed native writes, no handler trampoline ----------------------
writer=crates/magistr-engine/src/tas/native_engines.rs
hits=$(grep -nE 'Parameters|CallToolResult|::mcp::|serde_json::from_|Waker::noop|handler_call|\.record_check_full\(' "$writer" | grep -vE '^[0-9]+:\s*//' || true)
if [[ -n "$hits" ]]; then
    fail "native writer bypasses typed application use cases:"
    echo "$hits"
fi
if [[ -e crates/magistr-engine/src/tas/handler_call.rs ]]; then
    fail "the one-poll MCP handler trampoline must not exist"
fi

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

# --- Rule 6: the owned application boundary has no MCP dependency -----------
application=subsystems/workflow/crates/think-and-ship/src
while IFS= read -r f; do
    hits=$(grep -nE '(^|[^[:alnum:]_])(rmcp|CallToolResult|Parameters)|::mcp::|Waker::noop' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f couples a native application use case to MCP:"
        echo "$hits"
    fi
done < <(find "$application" -path '*/application/*' -name '*.rs' -not -name '*.tests.rs' | sort)

# --- Rule 7: check admission/settlement only through the boundary -----------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/tas/*) continue ;;
        crates/magistr-cli/src/application/*) continue ;;
    esac
    hits=$(grep -nE '\.(ship_admit_gate|ship_settle_gate|ship_record_gate)\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f admits or settles a check outside the composition boundary (use magistr::application::AdmittedCheck):"
        echo "$hits"
    fi
done < <(production_sources)

if [[ "$errors" -eq 0 ]]; then
    echo "OK: one native write path — engines composed in magistr-engine/tas, typed application verbs beneath transports, checks admitted through the composition boundary, no MCP roundtrips."
    exit 0
fi
echo "native-write-path-gate: $errors error(s)"
exit 1
