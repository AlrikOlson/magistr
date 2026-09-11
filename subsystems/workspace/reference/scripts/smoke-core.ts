/**
 * CI smoke-load for workbench-core artifacts (SDD §3 acceptance).
 *
 *   bun run scripts/smoke-core.ts native   # the platform-native addon
 *   bun run scripts/smoke-core.ts wasm     # the wasm fallback + auto-selection
 *   bun run scripts/smoke-core.ts          # default selection (native → wasm)
 *
 * Every mode is gated on the startup self-test: `loadVerifiedCore` throws
 * unless parse, search and rewrite all passed through the loaded binding.
 */

import { loadVerifiedCore } from "../src/adapters/core";

const mode = process.argv[2] ?? "auto";

function report(label: string, loaded: ReturnType<typeof loadVerifiedCore>): void {
  const langs = loaded.binding.languages();
  console.log(
    `${label}: flavor=${loaded.flavor} version=${loaded.binding.version()} languages=${langs.length}`,
  );
  if (langs.length !== 13) {
    throw new Error(`expected 13 bundled languages, got ${langs.length}`);
  }
}

switch (mode) {
  case "native": {
    report("native", loadVerifiedCore({ force: "native" }));
    break;
  }
  case "wasm": {
    report("wasm (forced)", loadVerifiedCore({ force: "wasm" }));
    // The acceptance criterion: wasm is AUTO-selected when native fails.
    const fallback = loadVerifiedCore({
      attempts: {
        tryNative: () => {
          throw new Error("simulated native load failure");
        },
      },
    });
    if (fallback.flavor !== "wasm") {
      throw new Error(`expected auto-fallback to wasm, got ${fallback.flavor}`);
    }
    report("wasm (auto-fallback)", fallback);
    break;
  }
  case "auto": {
    report("auto", loadVerifiedCore());
    break;
  }
  default:
    throw new Error(`unknown smoke mode: ${mode}`);
}

console.log("smoke-core: OK");
