/**
 * The config key registry (SDD §7.2): every key workbench understands, its
 * shape, its default, and — for security-relevant keys — which direction is
 * "tighter". The registry is the single source of truth the resolver merges
 * against: a key absent here is unknown (warn, never abort), and a key's
 * lattice behavior is data, not code sprinkled through the resolver.
 *
 * Two kinds of key never appear as plain entries:
 *
 * - `config_version` — per-file format version (§10.2). Recognized in every
 *   layer file; a file declaring a newer version than this binary supports
 *   fails closed (`UNSUPPORTED_VERSION`), like every durable format.
 * - `[commands]` — the `run` allowlist table (§9.6). Dynamic child keys
 *   (command names), so it is special-cased: org policy defining the table
 *   pins it (a centrally pinned allowlist, enforceable across a team);
 *   otherwise entries merge per-name, nearest-wins.
 */

/** The supported config file format version (§10.2). */
export const CONFIG_VERSION = 1;

/** Layer names, lowest precedence first (SDD §7.2). */
export const LAYER_ORDER = ["defaults", "org-policy", "user", "workspace", "session"] as const;

export type ConfigLayerName = (typeof LAYER_ORDER)[number];

/** A value a config key can hold. */
export type ConfigValue = string | number | boolean | readonly string[];

export type KeySpec =
  | {
      readonly kind: "boolean";
      readonly default: boolean;
      /**
       * Present on security-relevant booleans: the value that is "tighter".
       * Once a layer above defaults explicitly decides the key, later layers
       * may move toward this value but never away from it.
       */
      readonly tighter?: boolean;
    }
  | { readonly kind: "number"; readonly default: number }
  | {
      /**
       * Ordered enum, tightest level first. `capped` marks it security:
       * an explicit earlier setting caps later layers at that index.
       */
      readonly kind: "enum";
      readonly levels: readonly string[];
      readonly default: string;
      readonly capped?: true;
    }
  | {
      /**
       * String list. `additive` marks it security: layers union, so
       * loosening (removing an earlier layer's entry) is inexpressible.
       */
      readonly kind: "string-list";
      readonly default: readonly string[];
      readonly additive?: true;
    };

/**
 * Every known dotted key. Trace and doctor output follow this order, so it
 * is part of the deterministic-notes contract (Constitution V).
 */
export const CONFIG_KEYS: Readonly<Record<string, KeySpec>> = {
  // Security lattice keys (§7.2): tighten-only downward.
  "security.redaction": { kind: "boolean", default: true, tighter: true },
  "security.allow_sideload": { kind: "boolean", default: true, tighter: false },
  "security.protected_paths": { kind: "string-list", default: [], additive: true },
  "telemetry.consent": {
    kind: "enum",
    levels: ["off", "crash", "full"],
    default: "off",
    capped: true,
  },
  "edit.force_confirm": { kind: "boolean", default: false, tighter: true },
  // How a confirm-gated write asks (SDD §9.4). `elicit`: an MRTR-capable
  // client gets the in-call dialog (the human approves). `token`: always the
  // two-call CONFIRM_REQUIRED + confirmToken flow — the AGENT confirms, the
  // diff still rides the envelope, nobody is interrupted. Nearest-wins and
  // deliberately not a security-lattice key: org policy must not be able to
  // force dialogs onto a delegated session (nor silence them — force_confirm
  // is the tighten-only knob).
  "edit.confirm_mode": { kind: "enum", levels: ["elicit", "token"], default: "elicit" },
  // Non-security keys: nearest-wins.
  "budgets.default": { kind: "number", default: 4000 },
  "format.enabled": { kind: "boolean", default: true },
  // Deliberately NOT a security-lattice key (feature 005, research R5): the
  // privacy boundary is the tracer's non-configurable attribute allowlist;
  // capping this key would let org policy force tracing ON.
  "tracing.enabled": { kind: "boolean", default: true },
  // Operational, nearest-wins (feature 011, research R2): `auto` picks the
  // polling adapter on a detected network filesystem (native watching lies
  // there) and fs.watch otherwise; `off` wires nothing.
  "watcher.mode": {
    kind: "enum",
    levels: ["auto", "native", "poll", "off"],
    default: "auto",
  },
};

/** Whether a spec participates in the tighten-only security lattice. */
export function isSecuritySpec(spec: KeySpec): boolean {
  switch (spec.kind) {
    case "boolean":
      return spec.tighter !== undefined;
    case "enum":
      return spec.capped === true;
    case "string-list":
      return spec.additive === true;
    case "number":
      return false;
  }
}
