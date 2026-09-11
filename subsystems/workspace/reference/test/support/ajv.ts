/**
 * One strict-mode Ajv (draft 2020-12) for envelope tests, with the repo's
 * `x-workbench-*` annotation keywords registered so strict mode stays on.
 */
import { Ajv2020 } from "ajv/dist/2020.js";

export function envelopeValidatorFor(schema: object) {
  const ajv = new Ajv2020({ allErrors: true });
  ajv.addKeyword("x-workbench-status");
  ajv.addKeyword("x-workbench-owned-by");
  const validate = ajv.compile(structuredClone(schema) as Record<string, unknown>);
  return {
    validate,
    errorsText: () => ajv.errorsText(validate.errors),
  };
}
