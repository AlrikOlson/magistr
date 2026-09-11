/**
 * The composed, jail-checked WriteWorkspaceAccess (SDD §7.1, §7.6) — the one
 * filesystem surface the write engine sees. The jail decides before any
 * filesystem access exists; a refusal is a thrown typed error the tool layer
 * maps back to its envelope.
 *
 * `remove` is a journal-backed move, never an unlink (§7.6, Constitution III:
 * "nothing is ever unlinked destructively"): the file is renamed into
 * `.workbench/trash/` under a collision-safe name. The journal's before-blob
 * remains the restoration source for undo; the trash copy is the last-resort
 * physical artifact. Trash lives inside the jail-protected state dir, which
 * tool-addressed writes can never reach — only this composed access, moving a
 * file that was already admitted for mutation, writes there.
 */

import { randomUUID } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { basename, dirname, join } from "node:path";
import { decodeContent, type FileContent } from "../envelope";
import { checkRead, checkWrite } from "../jail";
import type { WriteWorkspaceAccess } from "../tools";

export interface WriteAccessOptions {
  /** Canonical absolute workspace root. */
  readonly root: string;
  /** `<root>/.workbench` — where the trash directory lives. */
  readonly stateDir: string;
}

/** Build the jail-checked file access the §7.5 write engine is composed over. */
export function createWriteAccess(options: WriteAccessOptions): WriteWorkspaceAccess {
  const admitRead = (path: string): string => {
    const decision = checkRead(options.root, path);
    if (!decision.ok) throw Object.assign(new Error(decision.reason), { code: decision.code });
    return decision.path;
  };
  const admitWrite = (path: string): string => {
    const decision = checkWrite(options.root, path);
    if (!decision.ok) throw Object.assign(new Error(decision.reason), { code: decision.code });
    return decision.path;
  };
  const readContent = async (path: string): Promise<FileContent> =>
    decodeContent(await readFile(admitRead(path)));
  // Per-file atomicity per the WriteWorkspaceAccess contract: temp + rename
  // in the same directory (rename replaces on every platform).
  const writeContent = async (path: string, content: FileContent): Promise<void> => {
    const target = admitWrite(path);
    await mkdir(dirname(target), { recursive: true });
    const tmp = `${target}.workbench-tmp-${process.pid}`;
    await writeFile(tmp, content, "utf8");
    await rename(tmp, target);
  };
  return {
    readContent,
    writeContent,
    read: async (path) => {
      const content = await readContent(path);
      if (typeof content !== "string")
        throw Object.assign(
          new Error(
            `${path}: binary content cannot be edited as text; use files for lossless copy, move or delete`,
          ),
          { code: "PATTERN_INVALID" },
        );
      return content;
    },
    write: writeContent,
    remove: async (path) => {
      const target = admitWrite(path);
      const trashDir = join(options.stateDir, "trash");
      await mkdir(trashDir, { recursive: true });
      await rename(target, join(trashDir, `${randomUUID()}-${basename(target)}`));
    },
  };
}
