---
name: roadmap
description: Advance the native Magistr roadmap by implementing one ready chunk. Use for /roadmap or a request to do the next roadmap item; use roadmap-refresh for research without implementation.
---

# Advance the Magistr roadmap

For implementation, follow the complete [next skill](../next/SKILL.md) in the
current session. It owns selection, recovery, research, planning, execution,
recorded gates, roadmap completion, and commit. Do one chunk and stop.

For a status-only request, read `roadmap_status` and relevant `roadmap_get`
records from **magistr**, report them, and stop without starting a chunk.
If empty, report it; seed or import a roadmap only when the user's request
includes creating one. Use the native tools and their current schemas.

Follow the shared runtime's roadmap/export policy. Do not open a standalone
think-and-ship store or treat generated files as an alternative source of truth.
