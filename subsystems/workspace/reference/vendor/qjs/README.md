# vendor/qjs — the `script` tool's guest interpreter

`qjs-wasi.wasm` is the official QuickJS-ng **v0.16.2** CLI compiled to
wasm32-wasi, downloaded from the QuickJS-ng GitHub release and proven under
the Wasmtime host in the parked M4 spike (`spikes/m4-component-host`,
pinned think:79).

- sha256: `d2939e98c808e8b9f4164cd0d7b0398cbc0121ddf52862bcd92157d923e461cc`
- upstream: https://github.com/quickjs-ng/quickjs (release v0.16.2, `qjs-wasi.wasm` asset)
- license: MIT (QuickJS-ng)

The engine child executes it through `sandboxRunWasiModule` (SDD §8.1): fuel
metering, a linear-memory cap, zero preopens, no env, no inherited stdio — the
guest's whole world is argv plus captured stdout/stderr. The `script` tool
(SDD §9.7, chunk:m4-script-sandbox) builds its bootstrap program around this
interpreter; `std`/`os` modules are absent by construction (verified: `typeof
std` is `undefined` under `-e`), so the guest has no module-level FS or OS
surface even before WASI denies it.

To upgrade: replace the artifact, update the hash and version here, and re-run
the script integration suite (`bun test test/script-tool.test.ts`).
