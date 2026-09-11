// Worker-thread half of the Bun-compatible WASI loader. Mirrors the
// napi-generated wasi-worker.mjs but uses the pure-JS WASI from
// `@napi-rs/wasm-runtime` instead of `node:wasi` (see wasi-bun.cjs).
import fs from 'node:fs'
import { createRequire } from 'node:module'
import { parse } from 'node:path'
import { parentPort, Worker } from 'node:worker_threads'

const require = createRequire(import.meta.url)

const {
  WASI,
  instantiateNapiModuleSync,
  MessageHandler,
  getDefaultContext,
  emnapiAsyncWorkPlugin,
  emnapiTSFNPlugin,
} = require('@napi-rs/wasm-runtime')

if (parentPort) {
  parentPort.on('message', (data) => {
    globalThis.onmessage({ data })
  })
}

Object.assign(globalThis, {
  self: globalThis,
  require,
  Worker,
  importScripts: function (f) {
    ;(0, eval)(fs.readFileSync(f, 'utf8') + '//# sourceURL=' + f)
  },
  postMessage: function (msg) {
    if (parentPort) {
      parentPort.postMessage(msg)
    }
  },
})

const emnapiContext = getDefaultContext()

const handler = new MessageHandler({
  onLoad({ wasmModule, wasmMemory }) {
    // No preopens: the core API is strings-in/strings-out (see wasi-bun.cjs).
    const wasi = new WASI({
      version: 'preview1',
      env: process.env,
    })

    return instantiateNapiModuleSync(wasmModule, {
      childThread: true,
      wasi,
      context: emnapiContext,
      // The wasm links a "basic" emnapi archive; async-work and
      // threadsafe-function implementations come from the JS plugins.
      plugins: [emnapiAsyncWorkPlugin, emnapiTSFNPlugin],
      overwriteImports(importObject) {
        importObject.env = {
          ...importObject.env,
          ...importObject.napi,
          ...importObject.emnapi,
          memory: wasmMemory,
        }
      },
    })
  },
})

globalThis.onmessage = function (e) {
  handler.handle(e)
}
