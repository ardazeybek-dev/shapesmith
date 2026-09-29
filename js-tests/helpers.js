import { z } from "zod";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";
import init, { generate } from "shapesmith";

await init();

export { generate };

/** Turns emitted Zod source into a live schema object. */
export function loadZod(source, rootName = "Root") {
  const body = source
    .replace(/^import .*$/m, "")
    .replace(/^export type .*$/gm, "")
    .replace(/^export const /gm, "const ");
  return new Function("z", `${body}\nreturn ${rootName}Schema;`)(z);
}

/** Compiles emitted JSON Schema with ajv in strict mode, formats enabled. */
export function loadJsonSchema(source) {
  const ajv = new Ajv2020({ strict: true, allowUnionTypes: true, ownProperties: true, allErrors: true });
  addFormats(ajv);
  return ajv.compile(JSON.parse(source));
}

/**
 * Asserts every sample passes both generated validators; returns the first
 * failure as a readable message instead of throwing so property tests can
 * report the counterexample.
 */
export function checkSamples(out, samples, rootName = "Root") {
  const zod = loadZod(out.zod, rootName);
  const validate = loadJsonSchema(out.jsonSchema);
  for (const sample of samples) {
    const parsed = zod.safeParse(sample);
    if (!parsed.success) {
      return `zod rejected ${JSON.stringify(sample)}: ${JSON.stringify(parsed.error.issues)}\n${out.zod}`;
    }
    if (!validate(sample)) {
      return `ajv rejected ${JSON.stringify(sample)}: ${JSON.stringify(validate.errors)}\n${out.jsonSchema}`;
    }
  }
  return null;
}
