/**
 * The skill drift check (feature 006-routing-skill, contract
 * specs/006-routing-skill/contracts/skill-drift-check.md).
 *
 * The companion routing skill is hand-written prose; this check binds its
 * three machine-bound sections — the routing table, the recovery table and
 * the worked examples — to the repository's single sources of truth
 * (schemas/tools/*.input.schema.json and schemas/error-taxonomy.json, the
 * same bytes codegen:check pins into src/generated) and fails on any
 * disagreement in EITHER direction: a cited tool/argument/selection/code
 * the product lacks, or a tool/selection/required argument/code the skill
 * omits. Omission is drift.
 *
 * It never writes. There is no fix mode, by construction: the checker is a
 * pure function over text, and the CLI refuses every `--option`.
 *
 *   bun run lint:skill                       check the canonical skill + dogfood copy
 *   bun run lint:skill <skill> [<copy>]      check other paths (tests)
 *
 * Exit 0 clean · 1 violations (printed `R<n> <where>: <message>`) · 2 usage.
 */

import { Ajv2020 } from "ajv/dist/2020.js";

export type Rule = "R1" | "R2" | "R3" | "R4" | "R5" | "R6" | "R7";

export interface Violation {
  readonly rule: Rule;
  readonly where: string;
  readonly message: string;
}

export interface ToolSurface {
  readonly name: string;
  /** The tool's input JSON Schema (draft 2020-12), as in schemas/tools. */
  readonly schema: Record<string, unknown>;
}

export interface TaxonomyEntry {
  readonly code: string;
  readonly recovery: string;
}

export interface DriftInput {
  readonly skill: string;
  readonly tools: readonly ToolSurface[];
  readonly taxonomy: readonly TaxonomyEntry[];
  /** The dogfood copy's text, when that file exists. */
  readonly copy?: string;
}

/** The one schema-less built-in: routed and exampled, never shape-validated. */
export const SCHEMALESS_TOOLS: readonly string[] = ["workbench_status"];
export const SKILL_NAME = "workbench-routing";
export const CANONICAL_SKILL_PATH = "skills/workbench-routing/SKILL.md";
export const DOGFOOD_COPY_PATH = ".claude/skills/workbench-routing/SKILL.md";

const ROUTING_HEADING = "## Routing table";
const RECOVERY_HEADING = "## Recovery table";
const EXAMPLES_HEADING = "## Worked examples";
const ROUTING_COLUMNS = ["Instead of", "Use", "Arguments that matter"] as const;
const RECOVERY_COLUMNS = ["Code", "Recovery", "What to do"] as const;

/** A generated/do-not-edit marker — narrow on purpose: prose may say "the generated schemas". */
const GENERATED_MARKER = /<!--[^>]*\b(generated|do not edit)\b[^>]*-->|DO NOT EDIT|@generated/;

interface RoutingRow {
  readonly line: number;
  readonly tool: string | null;
  readonly facets: readonly { facet: string; value: string }[];
  readonly args: readonly string[];
  readonly toolTokens: readonly string[];
}

interface RecoveryRow {
  readonly line: number;
  readonly code: string | null;
  readonly recovery: string | null;
}

interface Example {
  readonly line: number;
  readonly tool: string;
  readonly body: string;
}

function backticked(cell: string): string[] {
  return [...cell.matchAll(/`([^`]+)`/g)].map((m) => m[1] ?? "").filter((t) => t.length > 0);
}

/** The lines of the section under `heading`, up to the next `## ` heading. */
function section(
  lines: readonly string[],
  heading: string,
): { start: number; body: string[] } | null {
  const start = lines.findIndex((l) => l.trim() === heading);
  if (start < 0) return null;
  const body: string[] = [];
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (/^##\s/.test(line)) break;
    body.push(line);
  }
  return { start, body };
}

function tableRows(
  body: readonly string[],
  startLine: number,
): { header: string[]; rows: { line: number; cells: string[] }[] } | null {
  const rows: { line: number; cells: string[] }[] = [];
  let header: string[] | null = null;
  for (let i = 0; i < body.length; i++) {
    const line = body[i] ?? "";
    if (!/^\s*\|/.test(line)) continue;
    const cells = line
      .trim()
      .replace(/^\|/, "")
      .replace(/\|$/, "")
      .split("|")
      .map((c) => c.trim());
    if (header === null) {
      header = cells;
      continue;
    }
    if (cells.every((c) => /^:?-{3,}:?$/.test(c))) continue;
    rows.push({ line: startLine + 1 + i + 1, cells });
  }
  return header === null ? null : { header, rows };
}

function parseRouting(cells: string[], line: number): RoutingRow {
  const use = cells[1] ?? "";
  const tokens = backticked(use);
  const toolTokens = tokens.filter((t) => !t.includes(":"));
  const facets = tokens
    .filter((t) => t.includes(":"))
    .map((t) => {
      const idx = t.indexOf(":");
      return { facet: t.slice(0, idx), value: t.slice(idx + 1) };
    });
  return {
    line,
    tool: toolTokens.length === 1 ? (toolTokens[0] ?? null) : null,
    facets,
    args: backticked(cells[2] ?? ""),
    toolTokens,
  };
}

function parseExamples(body: readonly string[], startLine: number): Example[] {
  const examples: Example[] = [];
  let open: { tool: string; line: number; buf: string[] } | null = null;
  for (let i = 0; i < body.length; i++) {
    const line = body[i] ?? "";
    const fence = /^```(.*)$/.exec(line.trim());
    if (fence !== null) {
      if (open === null) {
        const info = (fence[1] ?? "").trim();
        const m = /^json\s+workbench:([A-Za-z0-9_./-]+)$/.exec(info);
        open = { tool: m?.[1] ?? "", line: startLine + 1 + i + 1, buf: [] };
      } else {
        if (open.tool.length > 0) {
          examples.push({ line: open.line, tool: open.tool, body: open.buf.join("\n") });
        }
        open = null;
      }
      continue;
    }
    if (open !== null) open.buf.push(line);
  }
  return examples;
}

function frontmatterName(lines: readonly string[]): string | null {
  if ((lines[0] ?? "").trim() !== "---") return null;
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (line.trim() === "---") break;
    const m = /^name:\s*["']?([^"'\s]+)["']?\s*$/.exec(line);
    if (m !== null) return m[1] ?? null;
  }
  return null;
}

function enumsOf(schema: Record<string, unknown>): Map<string, readonly string[]> {
  const out = new Map<string, readonly string[]>();
  const props = schema["properties"];
  if (typeof props !== "object" || props === null) return out;
  for (const [name, spec] of Object.entries(props as Record<string, unknown>)) {
    const e = (spec as { enum?: unknown }).enum;
    if (Array.isArray(e)) out.set(name, e.map(String));
  }
  return out;
}

function propertiesOf(schema: Record<string, unknown>): Set<string> {
  const props = schema["properties"];
  return new Set(typeof props === "object" && props !== null ? Object.keys(props) : []);
}

function requiredOf(schema: Record<string, unknown>): readonly string[] {
  const r = schema["required"];
  return Array.isArray(r) ? r.map(String) : [];
}

/** Pure: same inputs → same violations, sorted. Reads nothing, writes nothing. */
export function checkSkillDrift(input: DriftInput): readonly Violation[] {
  const out: Violation[] = [];
  const add = (rule: Rule, where: string, message: string): void => {
    out.push({ rule, where, message });
  };
  const lines = input.skill.split("\n");
  const byName = new Map(input.tools.map((t) => [t.name, t] as const));
  const universe = new Set<string>([...byName.keys(), ...SCHEMALESS_TOOLS]);

  // R7 — structure and hand-written guard.
  const name = frontmatterName(lines);
  if (name !== SKILL_NAME) {
    add(
      "R7",
      "frontmatter",
      `name must be ${JSON.stringify(SKILL_NAME)}, got ${JSON.stringify(name)}`,
    );
  }
  lines.forEach((line, i) => {
    if (GENERATED_MARKER.test(line)) {
      add("R7", `line ${i + 1}`, "generated/do-not-edit marker — the skill is written by hand");
    }
  });
  const routingSec = section(lines, ROUTING_HEADING);
  const recoverySec = section(lines, RECOVERY_HEADING);
  const examplesSec = section(lines, EXAMPLES_HEADING);
  for (const [heading, sec] of [
    [ROUTING_HEADING, routingSec],
    [RECOVERY_HEADING, recoverySec],
    [EXAMPLES_HEADING, examplesSec],
  ] as const) {
    if (sec === null) add("R7", heading, "required section missing");
  }

  // Routing table → rows.
  const routingRows: RoutingRow[] = [];
  if (routingSec !== null) {
    const table = tableRows(routingSec.body, routingSec.start);
    if (table === null) {
      add("R7", ROUTING_HEADING, "no table found");
    } else {
      if (ROUTING_COLUMNS.some((c, i) => table.header[i] !== c)) {
        add("R7", ROUTING_HEADING, `columns must be ${ROUTING_COLUMNS.join(" | ")}`);
      }
      for (const row of table.rows) routingRows.push(parseRouting(row.cells, row.line));
    }
  }

  // R1 / R2 / R3 — skill → schema direction, per row.
  const rowsByTool = new Map<string, RoutingRow[]>();
  for (const row of routingRows) {
    const where = `routing row line ${row.line}`;
    if (row.tool === null) {
      add(
        "R1",
        where,
        row.toolTokens.length === 0
          ? "names no tool in the Use column"
          : `names ${row.toolTokens.length} tools in one row (${row.toolTokens.join(", ")}); one tool per row`,
      );
      continue;
    }
    if (!universe.has(row.tool)) {
      add("R1", where, `unknown tool ${JSON.stringify(row.tool)}`);
      continue;
    }
    const list = rowsByTool.get(row.tool) ?? [];
    list.push(row);
    rowsByTool.set(row.tool, list);
    const tool = byName.get(row.tool);
    if (tool === undefined) {
      // schema-less built-in: nothing to select or pass.
      for (const f of row.facets) {
        add("R3", where, `${row.tool} has no arguments; ${f.facet}:${f.value} cannot be selected`);
      }
      for (const a of row.args)
        add("R3", where, `${row.tool} has no arguments; ${a} does not exist`);
      continue;
    }
    const enums = enumsOf(tool.schema);
    const props = propertiesOf(tool.schema);
    for (const f of row.facets) {
      const values = enums.get(f.facet);
      if (values === undefined) {
        add("R2", where, `${f.facet} is not an enum-typed argument of ${row.tool}`);
      } else if (!values.includes(f.value)) {
        add(
          "R2",
          where,
          `${f.facet}:${f.value} is not a value of ${row.tool}.${f.facet} (${values.join("|")})`,
        );
      }
    }
    for (const a of row.args) {
      if (!props.has(a)) add("R3", where, `${a} is not an argument of ${row.tool}`);
    }
  }

  // Worked examples → per tool, validated.
  const examples = examplesSec === null ? [] : parseExamples(examplesSec.body, examplesSec.start);
  const exampledTools = new Set<string>();
  const ajv = new Ajv2020({ allErrors: true });
  const validators = new Map<string, ReturnType<typeof ajv.compile>>();
  ajv.addKeyword("x-workbench-status");
  ajv.addKeyword("x-workbench-owned-by");
  for (const ex of examples) {
    const where = `example line ${ex.line}`;
    if (!universe.has(ex.tool)) {
      add("R1", where, `unknown tool ${JSON.stringify(ex.tool)}`);
      continue;
    }
    exampledTools.add(ex.tool);
    let parsed: unknown;
    try {
      parsed = JSON.parse(ex.body);
    } catch (error) {
      add("R5", where, `example for ${ex.tool} is not JSON: ${(error as Error).message}`);
      continue;
    }
    const tool = byName.get(ex.tool);
    if (tool === undefined) {
      if (JSON.stringify(parsed) !== "{}")
        add("R5", where, `${ex.tool} takes no arguments; example must be {}`);
      continue;
    }
    // One compiled validator per tool, reused: a tool may be exampled more
    // than once (a two-action tool needs one example per action), and ajv
    // refuses a second schema carrying the same $id.
    let validate = validators.get(ex.tool);
    if (validate === undefined) {
      validate = ajv.compile(structuredClone(tool.schema));
      validators.set(ex.tool, validate);
    }
    if (!validate(parsed)) {
      add("R5", where, `example for ${ex.tool} is invalid: ${ajv.errorsText(validate.errors)}`);
    }
  }

  // R1 / R2 / R3 — schema → skill direction (omission is drift).
  for (const toolName of [...universe].sort()) {
    const rows = rowsByTool.get(toolName) ?? [];
    if (rows.length === 0) add("R1", toolName, "no routing row routes this tool");
    if (!exampledTools.has(toolName)) add("R1", toolName, "no worked example for this tool");
    const tool = byName.get(toolName);
    if (tool === undefined) continue;
    const mentionedArgs = new Set<string>();
    const selected = new Set<string>();
    for (const row of rows) {
      for (const a of row.args) mentionedArgs.add(a);
      for (const f of row.facets) {
        mentionedArgs.add(f.facet);
        selected.add(`${f.facet}:${f.value}`);
      }
    }
    for (const [facet, values] of enumsOf(tool.schema)) {
      for (const value of values) {
        if (!selected.has(`${facet}:${value}`)) {
          add("R2", toolName, `${facet}:${value} appears in no routing row`);
        }
      }
    }
    for (const req of requiredOf(tool.schema)) {
      if (!mentionedArgs.has(req))
        add("R3", toolName, `required argument ${req} appears in no routing row`);
    }
  }

  // R4 — recovery table ↔ taxonomy, one-to-one.
  if (recoverySec !== null) {
    const table = tableRows(recoverySec.body, recoverySec.start);
    if (table === null) {
      add("R7", RECOVERY_HEADING, "no table found");
    } else {
      if (RECOVERY_COLUMNS.some((c, i) => table.header[i] !== c)) {
        add("R7", RECOVERY_HEADING, `columns must be ${RECOVERY_COLUMNS.join(" | ")}`);
      }
      const rows: RecoveryRow[] = table.rows.map((r) => ({
        line: r.line,
        code: backticked(r.cells[0] ?? "")[0] ?? null,
        recovery: backticked(r.cells[1] ?? "")[0] ?? null,
      }));
      const expected = new Map(input.taxonomy.map((t) => [t.code, t.recovery] as const));
      const seen = new Map<string, number>();
      for (const row of rows) {
        const where = `recovery row line ${row.line}`;
        if (row.code === null) {
          add("R4", where, "no backticked code in the Code column");
          continue;
        }
        seen.set(row.code, (seen.get(row.code) ?? 0) + 1);
        const want = expected.get(row.code);
        if (want === undefined) {
          add("R4", where, `unknown code ${row.code}`);
        } else if (row.recovery !== want) {
          add(
            "R4",
            where,
            `${row.code} recovery is ${row.recovery ?? "missing"}; taxonomy says ${want}`,
          );
        }
      }
      for (const [code, count] of seen) {
        if (count > 1) add("R4", code, `listed ${count} times; exactly once`);
      }
      for (const code of expected.keys()) {
        if (!seen.has(code)) add("R4", code, "missing from the recovery table");
      }
    }
  }

  // R6 — the dogfood copy, when present, is the same bytes.
  if (input.copy !== undefined && input.copy !== input.skill) {
    add("R6", DOGFOOD_COPY_PATH, `copy differs from canonical ${CANONICAL_SKILL_PATH}`);
  }

  return out.sort((a, b) =>
    a.rule === b.rule
      ? a.where === b.where
        ? a.message.localeCompare(b.message)
        : a.where.localeCompare(b.where)
      : a.rule.localeCompare(b.rule),
  );
}

/** Load the oracles from the repository — the single sources of truth. */
export async function loadOracles(
  repoRoot: string,
): Promise<{ tools: ToolSurface[]; taxonomy: TaxonomyEntry[] }> {
  const toolDir = `${repoRoot}/schemas/tools`;
  const suffix = ".input.schema.json";
  const names: string[] = [];
  for await (const file of new Bun.Glob(`*${suffix}`).scan(toolDir)) {
    names.push(file.slice(0, -suffix.length));
  }
  names.sort();
  const tools: ToolSurface[] = [];
  for (const name of names) {
    tools.push({
      name,
      schema: (await Bun.file(`${toolDir}/${name}${suffix}`).json()) as Record<string, unknown>,
    });
  }
  const taxonomyDoc = (await Bun.file(`${repoRoot}/schemas/error-taxonomy.json`).json()) as {
    codes: { code: string; recovery: string }[];
  };
  return {
    tools,
    taxonomy: taxonomyDoc.codes.map((c) => ({ code: c.code, recovery: c.recovery })),
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.some((a) => a.startsWith("--")) || args.length > 2) {
    console.error(
      "usage: bun run scripts/check-skill-drift.ts [<skill.md> [<copy.md>]]\n" +
        "  no options exist: this check only reads and never rewrites the skill",
    );
    process.exit(2);
  }
  const skillPath = args[0] ?? CANONICAL_SKILL_PATH;
  const copyPath = args[1] ?? (args[0] === undefined ? DOGFOOD_COPY_PATH : undefined);
  const skillFile = Bun.file(skillPath);
  if (!(await skillFile.exists())) {
    console.error(`skill-drift: ${skillPath} not found`);
    process.exit(2);
  }
  const copyFile = copyPath === undefined ? null : Bun.file(copyPath);
  const copy = copyFile !== null && (await copyFile.exists()) ? await copyFile.text() : undefined;
  const { tools, taxonomy } = await loadOracles(".");
  const violations = checkSkillDrift({
    skill: await skillFile.text(),
    tools,
    taxonomy,
    ...(copy === undefined ? {} : { copy }),
  });
  if (violations.length > 0) {
    console.error(`skill-drift: ${violations.length} violation(s)`);
    for (const v of violations) console.error(`  ${v.rule} ${v.where}: ${v.message}`);
    process.exit(1);
  }
  console.log(
    `skill-drift: ${skillPath} agrees with ${tools.length} tool schemas and ${taxonomy.length} codes`,
  );
}
