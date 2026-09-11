/**
 * The companion routing skill and its drift check (feature 006-routing-skill):
 *
 *  - The checker is a pure function: over synthetic inputs every rule fails
 *    in both directions with its named rule, a clean input yields nothing,
 *    and the output is deterministic.
 *  - The REAL skill agrees with the REAL schemas and taxonomy (zero
 *    violations), and every tool has a routing row and a worked example —
 *    asserted directly, not only through the checker.
 *  - Drift fixtures over copies of the real files (schema gains an enum
 *    value / a required argument / a tool; the skill cites an unknown
 *    argument, drops a tool, swaps a recovery class, breaks an example,
 *    carries a generated marker; the dogfood copy differs) each fail.
 *  - The CLI exits 0 / 1 / 2 as contracted and refuses every option.
 *  - Hand-written is structural: the checker's source has no write API.
 */

import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  CANONICAL_SKILL_PATH,
  checkSkillDrift,
  DOGFOOD_COPY_PATH,
  loadOracles,
  SCHEMALESS_TOOLS,
  type ToolSurface,
} from "../scripts/check-skill-drift";

const REPO = path.resolve(import.meta.dir, "..");
const CHECKER = path.join(REPO, "scripts", "check-skill-drift.ts");

// ---------------------------------------------------------------------------
// Synthetic inputs: two tools, three codes, one minimal valid skill.

const ALPHA: ToolSurface = {
  name: "alpha",
  schema: {
    type: "object",
    required: ["query"],
    properties: {
      query: { type: "string" },
      mode: { enum: ["a", "b"] },
      path: { type: "string" },
    },
  },
};
const BETA: ToolSurface = {
  name: "beta",
  schema: {
    type: "object",
    required: ["op"],
    properties: { op: { enum: ["x", "y"] }, to: { type: "string" } },
  },
};
const TOOLS = [ALPHA, BETA];
const TAXONOMY = [
  { code: "ONE", recovery: "retry" },
  { code: "TWO", recovery: "abort" },
  { code: "THREE", recovery: "narrow" },
];

function syntheticSkill(): string {
  return [
    "---",
    "name: workbench-routing",
    "description: test",
    "---",
    "",
    "# Test skill",
    "",
    "Prose mentioning the generated schemas is fine.",
    "",
    "## Routing table",
    "",
    "| Instead of | Use | Arguments that matter |",
    "|---|---|---|",
    "| grep | `alpha` `mode:a` | `query`, `path` |",
    "| grep -F | `alpha` `mode:b` | `query` |",
    "| mv | `beta` `op:x` | `to` |",
    "| cp | `beta` `op:y` | `to` |",
    "| status | `workbench_status` | — |",
    "",
    "## Recovery table",
    "",
    "| Code | Recovery | What to do |",
    "|---|---|---|",
    "| `ONE` | `retry` | wait |",
    "| `TWO` | `abort` | stop |",
    "| `THREE` | `narrow` | refine |",
    "",
    "## Worked examples",
    "",
    "```json workbench:alpha",
    '{ "query": "x", "mode": "a" }',
    "```",
    "",
    "```json workbench:beta",
    '{ "op": "x", "to": "y" }',
    "```",
    "",
    "```json workbench:workbench_status",
    "{}",
    "```",
    "",
  ].join("\n");
}

function rules(skill: string, tools = TOOLS, taxonomy = TAXONOMY, copy?: string): string[] {
  const violations = checkSkillDrift({
    skill,
    tools,
    taxonomy,
    ...(copy === undefined ? {} : { copy }),
  });
  return violations.map((v) => `${v.rule} ${v.where}: ${v.message}`);
}

describe("checkSkillDrift over synthetic inputs", () => {
  test("a clean skill yields zero violations, deterministically", () => {
    const skill = syntheticSkill();
    expect(rules(skill)).toEqual([]);
    expect(checkSkillDrift({ skill, tools: TOOLS, taxonomy: TAXONOMY })).toEqual(
      checkSkillDrift({ skill, tools: TOOLS, taxonomy: TAXONOMY }),
    );
  });

  test("R1 skill→schema: an unknown tool in a row or example", () => {
    const rowDrift = syntheticSkill().replace("| mv | `beta` `op:x`", "| mv | `gamma` `op:x`");
    expect(rules(rowDrift).some((r) => r.startsWith("R1") && r.includes("gamma"))).toBe(true);
    const exDrift = syntheticSkill().replace("```json workbench:beta", "```json workbench:gamma");
    expect(rules(exDrift).some((r) => r.startsWith("R1") && r.includes("gamma"))).toBe(true);
  });

  test("R1 schema→skill: a tool with no row or no example", () => {
    const gamma: ToolSurface = { name: "gamma", schema: { type: "object", properties: {} } };
    const out = rules(syntheticSkill(), [...TOOLS, gamma]);
    expect(out).toContain("R1 gamma: no routing row routes this tool");
    expect(out).toContain("R1 gamma: no worked example for this tool");
  });

  test("R1: a row naming two tools or none", () => {
    const two = syntheticSkill().replace("| mv | `beta` `op:x`", "| mv | `beta` `alpha` `op:x`");
    expect(rules(two).some((r) => r.startsWith("R1") && r.includes("2 tools"))).toBe(true);
    const none = syntheticSkill().replace(
      "| status | `workbench_status` |",
      "| status | nothing |",
    );
    expect(rules(none).some((r) => r.startsWith("R1") && r.includes("names no tool"))).toBe(true);
  });

  test("R2 both directions: unknown facet/value, and an unrouted enum value", () => {
    const bad = syntheticSkill().replace("`mode:a`", "`mode:zzz`");
    expect(rules(bad).some((r) => r.startsWith("R2") && r.includes("mode:zzz"))).toBe(true);
    const notEnum = syntheticSkill().replace("`mode:a`", "`path:a`");
    expect(
      rules(notEnum).some((r) => r.startsWith("R2") && r.includes("path is not an enum")),
    ).toBe(true);
    const widened: ToolSurface = {
      ...ALPHA,
      schema: {
        ...ALPHA.schema,
        properties: {
          ...(ALPHA.schema["properties"] as Record<string, unknown>),
          mode: { enum: ["a", "b", "c"] },
        },
      },
    };
    expect(rules(syntheticSkill(), [widened, BETA])).toContain(
      "R2 alpha: mode:c appears in no routing row",
    );
  });

  test("R3 both directions: unknown argument, and an unrouted required argument", () => {
    const unknown = syntheticSkill().replace("| `query`, `path` |", "| `qeury`, `path` |");
    expect(rules(unknown).some((r) => r.startsWith("R3") && r.includes("qeury"))).toBe(true);
    const stricter: ToolSurface = {
      ...BETA,
      schema: {
        ...BETA.schema,
        required: ["op", "to", "from"],
        properties: {
          ...(BETA.schema["properties"] as Record<string, unknown>),
          from: { type: "string" },
        },
      },
    };
    expect(rules(syntheticSkill(), [ALPHA, stricter])).toContain(
      "R3 beta: required argument from appears in no routing row",
    );
    // A facet name counts as mentioning the argument: `op:x` mentions `op`.
    expect(rules(syntheticSkill()).some((r) => r.includes("required argument op"))).toBe(false);
    // The schema-less built-in takes nothing.
    const statusArgs = syntheticSkill().replace(
      "| status | `workbench_status` | — |",
      "| status | `workbench_status` | `verbose` |",
    );
    expect(
      rules(statusArgs).some((r) => r.startsWith("R3") && r.includes("workbench_status")),
    ).toBe(true);
  });

  test("R4: missing, duplicate, unknown, and mis-classed codes", () => {
    const missing = syntheticSkill().replace("| `THREE` | `narrow` | refine |\n", "");
    expect(rules(missing)).toContain("R4 THREE: missing from the recovery table");
    const dup = syntheticSkill().replace(
      "| `THREE` | `narrow` | refine |",
      "| `THREE` | `narrow` | refine |\n| `THREE` | `narrow` | again |",
    );
    expect(rules(dup)).toContain("R4 THREE: listed 2 times; exactly once");
    const unknown = syntheticSkill().replace(
      "| `THREE` | `narrow` | refine |",
      "| `THREE` | `narrow` | refine |\n| `FOUR` | `abort` | ? |",
    );
    expect(rules(unknown).some((r) => r.startsWith("R4") && r.includes("unknown code FOUR"))).toBe(
      true,
    );
    const wrong = syntheticSkill().replace("| `ONE` | `retry` |", "| `ONE` | `abort` |");
    expect(rules(wrong).some((r) => r.startsWith("R4") && r.includes("taxonomy says retry"))).toBe(
      true,
    );
  });

  test("R5: invalid JSON, schema-invalid example, non-empty status example", () => {
    const notJson = syntheticSkill().replace('{ "query": "x", "mode": "a" }', "{ query: x }");
    expect(rules(notJson).some((r) => r.startsWith("R5") && r.includes("not JSON"))).toBe(true);
    const invalid = syntheticSkill().replace('{ "query": "x", "mode": "a" }', '{ "mode": "zzz" }');
    expect(rules(invalid).some((r) => r.startsWith("R5") && r.includes("alpha is invalid"))).toBe(
      true,
    );
    const status = syntheticSkill().replace(
      "```json workbench:workbench_status\n{}",
      '```json workbench:workbench_status\n{ "x": 1 }',
    );
    expect(rules(status).some((r) => r.startsWith("R5") && r.includes("must be {}"))).toBe(true);
  });

  test("R5: a tool exampled twice is validated twice, not refused as a duplicate", () => {
    // A two-action tool needs an example per action; ajv rejects a second
    // schema with the same $id, so the checker must reuse one validator.
    const twice = syntheticSkill().replace(
      '```json workbench:beta\n{ "op": "x", "to": "y" }\n```',
      '```json workbench:beta\n{ "op": "x", "to": "y" }\n```\n\n' +
        '```json workbench:beta\n{ "op": "y", "to": "z" }\n```',
    );
    expect(rules(twice)).toEqual([]);
    const second = syntheticSkill().replace(
      '```json workbench:beta\n{ "op": "x", "to": "y" }\n```',
      '```json workbench:beta\n{ "op": "x", "to": "y" }\n```\n\n' +
        '```json workbench:beta\n{ "op": "nope" }\n```',
    );
    expect(rules(second).some((r) => r.startsWith("R5") && r.includes("beta is invalid"))).toBe(
      true,
    );
  });

  test("R6: the dogfood copy must be the same bytes", () => {
    const skill = syntheticSkill();
    expect(rules(skill, TOOLS, TAXONOMY, skill)).toEqual([]);
    expect(rules(skill, TOOLS, TAXONOMY, `${skill} `).some((r) => r.startsWith("R6"))).toBe(true);
  });

  test("R7: generated marker, wrong frontmatter name, missing section, wrong columns", () => {
    const marked = `${syntheticSkill()}\n<!-- generated: do not edit -->\n`;
    expect(rules(marked).some((r) => r.startsWith("R7") && r.includes("generated"))).toBe(true);
    const upper = syntheticSkill().replace("# Test skill", "# Test skill\nDO NOT EDIT");
    expect(rules(upper).some((r) => r.startsWith("R7"))).toBe(true);
    const renamed = syntheticSkill().replace("name: workbench-routing", "name: other");
    expect(rules(renamed).some((r) => r.startsWith("R7") && r.includes("frontmatter"))).toBe(true);
    const noRecovery = syntheticSkill().replace("## Recovery table", "## Recovery");
    expect(rules(noRecovery)).toContain("R7 ## Recovery table: required section missing");
    const columns = syntheticSkill().replace(
      "| Instead of | Use | Arguments that matter |",
      "| When | Tool | Args |",
    );
    expect(rules(columns).some((r) => r.startsWith("R7") && r.includes("columns must be"))).toBe(
      true,
    );
  });
});

// ---------------------------------------------------------------------------
// The real files.

async function real(): Promise<{
  skill: string;
  copy: string | undefined;
  tools: ToolSurface[];
  taxonomy: { code: string; recovery: string }[];
}> {
  const skill = readFileSync(path.join(REPO, CANONICAL_SKILL_PATH), "utf8");
  const copyPath = path.join(REPO, DOGFOOD_COPY_PATH);
  const copyFile = Bun.file(copyPath);
  const copy = (await copyFile.exists()) ? readFileSync(copyPath, "utf8") : undefined;
  const { tools, taxonomy } = await loadOracles(REPO);
  return { skill, copy, tools, taxonomy };
}

describe("the real skill against the real tool surface", () => {
  test("zero violations; every tool routed and exampled (SC-001)", async () => {
    const { skill, copy, tools, taxonomy } = await real();
    expect(tools.map((t) => t.name)).toEqual([
      "edit",
      "files",
      "map",
      "read",
      "run",
      "script",
      "search",
      "undo",
    ]);
    expect(taxonomy).toHaveLength(24);
    expect(
      checkSkillDrift({ skill, tools, taxonomy, ...(copy === undefined ? {} : { copy }) }),
    ).toEqual([]);
    // Direct assertions, independent of the checker's own parsing.
    for (const name of [...tools.map((t) => t.name), ...SCHEMALESS_TOOLS]) {
      expect(skill.includes(`| \`${name}\``) ? "" : `no routing row for ${name}`).toBe("");
      expect(skill.includes(`\`\`\`json workbench:${name}\n`) ? "" : `no example for ${name}`).toBe(
        "",
      );
    }
    for (const entry of taxonomy) {
      expect(
        skill.includes(`| \`${entry.code}\` | \`${entry.recovery}\` |`)
          ? ""
          : `no row for ${entry.code}`,
      ).toBe("");
    }
  });

  test("the dogfood copy exists and is byte-identical (R6)", async () => {
    const { skill, copy } = await real();
    expect(copy).toBeDefined();
    expect(copy).toBe(skill);
  });

  test("drift fixtures over the real files fail with the named rule (SC-002)", async () => {
    const { skill, tools, taxonomy } = await real();
    const out = (s: string, t: ToolSurface[] = tools) =>
      checkSkillDrift({ skill: s, tools: t, taxonomy }).map(
        (v) => `${v.rule} ${v.where}: ${v.message}`,
      );

    // Schema gains a search mode the skill does not route.
    const search = tools.find((t) => t.name === "search");
    if (search === undefined) throw new Error("unreachable");
    const widened = structuredClone(search.schema) as { properties: { mode: { enum: string[] } } };
    widened.properties.mode.enum.push("zzz");
    expect(
      out(
        skill,
        tools.map((t) => (t.name === "search" ? { name: "search", schema: widened } : t)),
      ),
    ).toContain("R2 search: mode:zzz appears in no routing row");
    // Schema gains a required argument on read.
    const read = tools.find((t) => t.name === "read");
    if (read === undefined) throw new Error("unreachable");
    const stricter = structuredClone(read.schema) as {
      required: string[];
      properties: Record<string, unknown>;
    };
    stricter.required.push("zzz");
    stricter.properties["zzz"] = { type: "string" };
    expect(
      out(
        skill,
        tools.map((t) => (t.name === "read" ? { name: "read", schema: stricter } : t)),
      ),
    ).toContain("R3 read: required argument zzz appears in no routing row");
    // A new tool appears.
    expect(
      out(skill, [...tools, { name: "gizmo", schema: { type: "object", properties: {} } }]),
    ).toContain("R1 gizmo: no routing row routes this tool");
    // The skill cites an unknown argument.
    expect(
      out(
        skill.replace("| `query`, `path`, `context`, `include`, `budget` |", "| `qeury`, `path` |"),
      ).some((r) => r.startsWith("R3") && r.includes("qeury")),
    ).toBe(true);
    // The skill drops every undo row.
    const noUndo = skill
      .split("\n")
      .filter((l) => !l.includes("| `undo`"))
      .join("\n");
    expect(out(noUndo)).toContain("R1 undo: no routing row routes this tool");
    // Wrong recovery class.
    expect(
      out(
        skill.replace("| `INDEX_UNAVAILABLE` | `retry` |", "| `INDEX_UNAVAILABLE` | `abort` |"),
      ).some((r) => r.startsWith("R4") && r.includes("taxonomy says retry")),
    ).toBe(true);
    // Invalid example.
    expect(
      out(skill.replace('"mode": "references"', '"mode": "semantics"')).some(
        (r) => r.startsWith("R5") && r.includes("search is invalid"),
      ),
    ).toBe(true);
    // Generated marker.
    expect(
      out(`${skill}\n<!-- generated by a tool; do not edit -->\n`).some((r) => r.startsWith("R7")),
    ).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// The CLI and the hand-written guard.

describe("lint:skill CLI", () => {
  test("exit 0 on the repository; 1 on a drifted skill; 2 on any option", async () => {
    const clean = spawnSync(process.execPath, ["run", CHECKER], { cwd: REPO, encoding: "utf8" });
    expect(clean.status).toBe(0);

    const dir = mkdtempSync(path.join(tmpdir(), "workbench-skill-"));
    const drifted = path.join(dir, "SKILL.md");
    const { skill } = await real();
    writeFileSync(
      drifted,
      skill.replace("| `INDEX_UNAVAILABLE` | `retry` |", "| `INDEX_UNAVAILABLE` | `abort` |"),
    );
    const red = spawnSync(process.execPath, ["run", CHECKER, drifted], {
      cwd: REPO,
      encoding: "utf8",
    });
    expect(red.status).toBe(1);
    expect(red.stderr).toContain("R4");

    const usage = spawnSync(process.execPath, ["run", CHECKER, "--fix"], {
      cwd: REPO,
      encoding: "utf8",
    });
    expect(usage.status).toBe(2);
    expect(usage.stderr).toContain("never rewrites");
  }, 60_000);

  test("the checker's source has no write capability (FR-007)", () => {
    const source = readFileSync(CHECKER, "utf8");
    for (const pattern of [
      /\bwriteFile\b/,
      /\bwriteFileSync\b/,
      /\bappendFile\b/,
      /Bun\.write\b/,
      /--fix/,
      /--write/,
    ]) {
      expect(pattern.test(source) ? `checker source matches ${pattern}` : "").toBe("");
    }
  });
});
