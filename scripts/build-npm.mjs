// Builds the npm package into ./npm: a browser/bundler build (ES module that
// fetches the .wasm on `init()`) and a Node build (loads the .wasm
// synchronously). wasm-pack drops a `*` .gitignore into each output folder,
// which would make npm leave the builds out, so those are removed. Both expose the same API: `await init(); generate(...)`.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, readdirSync, readFileSync, rmdirSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Plain paths, and unlink/rmdir instead of fs.cpSync/rmSync: on Windows those
// silently do nothing for paths with non-ASCII characters (Node 24).
const root = fileURLToPath(new URL("..", import.meta.url));
const out = join(root, "npm");
const cargo = readFileSync(join(root, "Cargo.toml"), "utf8");
const version = cargo.match(/^version = "(.+)"$/m)[1];

function remove(path) {
  if (!existsSync(path)) return;
  if (statSync(path).isDirectory()) {
    for (const entry of readdirSync(path)) remove(join(path, entry));
    rmdirSync(path);
  } else {
    unlinkSync(path);
  }
}

const wasmPack = (target, dir) =>
  execFileSync(`wasm-pack build --release --no-pack --target ${target} --out-dir ${dir}`, {
    cwd: root,
    stdio: "inherit",
    shell: true,
  });

remove(out);
wasmPack("web", "npm/web");
wasmPack("nodejs", "npm/node");
for (const dir of ["web", "node"]) remove(join(out, dir, ".gitignore"));

// The Node build is CommonJS inside an ESM package.
writeFileSync(join(out, "node", "package.json"), JSON.stringify({ type: "commonjs" }, null, 2) + "\n");
writeFileSync(
  join(out, "node", "index.mjs"),
  `import wasm from "./shapesmith.js";

export const { generate, GenerateResult } = wasm;

/** The Node build is ready on import; kept so browser and Node code match. */
export default async function init() {}
export function initSync() {}
`,
);

const pkg = {
  name: "shapesmith",
  version,
  description: "Infer TypeScript types, Zod schemas and JSON Schema from JSON samples. Rust compiled to WebAssembly.",
  license: "MIT",
  author: "Arda Zeybek",
  repository: { type: "git", url: "git+https://github.com/ardazeybek-dev/shapesmith.git" },
  homepage: "https://ardazeybek-dev.github.io/shapesmith/",
  bugs: "https://github.com/ardazeybek-dev/shapesmith/issues",
  keywords: ["json", "typescript", "zod", "json-schema", "type-inference", "wasm", "webassembly", "rust"],
  type: "module",
  types: "./web/shapesmith.d.ts",
  exports: {
    ".": {
      types: "./web/shapesmith.d.ts",
      node: { import: "./node/index.mjs", require: "./node/shapesmith.js" },
      default: "./web/shapesmith.js",
    },
  },
  files: ["web/", "node/", "README.md", "LICENSE"],
  sideEffects: false,
  engines: { node: ">=18" },
};
writeFileSync(join(out, "package.json"), JSON.stringify(pkg, null, 2) + "\n");
for (const file of ["LICENSE", "README.md"]) copyFileSync(join(root, file), join(out, file));
console.log(`npm package shapesmith@${version} assembled in ./npm`);
