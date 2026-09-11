/** The config subsystem (SDD §7.2): five layers, tighten-only security lattice. */

export type { DetectedCommand } from "./detect";
export { detectCommands, renderCommandsTable } from "./detect";
export { renderDoctorConfig } from "./doctor";
export type {
  LayerReport,
  PolicyViolation,
  ResolveConfigOptions,
  ResolvedConfig,
  ResolvedKey,
} from "./resolve";
export {
  configNotes,
  defaultResolvedConfig,
  ORG_POLICY_ENV,
  parseSessionFlags,
  resolveConfig,
  USER_CONFIG_ENV,
} from "./resolve";
export type { ConfigLayerName, ConfigValue, KeySpec } from "./schema";
export { CONFIG_KEYS, CONFIG_VERSION, isSecuritySpec, LAYER_ORDER } from "./schema";
