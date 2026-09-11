/**
 * Red-team jail chapter (SDD §7.1, release-blocking): ≥25 escape/injection
 * attempts, every one of which must fail — symlink loops, `..` encodings,
 * git-hook writes, config self-edit, redaction bypass. A single success
 * blocks release.
 *
 * Two grades of assertion, both attack failures:
 *  - DENIED: the jail refuses with the expected §4.3 code.
 *  - CONTAINED: the candidate was never a traversal on this platform (e.g.
 *    encoded dots are not decoded — correctly), so it canonicalizes to an
 *    ordinary path *inside* the root. The attack still failed to escape.
 *
 * Symlink attempts create real links (junctions on Windows, where they need
 * no privilege for directories); if the environment refuses link creation
 * they are skipped — the chapter still runs ≥25 attempts without them, and
 * the final test enforces that count over *executed* attempts only.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { okEnvelope, sealEnvelope, taxonomyError } from "../src/envelope";
import { canonicalizeWithin, checkRead, checkWrite } from "../src/jail";

const root = mkdtempSync(path.join(tmpdir(), "wb-redteam-root-"));
const outside = mkdtempSync(path.join(tmpdir(), "wb-redteam-out-"));

mkdirSync(path.join(root, ".git", "hooks"), { recursive: true });
writeFileSync(path.join(root, ".git", "config"), "[core]\n");
writeFileSync(path.join(root, ".git", "HEAD"), "ref: refs/heads/main\n");
mkdirSync(path.join(root, ".workbench", "keys"), { recursive: true });
writeFileSync(path.join(root, ".workbench", "keys", "workspace.key"), Buffer.alloc(32, 7));
writeFileSync(path.join(root, ".workbench", "server.lock"), "{}");
writeFileSync(path.join(root, "workbench.toml"), "[run]\nallow = []\n");
mkdirSync(path.join(root, "src"), { recursive: true });
writeFileSync(path.join(root, "src", "app.ts"), "export {};\n");
mkdirSync(path.join(root, "a", "b", "c"), { recursive: true });
writeFileSync(path.join(outside, "secret.txt"), "you should never reach me\n");

// Directory links: junctions on Windows need no privilege; plain symlinks
// elsewhere. If the environment refuses, the guarded attempts are skipped.
let symlinksAvailable = false;
try {
  symlinkSync(outside, path.join(root, "lnk-out"), "junction");
  symlinkSync(path.join(root, ".git"), path.join(root, "wormhole"), "junction");
  symlinkSync(path.join(root, "loop-b"), path.join(root, "loop-a"), "junction");
  symlinkSync(path.join(root, "loop-a"), path.join(root, "loop-b"), "junction");
  symlinksAvailable = true;
} catch {
  symlinksAvailable = false;
}

afterAll(() => {
  rmSync(root, { recursive: true, force: true });
  rmSync(outside, { recursive: true, force: true });
});

const rootDecision = canonicalizeWithin(root, ".");
const rootReal = rootDecision.ok ? rootDecision.path : root;

function fold(p: string): string {
  return p.toLowerCase();
}

function insideRoot(p: string): boolean {
  return fold(p) === fold(rootReal) || fold(p).startsWith(fold(rootReal + path.sep));
}

/** The attempt must be refused with PATH_OUTSIDE_WORKSPACE. */
function deniedOutside(candidate: string): void {
  const decision = canonicalizeWithin(root, candidate);
  expect(decision.ok).toBe(false);
  if (!decision.ok) expect(decision.code).toBe("PATH_OUTSIDE_WORKSPACE");
}

/** The attempt must not escape: denied, or canonicalized to inside the root. */
function noEscape(candidate: string): void {
  const decision = canonicalizeWithin(root, candidate);
  if (decision.ok) {
    expect(insideRoot(decision.path)).toBe(true);
    expect(fold(decision.path).startsWith(fold(outside))).toBe(false);
  } else {
    expect(decision.code).toBe("PATH_OUTSIDE_WORKSPACE");
  }
}

/** The WRITE must be refused with the given code. */
function writeDenied(candidate: string, code: "PATH_OUTSIDE_WORKSPACE" | "PROTECTED_PATH"): void {
  const decision = checkWrite(root, candidate);
  expect(decision.ok).toBe(false);
  if (!decision.ok) expect(decision.code).toBe(code);
}

interface Attempt {
  readonly family: string;
  readonly name: string;
  readonly guarded?: boolean;
  readonly run: () => void;
}

const attempts: Attempt[] = [];
function attempt(family: string, name: string, run: () => void, guarded = false): void {
  attempts.push({ family, name, run, guarded });
}

// ---- escape family -------------------------------------------------------

attempt("escape", "plain parent traversal ../", () => deniedOutside("../escape.txt"));
attempt("escape", "bare .. to the root's parent", () => deniedOutside(".."));
attempt("escape", "traversal through a real subdir a/../../", () =>
  deniedOutside("a/../../escape.txt"),
);
attempt("escape", "deep traversal a/b/c/../../../../", () =>
  deniedOutside("a/b/c/../../../../escape.txt"),
);
attempt("escape", "absolute path outside the workspace", () =>
  deniedOutside(path.join(outside, "secret.txt")),
);
attempt("escape", "absolute path to the root's parent directory", () =>
  deniedOutside(path.dirname(root)),
);
attempt("escape", "sibling directory sharing the root as a name prefix", () =>
  deniedOutside(`${root}-evil${path.sep}escape.txt`),
);
attempt("escape", "UNC / network-share path (forward-slash form)", () =>
  deniedOutside("//evil/share/escape.txt"),
);
attempt("escape", "UNC / network-share path (backslash form)", () =>
  noEscape("\\\\evil\\share\\escape.txt"),
);
attempt("escape", "NUL byte injection", () => deniedOutside("src\0.ts"));
attempt("escape", "backslash parent traversal ..\\", () => noEscape("..\\escape.txt"));
attempt("escape", "quadruple-dot smuggling ....// stays a filename", () =>
  noEscape("....//escape.txt"),
);
attempt("escape", "URL-encoded dots %2e%2e are not decoded", () =>
  noEscape("%2e%2e/%2e%2e/escape.txt"),
);
attempt("escape", "URL-encoded separator ..%2f is not decoded", () =>
  noEscape("..%2f..%2fescape.txt"),
);
attempt("escape", "fullwidth unicode dots ．． are not dots", () => noEscape("．．/escape.txt"));
attempt("escape", "drive-relative Windows path c:evil", () => noEscape("c:evil.txt"));

// ---- symlink family (guarded: needs link-creation privilege) --------------

attempt(
  "symlink",
  "directory link pointing outside the root",
  () => deniedOutside(`lnk-out${path.sep}secret.txt`),
  true,
);
attempt(
  "symlink",
  "symlink loop a->b->a neither hangs nor escapes",
  () => noEscape(`loop-a${path.sep}x.txt`),
  true,
);
attempt(
  "symlink",
  "link inside the root aimed at .git as a write wormhole",
  () => writeDenied(`wormhole${path.sep}config`, "PROTECTED_PATH"),
  true,
);

// ---- protected-path family (git-hook writes, config/key self-edit) --------

attempt("protected", "git hook plant: .git/hooks/pre-commit", () =>
  writeDenied(".git/hooks/pre-commit", "PROTECTED_PATH"),
);
attempt("protected", "git config rewrite: .git/config", () =>
  writeDenied(".git/config", "PROTECTED_PATH"),
);
attempt("protected", "HEAD swap: .git/HEAD", () => writeDenied(".git/HEAD", "PROTECTED_PATH"));
attempt("protected", "hook plant via traversal src/../.git/hooks/post-merge", () =>
  writeDenied("src/../.git/hooks/post-merge", "PROTECTED_PATH"),
);
attempt("protected", "case-variant .GIT/config", () =>
  writeDenied(".GIT/config", "PROTECTED_PATH"),
);
attempt("protected", "MRTR key theft-by-overwrite: .workbench/keys/workspace.key", () =>
  writeDenied(".workbench/keys/workspace.key", "PROTECTED_PATH"),
);
attempt("protected", "index poisoning: .workbench/workspace.db", () =>
  writeDenied(".workbench/workspace.db", "PROTECTED_PATH"),
);
attempt("protected", "lock tamper: .workbench/server.lock", () =>
  writeDenied(".workbench/server.lock", "PROTECTED_PATH"),
);
attempt("protected", "run-allowlist self-amendment: workbench.toml", () =>
  writeDenied("workbench.toml", "PROTECTED_PATH"),
);
attempt("protected", "case-variant WORKBENCH.TOML", () =>
  writeDenied("WORKBENCH.TOML", "PROTECTED_PATH"),
);
attempt("protected", "the .git directory itself", () => writeDenied(".git", "PROTECTED_PATH"));
attempt("protected", "the .workbench directory itself", () =>
  writeDenied(".workbench", "PROTECTED_PATH"),
);

// ---- redaction-bypass family ----------------------------------------------

const AWS_KEY = "AKIAIOSFODNN7EXAMPLE";
const GITHUB_TOKEN = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef1234";
const ANTHROPIC_KEY = "sk-ant-api03-Zq8vR3kXw1Ny7Jt2Bf9Lm4Ps6Ud0Hc";
const HIGH_ENTROPY = "Zq8vR3kXw1Ny7Jt2Bf9Lm4Ps6Ud0Hc5Ga_TeQiYoWxEnKrVb";
const PEM_BODY = "MIIEowIBAAKCAQEA7bq0WonderfullySecretMaterial";
const PEM = `-----BEGIN RSA PRIVATE KEY-----\n${PEM_BODY}\n-----END RSA PRIVATE KEY-----`;

const BUDGET = { requested: 1, used: 1 } as const;

attempt("redaction", "AWS key smuggled in a nested result", () => {
  const sealed = sealEnvelope(
    okEnvelope({
      v: 1,
      freshness: "warm",
      budget: BUDGET,
      results: [{ file: "config.ts", snippet: `const k = "${AWS_KEY}";` }],
    }),
  );
  expect(sealed.json).not.toContain(AWS_KEY);
  expect(sealed.json).toContain("[REDACTED:aws-key]");
});

attempt("redaction", "GitHub token smuggled through envelope notes", () => {
  const sealed = sealEnvelope(
    okEnvelope({
      v: 1,
      freshness: "cold",
      budget: BUDGET,
      results: [],
      notes: [`ci says: ${GITHUB_TOKEN}`],
    }),
  );
  expect(sealed.json).not.toContain(GITHUB_TOKEN);
  expect(sealed.json).toContain("[REDACTED:github-token]");
});

attempt("redaction", "private key smuggled through an error payload diff", () => {
  const sealed = sealEnvelope(
    taxonomyError({
      v: 1,
      code: "PARSE_REJECTED",
      message: "syntax error introduced",
      parserMessage: "unexpected token",
      rejectedDiff: `+const pem = \`${PEM}\`;`,
    }),
  );
  expect(sealed.json).not.toContain(PEM_BODY);
  expect(sealed.json).toContain("[REDACTED:private-key]");
});

attempt("redaction", "unknown high-entropy token caught by the heuristic", () => {
  const sealed = sealEnvelope(
    okEnvelope({ v: 1, freshness: "warm", budget: BUDGET, results: [{ blob: HIGH_ENTROPY }] }),
  );
  expect(sealed.json).not.toContain(HIGH_ENTROPY);
  expect(sealed.json).toContain("[REDACTED:high-entropy]");
});

attempt("redaction", "anthropic key is labeled anthropic, not mislabeled openai", () => {
  const sealed = sealEnvelope(
    okEnvelope({ v: 1, freshness: "warm", budget: BUDGET, results: [ANTHROPIC_KEY] }),
  );
  expect(sealed.json).not.toContain(ANTHROPIC_KEY);
  expect(sealed.json).toContain("[REDACTED:anthropic-key]");
  expect(sealed.json).not.toContain("[REDACTED:openai-key]");
});

attempt("redaction", "redaction marks, never silently drops", () => {
  const sealed = sealEnvelope(
    okEnvelope({
      v: 1,
      freshness: "warm",
      budget: BUDGET,
      results: [`deploy log start token = abc123SECRETxyz9 end of log`],
    }),
  );
  expect(sealed.json).not.toContain("abc123SECRETxyz9");
  expect(sealed.json).toContain("[REDACTED:credential-assignment]");
  // The surrounding evidence survives — the model can see something was there.
  expect(sealed.json).toContain("deploy log start");
  expect(sealed.json).toContain("end of log");
});

// ---- the chapter -----------------------------------------------------------

let executed = 0;

describe("red-team jail chapter (SDD §7.1)", () => {
  for (const a of attempts) {
    test.skipIf(a.guarded === true && !symlinksAvailable)(`${a.family}: ${a.name}`, () => {
      a.run();
      executed++;
    });
  }

  test("sanity: legitimate work is not jailed", () => {
    const write = checkWrite(root, "src/app.ts");
    expect(write.ok).toBe(true);
    const read = checkRead(root, ".git/config");
    expect(read.ok).toBe(true);
  });

  test("the chapter executed >= 25 attempts", () => {
    expect(executed).toBeGreaterThanOrEqual(25);
  });
});
