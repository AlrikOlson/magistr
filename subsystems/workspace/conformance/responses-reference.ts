/** Shared policy-response corpus through the pinned runtime, without starting an owner. */
import assert from "node:assert/strict";
import {
  isErrorCode,
  negotiateEnvelopeVersion,
  okEnvelope,
  redactText,
  sealEnvelope,
  taxonomyError,
  truncateResults,
  type ErrorCode,
  type TaxonomyErrorInit,
  type TruncationReason,
} from "../reference/src/envelope";
import { PORT_VERSIONS } from "../reference/src/ports/versions";
import { envelopeSchema } from "../reference/src/generated/schemas";
import { envelopeValidatorFor } from "../reference/test/support/ajv";
import fixtures from "./responses.json";

const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

for (const fixture of [...fixtures.negotiation, ...fixtures.negotiation.toReversed()]) {
  assert.deepEqual(negotiateEnvelopeVersion(fixture.meta), fixture.expected);
}
for (const fixture of fixtures.redaction) {
  assert.deepEqual(redactText(fixture.input), fixture.expected);
}
for (const fixture of fixtures.errors) {
  assert.ok(isErrorCode(fixture.input.code));
  const sealed = sealEnvelope(taxonomyError(fixture.input as TaxonomyErrorInit<ErrorCode>));
  assert.deepEqual(sealed.envelope, fixture.expected);
  assert.deepEqual(JSON.parse(sealed.json), sealed.envelope);
  assert.ok(validate(sealed.envelope), errorsText());
}
for (const fixture of fixtures.success) {
  const input = fixture.input;
  const cut = { ...input.cut, reason: input.cut.reason as TruncationReason };
  const sealed = sealEnvelope(okEnvelope({
    v: 1,
    freshness: "warm",
    budget: input.budget,
    results: truncateResults(input.rows, input.keep, cut),
    ...("notes" in input ? { notes: input.notes } : {}),
  }));
  assert.deepEqual(sealed.envelope, fixture.expected);
  assert.deepEqual(JSON.parse(sealed.json), sealed.envelope);
  assert.ok(validate(sealed.envelope), errorsText());
}
assert.throws(() => truncateResults([1], 0, { reason: "budget", narrow: [], cursor: "" }), RangeError);
assert.deepEqual(truncateResults([1], 8, { reason: "budget", narrow: [], cursor: "" }), { kept: [1] });
assert.deepEqual(PORT_VERSIONS, fixtures.ports);
process.stdout.write("Reference responses passed negotiation, 24-code taxonomy, truncation, redaction and port version fixtures.\n");
