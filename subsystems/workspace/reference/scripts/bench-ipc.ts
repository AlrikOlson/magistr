/**
 * The IPC perf gate (SDD §6.3): p95 round-trip overhead under 1ms on a
 * local pipe, CI-enforced. Measures ping round-trips against a real engine
 * child — the stub core keeps the measurement about frames, pipes and the
 * event loop (ping never touches the core either way) and lets the gate run
 * on lanes without built artifacts.
 *
 * Usage: bun run scripts/bench-ipc.ts [--report-only] [--json <file>]
 */

import { EngineSupervisor } from "../src/adapters/engine-ipc";
import {
  BENCH_SCHEMA,
  type BenchReport,
  BenchUsageError,
  IPC_P95_GATE_MS,
  ipcP95Gate,
  machineFacts,
  parseBenchArgs,
  writeReport,
} from "./bench-common";

const WARMUP = 200;
const SAMPLES = 2000;
const GATE_MS = IPC_P95_GATE_MS;

let args: ReturnType<typeof parseBenchArgs>;
try {
  args = parseBenchArgs(process.argv.slice(2));
} catch (error) {
  if (error instanceof BenchUsageError) {
    console.error(`usage: ${error.message}`);
    process.exit(2);
  }
  throw error;
}

const supervisor = new EngineSupervisor({ env: { WORKBENCH_ENGINE_STUB_CORE: "1" } });

try {
  for (let i = 0; i < WARMUP; i += 1) await supervisor.call("ping", {});

  const samplesMs: number[] = [];
  for (let i = 0; i < SAMPLES; i += 1) {
    const start = Bun.nanoseconds();
    await supervisor.call("ping", {});
    samplesMs.push((Bun.nanoseconds() - start) / 1e6);
  }

  samplesMs.sort((a, b) => a - b);
  const at = (q: number) => samplesMs[Math.min(Math.floor(q * SAMPLES), SAMPLES - 1)] ?? 0;
  const p50 = at(0.5);
  const p95 = at(0.95);
  const p99 = at(0.99);
  const max = samplesMs[SAMPLES - 1] ?? 0;
  console.log(
    `ipc round-trip over ${SAMPLES} pings: ` +
      `p50=${p50.toFixed(3)}ms p95=${p95.toFixed(3)}ms p99=${p99.toFixed(3)}ms ` +
      `max=${max.toFixed(3)}ms`,
  );

  const gate = ipcP95Gate(p95);
  if (args.reportOnly) {
    console.log("report-only: gate not applied");
  } else if (!gate.pass) {
    console.error(`FAIL: p95 ${p95.toFixed(3)}ms exceeds the ${GATE_MS}ms gate (SDD §6.3)`);
    process.exitCode = 1;
  } else {
    console.log(`OK: p95 ${p95.toFixed(3)}ms is under the ${GATE_MS}ms gate`);
  }

  if (args.json !== undefined) {
    const report: BenchReport = {
      schema: BENCH_SCHEMA,
      script: "bench-ipc",
      profile: "ipc",
      recordedAt: new Date().toISOString(),
      machine: machineFacts("stub"),
      measurements: {
        pings: SAMPLES,
        p50Ms: Number(p50.toFixed(4)),
        p95Ms: Number(p95.toFixed(4)),
        p99Ms: Number(p99.toFixed(4)),
        maxMs: Number(max.toFixed(4)),
      },
      gates: [gate],
    };
    writeReport(args.json, report);
    console.log(`report: ${args.json}`);
  }
} finally {
  await supervisor.stop();
}
