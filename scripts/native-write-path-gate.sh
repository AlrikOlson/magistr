#!/usr/bin/env bash
# native-write-path-gate.sh — one native write path (arch-12)
#
# Native mutations run typed first-party application use cases under Magistr's
# admitted project capability. MCP and owner protocols serialize at their edge.
#
# 1. Magistr composes workflow engines/services only in magistr-engine/native
#    (the workflow crate itself, crates/magistr-workflow, is what gets composed).
# 2. Native readers alone inspect engine locks; writers use application methods.
# 3. Native writes contain no MCP roundtrip, one-poll future or raw check append.
# 4. MCP never constructs a call to its own wire.
# 5. Only magistr-engine and magistr-cli depend directly on the workflow crate.
# 6. First-party application modules contain no MCP dependency.
# 7. Check admission and settlement run only through the composition
#    boundary (magistr-cli/src/application) or the writer itself (arch-14f).
# 8. Every production writer admission carries the verification evidence
#    capability: a file that admits a ProjectAuthority builds its config
#    through magistr_core::evidence::writer_config, except the composition
#    of that capability itself, and the MCP service and the cockpit client
#    (which each refuse a config without it before taking the lease) (arch-14-verification-receipts). This
#    rule is supplementary: the ship family is the roadmap's completion
#    authority by construction and refuses every completion without the
#    capability (audit-260912-05); the rule keeps a production writer
#    from being admitted into that refusal.
# 9. WorkspaceService opens only on the two owner paths the client
#    inventory names: the workspace bridge (attach-or-admit) and the
#    run/loop host's HostedWorkspace (arch-14f3).
# 10. ProjectAuthority is admitted only at the inventory's admission
#    sites: the MCP service, admit_native_host, the loop's auto_native and
#    the cockpit host (arch-14f3).
# 11. Owner attachment happens only on the inventory's attached surfaces:
#    the second cockpit, the second mcp-serve proxy and the workspace
#    bridge (arch-14f3).
# 12. The ACP host writes no source file itself: a session's source writes
#    take the host's SourceRoute (the admitted workspace's pipeline) or are
#    refused, so magistr-acp production sources contain no direct file
#    write or directory creation (audit-260912-04).
# 13. Administrative writes are explicit: under crates/magistr-cli/src/scaffold
#    every planned change is written by the administrative service
#    (admin_service.rs) and nowhere else. The other files allowed to touch
#    the filesystem are the ownership manifest it saves, the explicit reset
#    (remove_scaffold.rs, owned_removal.rs), the legacy link migration
#    (codex_skill_links.rs), the post-generation enrichments of a fresh
#    project.json/CLAUDE.md (pipeline.rs, backlog
#    setup-plan-post-generate-enrichments), the CI workflow writer (ci.rs,
#    backlog ci-workflow-through-admin-plan), the legacy YAML roadmap seed
#    (generate.rs) and the interview trace log (trace.rs)
#    (audit-260912-16-admin-apply).
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

# --- Rule 1: composition only in magistr-engine/src/native -----------------------
# The workflow crate is the thing being composed; the rule is about Magistr.
names='\b(RoadmapEngine|ShipEngine|SignalEngine|ReasoningServer|ThinkService|ShipService|RoadmapService|SignalService|ThinkUseCases|ShipUseCases|RoadmapUseCases|SignalUseCases)\b|UnifiedService::new|open_engines\('
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/native/*) continue ;;
        crates/magistr-workflow/src/*) continue ;;
    esac
    hits=$(grep -nE "$names" "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f composes or names a workflow engine or service outside magistr-engine/src/native:"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 2: only native readers inspect raw engines ------------------------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/native/native_reader.rs) continue ;;
        crates/magistr-engine/src/native/*) ;;
        *) continue ;;
    esac
    hits=$(grep -nE 'roadmap_engine\(\)|ship_engine\(\)|think_engine\(\)|signal_engine\(\)|lock\(&self\.(roadmap|ship|think|signal)\)' "$f" || true)
    if [[ -n "$hits" ]]; then
        fail "$f takes an engine lock outside the native reader:"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 3: typed native writes, no handler trampoline ----------------------
writer=crates/magistr-engine/src/native/native_engines.rs
hits=$(grep -nE 'Parameters|CallToolResult|::mcp::|serde_json::from_|Waker::noop|handler_call|\.record_check_full\(' "$writer" | grep -vE '^[0-9]+:\s*//' || true)
if [[ -n "$hits" ]]; then
    fail "native writer bypasses typed application use cases:"
    echo "$hits"
fi
if [[ -e crates/magistr-engine/src/native/handler_call.rs ]]; then
    fail "the one-poll MCP handler trampoline must not exist"
fi

# --- Rule 4: the transport never calls its own wire ---------------------------
hits=$(grep -nE 'CallToolRequestParams::new\(' crates/magistr-cli/src/mcp_tools/*.rs 2>/dev/null | grep -v '\.tests\.rs' || true)
if [[ -n "$hits" ]]; then
    fail "mcp_tools constructs a tool call to its own wire — act through NativeEngines instead:"
    echo "$hits"
fi

# --- Rule 5: direct dependents of magistr-workflow ------------------------------
if [[ -z "${LINT_PROJECT_ROOT:-}" ]]; then
    dependents=$(cargo tree -i magistr-workflow --depth 1 --prefix none 2>/dev/null \
        | awk 'NR > 1 { print $1 }' | sort | tr '\n' ' ')
    if [[ "$dependents" != "magistr-cli magistr-engine " ]]; then
        fail "magistr-workflow direct dependents must be exactly magistr-cli and magistr-engine; found: ${dependents:-none}"
    fi
fi

# --- Rule 6: the owned application boundary has no MCP dependency -----------
application=crates/magistr-workflow/src
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
        crates/magistr-engine/src/native/*) continue ;;
        crates/magistr-cli/src/application/*) continue ;;
    esac
    hits=$(grep -nE '\.(ship_admit_gate|ship_settle_gate|ship_record_gate)\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f admits or settles a check outside the composition boundary (use magistr::application::AdmittedCheck):"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 8: writer admissions carry the evidence capability ---------------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/native/*) continue ;;
        crates/magistr-core/src/evidence/*) continue ;;
    esac
    hits=$(grep -nE 'ProjectAuthority::(admit|admit_for_project)\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    [[ -z "$hits" ]] && continue
    case "$f" in
        crates/magistr-cli/src/mcp_tools/service.rs)
            grep -qE 'evidence\(\)\.is_none\(\)' "$f" || fail "$f admits the MCP service without refusing a config that lacks the evidence capability"
            ;;
        crates/magistr-cli/src/application/workflow_client/client.rs)
            grep -qE 'evidence\(\)\.is_none\(\)' "$f" || fail "$f admits a cockpit host without refusing a config that lacks the evidence capability"
            ;;
        *)
            grep -qE 'writer_config\(' "$f" || {
                fail "$f admits a writer without magistr_core::evidence::writer_config (its checks would certify no source and its chunks could not complete):"
                echo "$hits"
            }
            ;;
    esac
done < <(production_sources)

# --- Rule 9: workspace owners open only on the inventory's owner paths -------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-workspace/src/*) continue ;;
        crates/magistr-cli/src/workspace_mcp/connection.rs) continue ;;
        crates/magistr-cli/src/application/hosted_workspace.rs) continue ;;
    esac
    hits=$(grep -nE 'WorkspaceService::open\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f opens a WorkspaceService outside the bridge owner and the hosted workspace (a second owner path):"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 10: writer admission only at the inventory's admission sites -------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/native/*) continue ;;
        crates/magistr-cli/src/mcp_tools/service.rs) continue ;;
        crates/magistr-cli/src/orchestration/roadmap/admit_native_host.rs) continue ;;
        crates/magistr-core/src/workflow/iteration.rs) continue ;;
        crates/magistr-cli/src/application/workflow_client/client.rs) continue ;;
    esac
    hits=$(grep -nE 'ProjectAuthority::(admit|admit_for_project)\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f admits a workflow writer outside the client inventory's admission sites (docs/architecture/clients.md):"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 11: attachment only from the inventory's attached surfaces --------
while IFS= read -r f; do
    case "$f" in
        crates/magistr-engine/src/native/*) continue ;;
        crates/magistr-cli/src/application/workflow_client/client.rs) continue ;;
        crates/magistr-cli/src/mcp_tools/owner_attach.rs) continue ;;
        crates/magistr-cli/src/workspace_mcp/connection.rs) continue ;;
    esac
    hits=$(grep -nE 'OwnerClient::attach(_project|_with|_mcp|_workspace)?\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f attaches to an owner outside the client inventory's attached surfaces (docs/architecture/clients.md):"
        echo "$hits"
    fi
done < <(production_sources)

# --- Rule 12: the ACP host never writes a source file directly -------------
while IFS= read -r f; do
    hits=$(grep -nE '(tokio::fs|std::fs|fs)::(write|create_dir|create_dir_all|remove_file|rename|copy)\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f writes the filesystem from the ACP host (a session's source writes take its SourceRoute or are refused):"
        echo "$hits"
    fi
done < <(find crates/magistr-acp/src -name '*.rs' -not -name '*.tests.rs' | sort)

# --- Rule 13: administrative writes only through the administrative service --
while IFS= read -r f; do
    case "$f" in
        crates/magistr-cli/src/scaffold/admin_service.rs) continue ;;
        crates/magistr-cli/src/scaffold/manifest.rs) continue ;;
        crates/magistr-cli/src/scaffold/remove_scaffold.rs) continue ;;
        crates/magistr-cli/src/scaffold/owned_removal.rs) continue ;;
        crates/magistr-cli/src/scaffold/codex_skill_links.rs) continue ;;
        crates/magistr-cli/src/scaffold/pipeline.rs) continue ;;
        crates/magistr-cli/src/scaffold/ci.rs) continue ;;
        crates/magistr-cli/src/scaffold/generate.rs) continue ;;
        crates/magistr-cli/src/scaffold/trace.rs) continue ;;
        crates/magistr-cli/src/scaffold/*) ;;
        *) continue ;;
    esac
    hits=$(grep -nE '(std::fs|fs)::(write|create_dir|create_dir_all|remove_file|remove_dir|remove_dir_all|rename|copy|hard_link|set_permissions)\(|OpenOptions::new\(|File::create\(' "$f" | grep -vE '^[0-9]+:\s*//' || true)
    if [[ -n "$hits" ]]; then
        fail "$f writes the filesystem outside the administrative service (plan the change and execute it through scaffold::AdminService):"
        echo "$hits"
    fi
done < <(production_sources)

if [[ "$errors" -eq 0 ]]; then
    echo "OK: one native write path — engines composed in magistr-engine/native, typed application verbs beneath transports, checks admitted through the composition boundary with the evidence capability, owners opened and attached only on the inventoried paths, no MCP roundtrips, no direct file write in the ACP host, administrative writes only through the administrative service."
    exit 0
fi
echo "native-write-path-gate: $errors error(s)"
exit 1
