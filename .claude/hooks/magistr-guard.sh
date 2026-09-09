#!/bin/sh
# magistr guard hook — scaffolded by `magistr init`.
#
# Wired from .claude/settings.json as a Stop hook (`stop`), a PreToolUse
# hook on Bash (`commit`) and a PreToolUse hook on Edit/Write/MultiEdit/
# NotebookEdit (`write`, refusing writes under a frozen tree declared in
# .magistr/legacy-gui-ratchet.json). Forwards the hook JSON on stdin to `magistr guard`,
# which reads the ship/roadmap state (never a plan file) and exits 2 with the
# reason on stderr to block. A magistr that is missing from PATH, or predates
# the `guard` verb, allows through and says so — an old binary must never
# freeze a session.
event="${1:?usage: magistr-guard.sh <stop|commit|write>}"
bin="${MAGISTR_BIN:-magistr}"

if ! command -v "$bin" >/dev/null 2>&1; then
  echo "magistr-guard: '$bin' is not on PATH; allowing (install magistr to enforce)" >&2
  exit 0
fi

err=$("$bin" guard "$event" "${CLAUDE_PROJECT_DIR:-.}" 2>&1 >/dev/null)
rc=$?

case "$err" in
  *"unrecognized subcommand"*|*"unexpected argument"*|*"invalid value"*)
    echo "magistr-guard: the magistr on PATH has no 'guard $event' verb; allowing (upgrade magistr to enforce)" >&2
    exit 0
    ;;
esac

[ -n "$err" ] && printf '%s\n' "$err" >&2
[ "$rc" -eq 2 ] && exit 2
exit 0
