// Property: for ANY list of JSON values, every value validates against the
// Zod schema and JSON Schema inferred from that list.
import { test } from "node:test";
import assert from "node:assert/strict";
import fc from "fast-check";
import { checkSamples, generate } from "./helpers.js";

// Strings that trip the format and enum detectors, mixed with arbitrary text.
const trickyString = fc.oneof(
  fc.string(),
  fc.constantFrom("paid", "pending", "failed"),
  fc.emailAddress(),
  fc.uuid(),
  fc.webUrl({ withQueryParameters: true, withFragments: true }),
  fc.date({ min: new Date("0001-01-01"), max: new Date("9999-12-31"), noInvalidDate: true }).map((d) => d.toISOString()),
  fc.date({ noInvalidDate: true, min: new Date("1000-01-01"), max: new Date("9999-12-31") }).map((d) => d.toISOString().slice(0, 10)),
);

const value = fc.letrec((tie) => ({
  json: fc.oneof(
    { depthSize: "small", withCrossShrink: true },
    fc.constant(null),
    fc.boolean(),
    fc.integer(),
    fc.double({ noNaN: true, noDefaultInfinity: true }),
    fc.maxSafeNat().map((n) => n * 4), // integers past 2^53
    trickyString,
    fc.array(tie("json"), { maxLength: 4 }),
    fc.dictionary(fc.oneof(fc.string(), fc.constantFrom("id", "__proto__", "constructor", "a-b")), tie("json"), {
      maxKeys: 5,
    }),
  ),
})).json;

test("every sample validates against its inferred schemas", () => {
  fc.assert(
    fc.property(fc.array(value, { minLength: 1, maxLength: 6 }), fc.boolean(), (samples, detectEnums) => {
      // Round-trip through JSON text exactly as a user would paste it.
      const text = JSON.stringify(samples);
      const parsed = JSON.parse(text);
      const failure = checkSamples(generate(text, { detectEnums }), parsed);
      assert.equal(failure, null);
    }),
    { numRuns: Number(process.env.NUM_RUNS ?? 400) },
  );
});
