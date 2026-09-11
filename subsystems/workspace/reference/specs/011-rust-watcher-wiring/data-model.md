# Phase 1 Data Model: Watcher Wiring

One new cache-class record, one new config key, one widened bridge option shape. No durable
store changes; no index schema change.

## New (cache-class): `.workbench/watcher.json`

```
WatcherStatusRecord
  v            1
  pid          number        the server process that wrote it
  mode         "auto" | "native" | "poll" | "off"   the resolved setting
  adapter      "fs.watch" | "polling"               what auto/forced resolved to
  root         string        absolute workspace root
  startedAtMs  number
  updatedAtMs  number        last write (throttled to ≥ 1 s apart while events flow)
  stoppedAtMs? number        present after an orderly stop
  events       number        settled events received by the bridge
  refreshes    number        refreshFile calls completed
  removals     number        removeFile calls completed
  failures     number        calls that rejected (dropped; verification covers them)
  lastEventAtMs? number
  lastError?   string        verbatim, last failure
```

Rules: written at subscribe, throttled on events, at stop; never read by the server; doctor's
reading is read-only; unreadable ⇒ reported, ignored; a record whose `pid` is not alive is
"last session", never "live". Gitignored with the state dir.

## New config key

```
"watcher.mode": { kind: "enum", levels: ["auto", "native", "poll", "off"], default: "auto" }
```
Nearest-wins (not `capped`): operational, not security.

## Extended (additive): the bridge

```
bindWatcherToIndex(watcher, index, root, options?)
  options.maxInFlight?: number           default 8
  options.ignore?: (rel: string) => boolean   extra filter (the temp-file suffix by default)
  options.onEvent?: (kind, rel) => void  status hook
  options.onSettled?: (rel, outcome: "refreshed"|"removed"|"failed", error?) => void
returns WatchSubscription & { idle(): Promise<void> }   // resolves when the queue drains (tests)
```
Existing callers (the fuzzer, fswatch tests) pass no options and see the same behavior plus
the bound.

## doctor environment detail (additive)

```
detail.watcher:
  { state: "live",         record }      pid alive
  { state: "last-session", record }      pid gone
  { state: "not-active",   reason }      no record
  { state: "unreadable",   file, error }
```
The findings line names the state; the detail carries the record.
