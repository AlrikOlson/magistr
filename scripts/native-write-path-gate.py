#!/usr/bin/env python3
"""Enforce all thirteen native ownership rules in one source scan."""
import os
from pathlib import Path
import subprocess
from source_scan import SourceTree, hits, pattern

NATIVE = "crates/magistr-engine/src/native/"
WORKFLOW = "crates/magistr-workflow/src/"
APPLICATION = "crates/magistr-cli/src/application/"
ADMISSIONS = ("crates/magistr-cli/src/mcp_tools/service.rs", APPLICATION + "workflow_client/client.rs")
ATTACHMENTS = (*ADMISSIONS[1:], "crates/magistr-cli/src/mcp_tools/owner_link.rs",
               "crates/magistr-cli/src/workspace_mcp/connection.rs")
ADMIN_FILES = {f"crates/magistr-cli/src/scaffold/{name}.rs" for name in (
    "admin_service", "manifest", "remove_scaffold", "owned_removal",
    "codex_skill_links", "generate", "trace")}


def violations(tree, check_dependencies=True):
    errors = []
    sources = {name: text for name, text in tree.rust().items()
               if "/src/" in name and not name.endswith(".tests.rs")}

    def refuse(name, text, expression, message, comments=False):
        found = hits(text, expression, comments)
        if found:
            errors.append(f"ERROR: {name} {message}:\n" + "\n".join(found))
        return bool(found)

    for name, text in sources.items():
        if not name.startswith((NATIVE, WORKFLOW)):
            refuse(name, text, r"\b(RoadmapEngine|ShipEngine|SignalEngine|ReasoningServer|ThinkService|ShipService|RoadmapService|SignalService|ThinkUseCases|ShipUseCases|RoadmapUseCases|SignalUseCases)\b|UnifiedService::new|open_engines\(",
                   "composes or names a workflow engine or service outside magistr-engine/src/native")
        if name.startswith(NATIVE) and name != NATIVE + "native_reader.rs":
            refuse(name, text, r"roadmap_engine\(\)|ship_engine\(\)|think_engine\(\)|signal_engine\(\)|lock\(&self\.(roadmap|ship|think|signal)\)",
                   "takes an engine lock outside the native reader", comments=True)
        if name == NATIVE + "native_engines.rs":
            refuse(name, text, r"Parameters|CallToolResult|::mcp::|serde_json::from_|Waker::noop|handler_call|\.record_check_full\(",
                   "bypasses typed application use cases")
        if name.startswith("crates/magistr-cli/src/mcp_tools/"):
            refuse(name, text, r"CallToolRequestParams::new\(",
                   "constructs a tool call to its own wire — act through NativeEngines instead", comments=True)
        if name.startswith(WORKFLOW) and "/application/" in name:
            refuse(name, text, r"(^|[^a-zA-Z0-9_])(rmcp|CallToolResult|Parameters)|::mcp::|Waker::noop",
                   "couples a native application use case to MCP")
        if not name.startswith((NATIVE, APPLICATION)):
            refuse(name, text, r"\.(ship_admit_gate|ship_admit_check|ship_reuse_gate|ship_settle_gate|ship_record_gate)\(",
                   "admits or settles a check outside the composition boundary (use magistr::application::AdmittedCheck)")
        admission = bool(hits(text, r"ProjectAuthority::(admit|admit_for_project)\(", comments=False))
        if admission and not name.startswith((NATIVE, "crates/magistr-core/src/evidence/")):
            required = r"evidence\(\)\.is_none\(\)" if name in ADMISSIONS else r"writer_config\("
            if not pattern(required).search(text):
                errors.append(f"ERROR: {name} admits a writer without the verification evidence capability")
        if not name.startswith("crates/magistr-workspace/src/") and name not in (
                "crates/magistr-cli/src/workspace_mcp/connection.rs", APPLICATION + "hosted_workspace.rs"):
            refuse(name, text, r"WorkspaceService::open\(",
                   "opens a WorkspaceService outside the bridge owner and the hosted workspace")
        if admission and not name.startswith(NATIVE) and name not in ADMISSIONS:
            errors.append(f"ERROR: {name} admits a workflow writer outside the client inventory's admission sites")
        if not name.startswith(NATIVE) and name not in ATTACHMENTS:
            refuse(name, text, r"OwnerClient::attach(_project|_with|_mcp|_workspace)?\(",
                   "attaches to an owner outside the client inventory's attached surfaces")
        if name.startswith("crates/magistr-acp/src/"):
            refuse(name, text, r"(tokio::fs|std::fs|fs)::(write|create_dir|create_dir_all|remove_file|rename|copy)\(",
                   "writes the filesystem from the ACP host (source writes take SourceRoute or are refused)")
        if name.startswith("crates/magistr-cli/src/scaffold/") and name not in ADMIN_FILES:
            refuse(name, text, r"(std::fs|fs)::(write|create_dir|create_dir_all|remove_file|remove_dir|remove_dir_all|rename|copy|hard_link|set_permissions)\(|OpenOptions::new\(|File::create\(",
                   "writes the filesystem outside the administrative service")
    if (tree.root / NATIVE / "handler_call.rs").exists():
        errors.append("ERROR: the one-poll MCP handler trampoline must not exist")
    if check_dependencies:
        result = subprocess.run(["cargo", "tree", "-i", "magistr-workflow", "--depth", "1", "--prefix", "none"],
                                cwd=tree.root, text=True, capture_output=True)
        dependents = sorted(line.split()[0] for line in result.stdout.splitlines()[1:] if line.strip())
        if result.returncode or dependents != ["magistr-cli", "magistr-engine"]:
            errors.append(f"ERROR: magistr-workflow direct dependents must be exactly magistr-cli and magistr-engine; found: {dependents}\n{result.stderr}")
    return errors


def main():
    root = os.environ.get("LINT_PROJECT_ROOT") or Path(__file__).resolve().parent.parent
    errors = violations(SourceTree(root), check_dependencies=not os.environ.get("LINT_PROJECT_ROOT"))
    if errors:
        print("\n".join(errors))
        print(f"native-write-path-gate: {len(errors)} error(s)")
        return 1
    print("OK: all thirteen native ownership rules passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
