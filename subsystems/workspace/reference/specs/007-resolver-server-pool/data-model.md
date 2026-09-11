# Phase 1 Data Model: Resolver Language-Server Session Pool

No new persistent stores. Everything below is in-flight, engine-child-lifetime state (never
serialized, never crosses the envelope) plus two additive shapes on existing boundaries.

## New (engine-child lifetime): ServerPool

```
ServerPool {
  sessions: Map<PoolKey, PooledSession>
  job: OsKillGroup?            // Windows job object handle; absent elsewhere
  spawner: SpawnerThread       // Linux: the thread that owns PR_SET_PDEATHSIG
  ticker: ReaperThread?        // low-frequency idle sweep
}
PoolKey = (workspace_root, language_id, bin, args[])   // exactly the request-carried identity
PooledSession {
  key: PoolKey
  child: Process
  stdin: Pipe
  reader: Thread + AtomicBool leased + queue<Frame>
  opened_at, last_used_at
  lease: LeaseId?              // at most one; None = warm and available
  handshakes_seen: u32         // for tests/diagnostics only (host counts opens, not protocol)
}
```

Invariants:
- One `PooledSession` per `PoolKey`; a second lease request while `lease` is set is refused
  (`spawn-refused`), never queued.
- `leased == false` ⇒ incoming frames are dropped; `leased == true` ⇒ queued for the guest.
- A session is removed from the map on: `close` (kill), non-normal lease end (kill), idle
  limit (kill), dead-on-open detection (kill+respawn), shutdown (kill).
- `last_used_at` is reset on lease start and lease end.

State transitions (per session):

```
(absent) --open(key)------------------> leased(fresh)      spawn + lease
leased(fresh|warm) --invoke end ok----> warm               lease cleared, queue drained on next lease
leased --release(session)------------> warm
leased --close(session)--------------> (absent)           killed
leased --invoke end non-ok-----------> (absent)           killed (fuel/trap/memory/invalid/panic)
warm --open(key)---------------------> leased(warm)        reused = true, queue drained first
warm --idle > SERVER_IDLE_TTL_MS-----> (absent)           killed by reaper
warm|leased --child process dead-----> (absent)           detected on next open or read; killed/reaped
any --pool shutdown------------------> (absent)           killed
```

## Existing, re-scoped: per-invoke SubprocessPool → LeaseView

```
LeaseView {
  mode: PerInvoke | Pooled     // decided by the import name the guest's component carries
  specs, workspace_root        // as today
  handles: Map<u32, Handle>    // Handle = PerInvokeSession (today's struct) | LeaseOf(PoolKey)
  next_handle
}
```

- `mode == PerInvoke` (guest imports `…@0.1.0`): byte-for-byte today's behavior; `reused`
  is not linked; `Drop` kills.
- `mode == Pooled` (guest imports `…@0.2.0`): `open` leases; `finish(ok)` returns or kills;
  `Drop` without `finish` kills.

## Extended (additive): ResolverOutcome / SandboxResolverOutcome / ResolverGuestOutcome

```
+ sessions: { opened: u32, reused: u32 }   // fresh spawns vs warm leases in this invoke
```

TS: `readonly sessions?: { readonly opened: number; readonly reused: number }` (optional so
the adapter rigs that script outcomes need not change).

## Extended (additive): `workbench:subprocess` interface, 0.1.0 → 0.2.0

```
+ reused: func(session: u32) -> bool
+ release: func(session: u32)
```

Existing functions, types and variant cases unchanged. Doc comments updated to describe
pooled semantics for 0.2.0 importers and per-invoke semantics for 0.1.0 importers.

## Extended (reading rule, no shape change): InstalledPluginRecord.grants

A grant string `workbench:subprocess/subprocess@0.M.P` authorizes a manifest import
`workbench:subprocess/subprocess@0.N.Q` when the majors match (here major 0). The
blessed-tier rule applies to any such import. No migration of `.workbench/plugins.json`.

## Constants

| Name | Value | Where |
|---|---|---|
| `SERVER_IDLE_TTL_MS` | 300 000 | sandbox.rs |
| `SERVER_REAPER_TICK_MS` | 30 000 | sandbox.rs |
| `SUBPROCESS_SESSION_CAP` | 4 (unchanged, per invoke) | sandbox.rs |
| `SUBPROCESS_FRAME_CAP` | 4 MiB (unchanged) | sandbox.rs |
| `RESOLVER_BUDGET_MS_DEFAULT` | 2 000 (unchanged) | resolver-plugin.ts |
| `RESOLVER_RETRY_AFTER_MS` | 60 000 (unchanged) | resolver-plugin.ts |
