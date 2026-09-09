# Project authority

Each cooperating Magistr MCP service acquires an owner lease before opening
native engines or serving tools. A second service for the same project and
store refuses startup. It does not attach to the existing process.

`MagistrService::new` and `for_project` return an asynchronous
`std::io::Result<Self>`. Callers await admission and handle startup errors.
Clones of an admitted service share its engines and `Arc<OwnerLease>`.

## Lock identity

Persistent admission uses
`<data_dir>/roadmap/sessions/<project_id>.mcp-owner.lock`. The path comes from the
native persistence identity. Copied project identities using one store contend;
distinct project/store pairs remain independent. Filesystem aliases contend on
the same lock inode.

Stateless admission uses `<project_root>/.magistr/mcp-owner.lock`. Both forms
refuse startup on contention or I/O failure. Lock files are never truncated,
removed, or replaced. The operating system releases the lock when its last
handle closes or the process exits. A leftover file is normal; deleting it
while an owner runs can allow a second inode and defeat exclusion.

## Event sockets

A service publishing events also locks `<socket>.mcp-owner.lock`, so different
stores cannot race on the same publisher path. A bounded connection probe keeps
a live listener intact. Only a missing or connection-refused endpoint proceeds
to binding. Probe timeout, other connection errors, and bind failures refuse
startup. A stale socket can be reclaimed without deleting its lock file.

Unix CLI startup requests the project feed by default. Explicit configurations
can omit broadcasting. Non-Unix startup keeps a no-feed default, and an explicit
Unix-socket request returns an unsupported error.

## Cached state and reload

Native engines cache state in memory. A persistence merge can retain another
writer's additions on disk without refreshing the caller's snapshot. Explicit
reload methods update roadmap, ship, and signal views. The cockpit uses reloads
when its detached-feed timer observes a changed store stamp.

Reloading is separate from command authority. It neither admits a second writer
nor authenticates a client. Use the same owner for project writes instead of
opening independent cached engines to work around an occupied lease.

## Limits

Exclusion is cooperative among services that acquire the lease. Older binaries,
standalone think-and-ship, and direct `NativeEngines` callers do not participate.
Advisory locks do not protect against deliberate store or lock-file modification.

Proxy attachment, client authentication, generation fencing, and shared writer
routing are separate work. The current lease must not be described as a
distributed authority or a governed peer runtime.

The event publisher can outlive a dropped service inside a running async
runtime. Admission continues to refuse a still-live endpoint; releasing an
owner lease alone does not establish that its publisher has shut down. Stop the
owning runtime or process normally before reconnecting.

The CLI integration tests cover lock lifetime, contention, aliases, independent
stores, stale sockets, and process exit. See `crates/magistr-cli/tests` and the
native-engine tests in `crates/magistr-engine/src/tas`.
