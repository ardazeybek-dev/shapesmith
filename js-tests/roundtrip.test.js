// Every fixture must validate against the schemas inferred from it, with the
// real Zod, ajv and TypeScript compiler.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { checkSamples, generate } from "./helpers.js";

const here = fileURLToPath(new URL(".", import.meta.url));
const fixtures = join(here, "fixtures");
const tmp = join(here, ".tmp");
mkdirSync(tmp, { recursive: true });

function samplesOf(text) {
  try {
    const doc = JSON.parse(text);
    return Array.isArray(doc) ? doc : [doc];
  } catch {
    return text.split("\n").filter((l) => l.trim()).map((l) => JSON.parse(l));
  }
}

const files = readdirSync(fixtures);
const tsFiles = [];

for (const file of files) {
  test(`zod + ajv accept every sample: ${file}`, () => {
    const text = readFileSync(join(fixtures, file), "utf8");
    const out = generate(text);
    assert.equal(out.samples, samplesOf(text).length);
    assert.equal(checkSamples(out, samplesOf(text)), null);
  });

  // Type-check the generated TypeScript and Zod-inferred types against the samples.
  // Skipped for inherited key names: TypeScript checks a missing optional
  // `constructor` against the inherited `Function`, and a `"__proto__"` key in
  // an object literal sets the prototype, so those samples cannot be written
  // as literals at all. Their runtime behaviour is covered above.
  if (file === "inherited-keys.json") continue;
  const text = readFileSync(join(fixtures, file), "utf8");
  const out = generate(text);
  const samples = JSON.stringify(samplesOf(text), null, 2);
  const base = file.replace(/\W/g, "_");
  writeFileSync(join(tmp, `${base}.types.ts`), `${out.typescript}\nexport const samples: Root[] = ${samples};\n`);
  writeFileSync(join(tmp, `${base}.zod.ts`), `${out.zod}\nexport const samples: Root[] = ${samples};\n`);
  tsFiles.push(`${base}.types.ts`, `${base}.zod.ts`);
}

test("tsc --strict accepts the samples as the generated types", () => {
  const tsc = fileURLToPath(import.meta.resolve("typescript/package.json").replace(/package\.json$/, "bin/tsc"));
  const args = ["--noEmit", "--strict", "--skipLibCheck", "--target", "es2022", "--module", "nodenext", ...tsFiles];
  try {
    execFileSync(process.execPath, [tsc, ...args], { cwd: tmp, encoding: "utf8" });
  } catch (e) {
    assert.fail(`tsc failed:\n${e.stdout}${e.stderr}`);
  }
});

test("options reach the WASM core", () => {
  const out = generate('{"a": 1}', { rootName: "my thing", detectFormats: false });
  assert.match(out.typescript, /^export interface MyThing \{/);
  assert.throws(() => generate("{nope"), /key must be a string/);
});
