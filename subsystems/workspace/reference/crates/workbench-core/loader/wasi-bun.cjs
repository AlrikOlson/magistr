/**
 * Bun-compatible WASI loader for the `workbench-core` wasm fallback.
 *
 * The napi-generated `workbench-core.wasi.cjs` binds to `node:wasi`, whose
 * Bun implementation lacks `WASI.initialize` (Bun 1.4.0). This loader mirrors
 * the generated instantiation but uses the pure-JS WASI implementation from
 * `@napi-rs/wasm-runtime`, which works on both Bun and Node. Worker threads go
 * through `wasi-worker-bun.mjs` for the same reason.
 */
const fs = require('node:fs')
const path = require('node:path')
const { Worker } = require('node:worker_threads')
const {
  WASI,
  emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin,
  createOnMessage,
  instantiateNapiModuleSync,
} = require('@napi-rs/wasm-runtime')
const { createContext } = require('@emnapi/runtime')

/**
 * Instantiate the wasm build and return its napi exports
 * (version/languages/parse/search/rewrite/selfTest).
 *
 * @param {string} wasmPath absolute path to workbench-core.wasm32-wasi.wasm
 */
function loadWorkbenchCoreWasm(wasmPath) {
  const wasmFile = fs.readFileSync(wasmPath)
  // No preopens: the core API is strings-in/strings-out, so the wasm guest
  // needs no filesystem at all (and gets none).
  const wasi = new WASI({
    version: 'preview1',
    env: process.env,
  })
  // Same geometry as the napi-generated loader: shared, grows to 4GiB.
  const sharedMemory = new WebAssembly.Memory({
    initial: 4000,
    maximum: 65536,
    shared: true,
  })
  const context = createContext()
  const workerPath = path.join(__dirname, 'wasi-worker-bun.mjs')
  const { napiModule } = instantiateNapiModuleSync(wasmFile, {
    context,
    asyncWorkPoolSize: 4,
    reuseWorker: true,
    plugins: [emnapiAsyncWorkPlugin, emnapiTSFNPlugin],
    wasi,
    onCreateWorker() {
      const worker = new Worker(workerPath, { env: process.env })
      worker.onmessage = ({ data }) => {
        createOnMessage(fs)(data)
      }
      worker.unref()
      return worker
    },
    overwriteImports(importObject) {
      importObject.env = {
        ...importObject.env,
        ...importObject.napi,
        ...importObject.emnapi,
        memory: sharedMemory,
      }
      return importObject
    },
    beforeInit({ instance }) {
      for (const name of Object.keys(instance.exports)) {
        if (name.startsWith('__napi_register__')) {
          instance.exports[name]()
        }
      }
    },
  })
  return napiModule.exports
}

module.exports = { loadWorkbenchCoreWasm }
