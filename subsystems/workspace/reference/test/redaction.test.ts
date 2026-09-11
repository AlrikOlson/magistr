/**
 * Secret redaction at the envelope choke point (SDD §7.1): every named
 * pattern marks its kind, the entropy heuristic catches opaque tokens
 * without mangling hashes/paths/prose, and sealEnvelope keeps the wire
 * bytes and the structured content identical by construction.
 */

import { describe, expect, test } from "bun:test";
import {
  okEnvelope,
  redactEnvelope,
  redactText,
  sealEnvelope,
  shannonEntropy,
  taxonomyError,
} from "../src/envelope";

describe("named credential patterns", () => {
  const cases: readonly [kind: string, secret: string][] = [
    ["aws-key", "AKIAIOSFODNN7EXAMPLE"],
    ["github-token", "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef1234"],
    ["github-token", "github_pat_11ABCDEFG0123456789_abcdefghij"],
    ["slack-token", "xoxb-2444333222111-0123456789012-AbCdEfGhIjKl"],
    ["anthropic-key", "sk-ant-api03-Zq8vR3kXw1Ny7Jt2Bf9Lm4Ps6Ud0Hc"],
    ["openai-key", "sk-proj4Zq8vR3kXw1Ny7Jt2Bf9L"],
    ["google-api-key", "AIzaSyA1bC2dE3fG4hI5jK6lM7nO8pQ9rS0tUvW"],
    [
      "jwt",
      "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
    ],
  ];

  for (const [kind, secret] of cases) {
    test(`${kind} is replaced by a marked token`, () => {
      const { text, kinds } = redactText(`before ${secret} after`);
      expect(text).not.toContain(secret);
      expect(text).toContain(`[REDACTED:${kind}]`);
      expect(text).toStartWith("before ");
      expect(text).toEndWith(" after");
      expect(kinds).toContain(kind);
    });
  }

  test("an unterminated private key block is still caught", () => {
    const { text } = redactText("-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0B");
    expect(text).not.toContain("MIIEvQIBADANBgkqhkiG9w0B");
    expect(text).toContain("[REDACTED:private-key]");
  });

  test("credential assignments keep the key name, redact only the value", () => {
    const { text } = redactText('password = "correct-horse-battery"');
    expect(text).toContain("password");
    expect(text).not.toContain("correct-horse-battery");
    expect(text).toContain("[REDACTED:credential-assignment]");
  });
});

describe("the high-entropy heuristic", () => {
  test("shannonEntropy is 0 for a constant string and high for a diverse one", () => {
    expect(shannonEntropy("aaaaaaaa")).toBe(0);
    expect(shannonEntropy("abcdefghijklmnop")).toBe(4);
  });

  test("an opaque 48-char token is redacted as high-entropy", () => {
    const token = "Zq8vR3kXw1Ny7Jt2Bf9Lm4Ps6Ud0Hc5Ga_TeQiYoWxEnKrVb";
    const { text } = redactText(`blob ${token} end`);
    expect(text).not.toContain(token);
    expect(text).toContain("[REDACTED:high-entropy]");
  });

  test("a 40-char git SHA survives — pure hex is data, not a secret", () => {
    const sha = "356a192b7913b04c54574d18c28d46e6395428ab";
    expect(redactText(`commit ${sha}`).text).toContain(sha);
  });

  test("file paths survive — separators exclude a token from candidacy", () => {
    const p = "/home/runner/work/workbench-mcp/workbench-mcp/src/envelope/redact.ts";
    expect(redactText(p).text).toBe(p);
  });

  test("dotted names survive — versions, domains, file names", () => {
    const s = "registry.example.com/some-organization-name/averylongpackagename.1.2.3";
    expect(redactText(s).text).toBe(s);
  });

  test("long but low-entropy text survives", () => {
    const s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    expect(redactText(s).text).toBe(s);
  });

  test("ordinary prose is untouched and reports no kinds", () => {
    const s = "The ranking pipeline orders declarations before calls.";
    const out = redactText(s);
    expect(out.text).toBe(s);
    expect(out.kinds).toEqual([]);
  });
});

describe("envelope-level redaction", () => {
  const BUDGET = { requested: 4, used: 2 } as const;
  const AWS = "AKIAIOSFODNN7EXAMPLE";

  test("results are deep-walked: nested objects and arrays", () => {
    const envelope = okEnvelope({
      v: 1,
      freshness: "warm",
      budget: BUDGET,
      results: [{ hits: [{ line: `key ${AWS}` }] }],
    });
    const redacted = redactEnvelope(envelope);
    expect(JSON.stringify(redacted)).not.toContain(AWS);
    expect(JSON.stringify(redacted)).toContain("[REDACTED:aws-key]");
  });

  test("notes and error payloads are covered", () => {
    const error = taxonomyError({
      v: 1,
      code: "PARSE_REJECTED",
      message: `parser saw ${AWS}`,
      parserMessage: "unexpected token",
      rejectedDiff: `+${AWS}`,
      notes: [`note ${AWS}`],
    });
    const json = JSON.stringify(redactEnvelope(error));
    expect(json).not.toContain(AWS);
  });

  test("non-string values are preserved exactly", () => {
    const envelope = okEnvelope({
      v: 1,
      freshness: "cold",
      budget: BUDGET,
      results: [{ n: 42, flag: true, nothing: null }],
    });
    expect(redactEnvelope(envelope)).toEqual(envelope);
  });

  test("sealEnvelope: json is exactly the serialized redacted envelope", () => {
    const sealed = sealEnvelope(
      okEnvelope({ v: 1, freshness: "warm", budget: BUDGET, results: [AWS] }),
    );
    expect(sealed.json).toBe(JSON.stringify(sealed.envelope));
    expect(sealed.json).toContain("[REDACTED:aws-key]");
  });

  test("redaction is deterministic — statelessness holds through the seal", () => {
    const make = () =>
      sealEnvelope(okEnvelope({ v: 1, freshness: "warm", budget: BUDGET, results: [AWS, "plain"] }))
        .json;
    expect(make()).toBe(make());
  });
});
