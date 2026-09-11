/**
 * Shared parsing helpers for `run` and its bundled runner families. Kept in
 * their own module so the cargo family (./run-cargo) can import them without
 * a run ↔ run-cargo import cycle: run.ts value-imports the adapters, the
 * adapters take only types from run.ts.
 */

/** Program-name normalization: strip path (both separators) and .exe. */
export function programBasename(token: string): string {
  const base = token.split(/[\\/]/).pop() ?? token;
  return base.toLowerCase().replace(/\.exe$/, "");
}

export function tokensOf(commandLine: string): readonly string[] {
  return commandLine
    .split(/\s+/)
    .filter((t) => t.length > 0)
    .map(programBasename);
}

export function hasToken(commandLine: string, name: string): boolean {
  return tokensOf(commandLine).includes(name);
}

/** Trim a trace block to at most `max` lines, dropping blank padding. */
export function trimTrace(lines: readonly string[], max = 8): string {
  const kept = lines.map((l) => l.trimEnd()).filter((l) => l.trim().length > 0);
  return kept.slice(0, max).join("\n");
}

/** Workspace-relative paths are reported with forward slashes on every
 * platform (rustc prints the host separator). */
export function normalizePath(path: string): string {
  return path.replace(/\\/g, "/");
}
