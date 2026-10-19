# Claude Code integration: verification and remaining acceptance

Recorded 2 October 2026 on Windows, with Claude Code 2.1.287, Node 24.3.0,
Python 3.13 and the Magistr source build. The installed Magistr on PATH still
reports 0.1.0; package version alone does not prove the new query is available.
The native stdio test uses the freshly built executable in `target/debug`.

## Executed evidence

| Check | Actual result | What it establishes |
| --- | --- | --- |
| Context unit tests | 5 passed | Unknown/scoped phase, canonical check resolver, read descriptor, malformed selectors, bounded Unicode evidence |
| MCP service integration | 13 passed | Real rmcp queries and refusals, discovery, native gate recording and existing behavior |
| Service owner integration | 6 passed | Native admission, lease lifetime, role coexistence, copied roots and Windows feed refusal |
| Independent adapter contracts | 26 passed | Shipped decoder, renderer and handlers, action confirmation and bounded check limits, changed target/command refusal, two raw reads, stale replies, cancellation, missing/incompatible servers, duplicate clicks, coexistence and text fallback |
| Native stdio boundary | Passed | A new isolated fixture's genuine Magistr MCP service is decoded by the shipped adapter; explicitly announced phase, canonical task and durable check receipt are read back |
| Rust formatting and CLI Clippy | Passed | Affected native code formats and compiles without Clippy warnings |
| Structure and native-write-path gates | Passed | Existing facade, dependency and native ownership checks; early runs observed intervening source edits, so their receipts do not claim a single unchanged final tree |
| Plugin/marketplace validation | Passed | Claude accepts both manifests and the package layout |
| Installed lifecycle | Passed | Upgrade to 0.1.6, exact cached runtime files, disable/enable, uninstall/catalog removal and reinstall; unrelated plugins/settings and protected checkout files retain their values |
| Official Mod test harness | Refused before tests ran | Host rollout restriction below |
| Actual Claude command | Module rejected, then authentication failed | Host restriction and expired OAuth below |

The independent runner is deliberately labeled as such. It is not Claude's
official harness. The native stdio fixture proves a native protocol boundary,
not live Claude-to-MCP integration. The real connected Workbench status was
also inspected and reports this checkout's canonical root. It is not proof that
Claude has admitted its Workbench registration.

## Host acceptance remains open

Eight tests in `plugins/magistr/tests/mod.test.ts` use the documented
`claude-code/testing` harness. They cover terminal/Desktop element trees,
navigation, confirmed normal-tool admission, cancellation, permission refusal,
restricted MCP, unrelated tools, narrow layouts/session clear and neighboring
plugin composition. They have **not executed** on this host.

The actual installed host returns:

```text
claude plugin test: hooks modules are turned off in this process:
the rollout switch served off, and a plugin's tests run only while it is on
```

A real command with the package returns:

```text
magistr: hooks module not loaded ... the rollout switch served off
Failed to authenticate: OAuth session expired and could not be refreshed
```

A second attempt is not evidence that the restriction has lifted. The official
troubleshooting contract says there is no local setting to override that remote
switch. Managed restrictions must also be respected. This work did not enable
a forbidden API, alter managed settings, invoke a model or replace sign-in.

Consequently these agreed acceptance criteria remain unverified:

- Generated types from the installed 2.1.287 build. The inspected public bundle
  identifies itself as 2.1.277 and is supporting evidence only.
- Official hook ordering, host permissions, interrupted/reloaded hook execution,
  disabled/managed behavior and genuine pane interactions.
- Live Claude MCP calls against admitted Magistr and Workbench connections.
- Actual keyboard focus, escape, terminal painting/contrast and Desktop painting.
  Deterministic narrow/wide element trees are not screenshots or live keyboard evidence.
- Invocation of the namespaced text skill in an authenticated Claude session.

The native objective and integration chunk must remain open until those checks
pass. The package is a preview, not a verified finished release.

## Connection boundary at capture time

Current installations use one `magistr mcp --dir .` connection. The first CLI
distribution targets Apple Silicon macOS; the following is retained Windows
historical evidence, not a current installation recipe.

This historical Windows check predates the unified endpoint. At capture time,
Claude registrations launched `magistr mcp-launch magistr .` and
`magistr mcp-launch workbench-mcp .`. Claude reported the workflow connection
closed while another Service owner holds the project; source connection approval
is pending. Workflow MCP attachment remains Unix-only. Windows workspace
attachment has an authenticated named-pipe path, which is a different capability.

The Mod reports unavailable/stale connections and refuses actions when identity
cannot be checked. It never kills the existing owner, deletes its lock or starts
an alternative owner. A fresh isolated fixture can start its own admitted
workflow server normally. A server that predates `magistr_context` supports the
older read views, while progress/evidence/action admission explains the need to
update and reconnect. Do not replace a running binary or restart someone else's
session to clear the limitation.

## Reproduce focused checks

From the checkout:

```sh
cargo fmt --all -- --check
cargo clippy -p magistr-cli --all-targets -- -D warnings
cargo test -p magistr-cli --lib mcp_tools::view_context::tests
cargo test -p magistr-cli --test mcp_serve_integration
cargo test -p magistr-cli --test mcp_service_owner
node --test plugins/magistr/tests/*.contract.mjs
node --test plugins/magistr/tests/native.native.mjs
npx --yes prettier@3.9.9 --check "plugins/magistr/hooks/*.js" "plugins/magistr/tests/*.{js,mjs,ts}"
claude plugin validate .
claude plugin validate plugins/magistr
claude plugin test plugins/magistr
```

Record actual commands through `ship_check`, and run the applicable
`structure` and `native-write-path` named gates through `magistr_gate`.
Keep verification focused. These adapter changes do not require workspace
reference compatibility, release smoke/latency or desktop/terminal goldens.

On Windows a native server may have a verbatim-prefixed working directory that
MSVC and Node cannot use. The committed wrapper normalizes only its own project
path and sets an ordinary Cargo target path; it preserves the real command exit
code. Invoke it with an ordinary absolute path, for example:

```powershell
powershell -NoProfile -File D:/Code/magistr/scripts/claude-mod-native-check.ps1 -Check context
```

Its other choices are `format`, `lint`, `mcp`, `adapter`, `native`,
`jsformat`, `package`, `host` and `lifecycle`. The lifecycle choice is
opt-in and temporarily uninstalls/reinstalls only this checkout's user-scope
Magistr plugin. Run it only when that is intended; it checks ownership before
changing anything and compares unrelated state after each stage.

## Finish live acceptance

Once Anthropic enables third-party Mods and ordinary Claude sign-in succeeds:

1. Generate the installed types with `/plugin-types`; compare used APIs and
   events with the official contract and run the official harness.
2. In a separate fixture project, use Magistr's supported setup and approve its
   two existing MCP registrations through Claude's ordinary permission flow.
3. Install/upgrade through the local marketplace and restart Claude normally.
   Open `/magistr`, navigate all five views with the keyboard, inspect source
   and check receipts, confirm a chosen task and focused check, then verify the
   actual native read-back. Exercise cancellation and a native refusal.
4. Capture loading, empty, unavailable, stale and recovered states; test a narrow
   terminal and the supported Desktop host. Observe another plugin's band and
   tool result without changing either.
5. Test reload, disablement and managed refusal in allowed disposable host
   configurations. Verify `/magistr:status` and ordinary text commands.
6. Record actual evidence, resolve observed failures and only then close native
   acceptance and remove the preview notice.

Official sources: [Mod overview](https://code.claude.com/docs/en/plugins/mods/overview),
[API](https://code.claude.com/docs/en/plugins/mods/api),
[interface](https://code.claude.com/docs/en/plugins/mods/interface),
[tests](https://code.claude.com/docs/en/plugins/mods/test),
[administration](https://code.claude.com/docs/en/plugins/mods/admin) and
[troubleshooting](https://code.claude.com/docs/en/plugins/mods/troubleshoot).
