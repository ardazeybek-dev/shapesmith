import init, { generate } from "./pkg/shapesmith.js";
import { examples } from "./examples.js";

const $ = (id) => document.getElementById(id);
const input = $("input");
const output = $("output").querySelector("code");
const status = $("status");
const optionIds = ["detectFormats", "detectEnums", "splitTopLevelArrays"];
const suggestedNames = { Orders: "Order", "GitHub events (NDJSON)": "Event", Weather: "Forecast" };
let active = "typescript";
let result = null;

const KEYWORDS = /^(?:export|interface|type|const|import|from|typeof|null|true|false|string|number|boolean|unknown)$/;

// Generated code is regular enough for a small tokenizer: comments, strings,
// numbers, keywords, type names and property keys.
function highlight(code) {
  const escape = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const token =
    /(\/\*\*[\s\S]*?\*\/|\/\/.*)|("(?:[^"\\\n]|\\.)*")(\s*:)?|(-?\b\d+(?:\.\d+)?\b)|([A-Za-z_$][\w$]*)(\??:)?/g;
  let html = "";
  let last = 0;
  for (const m of code.matchAll(token)) {
    html += escape(code.slice(last, m.index));
    last = m.index + m[0].length;
    const [, comment, str, strColon, num, word, wordColon] = m;
    if (comment) html += `<span class="t-comment">${escape(comment)}</span>`;
    else if (str && strColon) html += `<span class="t-key">${escape(str)}</span>${strColon}`;
    else if (str) html += `<span class="t-string">${escape(str)}</span>`;
    else if (num) html += `<span class="t-number">${num}</span>`;
    else if (wordColon) {
      const mark = wordColon.startsWith("?") ? '<span class="t-optional">?</span>:' : ":";
      html += `<span class="t-key">${word}</span>${mark}`;
    } else if (KEYWORDS.test(word)) html += `<span class="t-keyword">${word}</span>`;
    else if (/^[A-Z]/.test(word)) html += `<span class="t-type">${word}</span>`;
    else html += word;
  }
  return html + escape(code.slice(last));
}

function render() {
  if (result) output.innerHTML = highlight(result[active]);
}

const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

function run() {
  const options = { rootName: $("rootName").value };
  for (const id of optionIds) options[id] = $(id).checked;
  const started = performance.now();
  try {
    const out = generate(input.value, options);
    const ms = performance.now() - started;
    result = { typescript: out.typescript, zod: out.zod, jsonSchema: out.jsonSchema };
    status.textContent = `${plural(out.samples, "sample")} → ${plural(out.types, "type")} in ${ms.toFixed(2)} ms`;
    out.free();
    status.classList.remove("error");
    render();
  } catch (e) {
    status.textContent = `Can't read the samples: ${e.message}`;
    status.classList.add("error");
  }
}

function debounce(fn, ms) {
  let timer;
  return () => {
    clearTimeout(timer);
    timer = setTimeout(fn, ms);
  };
}

function loadExample(name) {
  input.value = examples[name];
  $("rootName").value = suggestedNames[name] ?? "Root";
  $("example").value = name;
  run();
}

for (const name of Object.keys(examples)) $("example").append(new Option(name, name));
$("example").addEventListener("change", (e) => e.target.value && loadExample(e.target.value));

const tabs = [...document.querySelectorAll('[role="tab"]')];
function selectTab(tab) {
  for (const t of tabs) {
    t.setAttribute("aria-selected", String(t === tab));
    t.tabIndex = t === tab ? 0 : -1;
  }
  $("output").setAttribute("aria-labelledby", tab.id);
  active = tab.dataset.target;
  render();
}
tabs.forEach((tab, i) => {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (e) => {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (!step) return;
    const next = tabs[(i + step + tabs.length) % tabs.length];
    next.focus();
    selectTab(next);
  });
});

$("copy").addEventListener("click", async () => {
  if (!result) return;
  await navigator.clipboard.writeText(result[active]);
  $("copy").textContent = "Copied";
  setTimeout(() => ($("copy").textContent = "Copy"), 1200);
});

input.addEventListener("input", debounce(run, 120));
$("rootName").addEventListener("input", debounce(run, 120));
for (const id of optionIds) $(id).addEventListener("change", run);

await init();
loadExample("Orders");
