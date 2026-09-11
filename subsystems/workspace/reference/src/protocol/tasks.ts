/**
 * The Tasks extension surface (SDD §5.1, §7.3.2): the background Tier-1 index
 * build exposed as the pollable `index/build` task.
 *
 * There is no task store — statelessness is structural, as everywhere in the
 * protocol layer. The Task is a pure derivation of `IndexPort.status()`: the
 * engine child owns the build lifecycle including its timestamps (port v7),
 * so any server process over the same workspace derives the same answer. A
 * server restarted onto a warm index has no build in its engine child and the
 * task honestly disappears rather than being replayed from stored state.
 */

import type { Task } from "@modelcontextprotocol/sdk/types.js";
import type { IndexStatus } from "../ports";

/** The one background task a v1 workspace can have (SDD §7.3.2). */
export const INDEX_BUILD_TASK_ID = "index/build";

/** Suggested poll cadence while the build works: fast, because builds are. */
const POLL_INTERVAL_MS = 200;

/**
 * Derive the `index/build` task from the index status, or `null` when no
 * build has run in this engine child's lifetime (nothing to poll). `ttl` is
 * null by design: the record lives exactly as long as the engine child does.
 */
export function indexBuildTask(status: IndexStatus | null): Task | null {
  if (status === null || status.buildStartedAtMs === undefined) return null;
  const base = {
    taskId: INDEX_BUILD_TASK_ID,
    ttl: null,
    createdAt: new Date(status.buildStartedAtMs).toISOString(),
    lastUpdatedAt: new Date(status.buildUpdatedAtMs ?? status.buildStartedAtMs).toISOString(),
  };
  if (status.building === true) {
    return {
      ...base,
      status: "working",
      pollInterval: POLL_INTERVAL_MS,
      statusMessage: `index build in progress: ${status.filesIndexed ?? 0}/${status.filesTotal ?? 0} files`,
    };
  }
  if (status.lastError !== undefined) {
    return { ...base, status: "failed", statusMessage: `index build failed: ${status.lastError}` };
  }
  if (status.tier >= 1) {
    return {
      ...base,
      status: "completed",
      statusMessage:
        `index at tier ${status.tier}, generation ${status.generation}` +
        (status.filesTotal === undefined ? "" : ` (${status.filesTotal} files)`),
    };
  }
  // Kicked, unfinished, no recorded failure, still Tier 0: the settle race
  // between a build's last commit and its status flip. Working is the only
  // honest label — the client polls again and lands on a terminal state.
  return {
    ...base,
    status: "working",
    pollInterval: POLL_INTERVAL_MS,
    statusMessage: `index build in progress: ${status.filesIndexed ?? 0}/${status.filesTotal ?? 0} files`,
  };
}
