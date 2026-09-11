/**
 * The runner-plugin lane (chunk:m5-runner-adapter): a bundled runner adapter
 * served by a WASM component on the public `workbench:runner@0.1.0`
 * interface, wrapped as an ordinary [`RunnerAdapter`] for `runRun`'s
 * existing trust-order seam.
 *
 * Protocol layer for the same reason as ./resolver-plugin: it composes
 * tools-layer vocabulary with an injected engine-child lane. The invoke is
 * typed structurally so no adapters import is needed here.
 *
 * Degradation (Constitution I): ANY lane problem — engine child down,
 * sandbox absent (wasm-fallback lane), guest trap, fuel kill, a claim
 * mismatch — falls back to the bundled TS twin, so a degraded composition
 * parses exactly as the pre-migration product did. The component is the
 * primary path; the twin is the floor, and the golden-equality suite pins
 * that both produce identical structure.
 */

import type { RunnerAdapter, RunOutput, RunReport, StructuredFailure } from "../tools/run";

/** One lifted run-report as the engine child answers it (mirrors the FFI). */
export interface RunnerGuestReport {
  readonly failures: readonly {
    readonly testName: string;
    /** Absent/null both mean "no file" — napi lowers Option::None as undefined. */
    readonly file?: string | null;
    readonly assertion: string;
    readonly trace: string;
  }[];
  readonly summary?: string | null;
}

/** Typed runner-lane outcome (mirrors the engine child's answer). */
export interface RunnerGuestOutcome {
  readonly outcome:
    | "ok"
    | "out-of-fuel"
    | "memory-cap"
    | "cap-denied"
    | "guest-trap"
    | "guest-exit"
    | "invalid-guest";
  readonly detail: string;
  readonly fuelUsed: number;
  readonly claimed: boolean;
  /** Absent/null both mean "no structure" — napi lowers Option::None as undefined. */
  readonly report?: RunnerGuestReport | null;
}

/** The engine child's `sandbox/runner` call, injected by the composition. */
export type RunnerLaneInvoke = (
  absPath: string,
  command: string,
  exitCode: number,
  stdout: string,
  stderr: string,
  capabilities: readonly string[],
  policy: { readonly fuel: number; readonly memoryCapBytes: number },
) => Promise<RunnerGuestOutcome>;

export interface ComponentRunnerAdapterInit {
  /** Adapter display name — keep the twin's so report rows are unchanged. */
  readonly name: string;
  /** Absolute path of the bundled component (e.g. plugins/runner-go/guest.wasm). */
  readonly guestPath: string;
  readonly invoke: RunnerLaneInvoke;
  /** The bundled TS twin: the degradation floor and the claim gate. */
  readonly fallback: RunnerAdapter;
  /** Guest policy; defaults mirror plugins/runner-go/plugin.toml [limits]. */
  readonly fuel?: number;
  readonly memoryCapBytes?: number;
}

/** Fold the guest's lifted report into the tools-layer shape (drop nulls). */
function reportOf(lifted: RunnerGuestReport): RunReport {
  const failures: StructuredFailure[] = lifted.failures.map((f) => ({
    testName: f.testName,
    ...(f.file === null || f.file === undefined ? {} : { file: f.file }),
    assertion: f.assertion,
    trace: f.trace,
  }));
  return {
    failures,
    ...(lifted.summary === null || lifted.summary === undefined ? {} : { summary: lifted.summary }),
  };
}

/**
 * Wrap a bundled runner component as a [`RunnerAdapter`]. `canParse` answers
 * from the TS twin (the guest re-verifies the claim on invoke — a `claimed:
 * false` disagreement falls back); `parseOutput` goes through the component
 * lane first and the twin on any failure.
 */
export function componentRunnerAdapter(init: ComponentRunnerAdapterInit): RunnerAdapter {
  const policy = {
    fuel: init.fuel ?? 200_000_000,
    memoryCapBytes: init.memoryCapBytes ?? 32 * 1024 * 1024,
  };
  return {
    name: init.name,
    canParse: (commandLine) => init.fallback.canParse(commandLine),
    parseOutput: async (output: RunOutput) => {
      try {
        const outcome = await init.invoke(
          init.guestPath,
          output.command,
          output.exitCode,
          output.stdout,
          output.stderr,
          [],
          policy,
        );
        if (outcome.outcome === "ok" && outcome.claimed) {
          // A nullish report = the guest could not extract structure — that
          // IS the adapter's answer (undefined), same as the twin's.
          const report = outcome.report ?? null;
          return report === null ? undefined : reportOf(report);
        }
      } catch {
        // Engine child down / sandbox lane absent: fall through to the twin.
      }
      return init.fallback.parseOutput(output);
    },
  };
}
