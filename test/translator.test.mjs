import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { test } from "node:test";
import { packagePlugin } from "../scripts/package-plugin.mjs";
import { root } from "../scripts/validate.mjs";

test("the translator archive is reproducible and matches the reviewed release", () => {
  const first = packagePlugin(path.join(root, "plugins/translator"));
  assert.deepEqual(
    first.bytes,
    packagePlugin(path.join(root, "plugins/translator")).bytes,
  );
  assert.deepEqual(
    first.bytes,
    new Uint8Array(
      readFileSync(path.join(root, "packages/echobird.translator-1.0.0.zip")),
    ),
  );
  const release = JSON.parse(
    readFileSync(path.join(root, "registry/echobird.translator.json"), "utf8"),
  ).releases[0];
  assert.equal(release.sha256, first.sha256);
  assert.deepEqual(release.manifest, first.manifest);
});

test("translation and copying use declared host APIs only after explicit actions", async () => {
  const page = readFileSync(
    path.join(root, "plugins/translator/ui/index.html"),
    "utf8",
  );
  for (const forbidden of [
    "__MODEL_CONFIG__",
    "apiKey",
    "fetch(",
    "navigator.clipboard",
    "cursor: pointer",
  ])
    assert.ok(!page.includes(forbidden), forbidden);
  const nodes = new Map();
  const element = () => ({
    textContent: "",
    value: "",
    className: "",
    innerHTML: "",
    dataset: {},
    children: [],
    events: {},
    classList: { contains: () => false, add() {}, remove() {}, toggle() {} },
    appendChild(child) {
      this.children.push(child);
    },
    addEventListener(type, callback) {
      this.events[type] = callback;
    },
    querySelectorAll() {
      return this.children;
    },
    querySelector() {
      return null;
    },
  });
  const node = (id) => {
    if (!nodes.has(id)) nodes.set(id, element());
    return nodes.get(id);
  };
  const calls = [],
    copies = [];
  let fail = false;
  const context = vm.createContext({
    window: {
      echobird: {
        context: { locale: "en", modelName: "Fixture model" },
        generateText: async (prompt) => {
          calls.push(prompt);
          if (fail) throw new Error("fixture failure");
          return "你好";
        },
        writeClipboardText: async (text) => {
          copies.push(text);
        },
      },
    },
    document: {
      getElementById: node,
      createElement: element,
      addEventListener() {},
      querySelectorAll: () => [],
    },
    navigator: { language: "en" },
    setTimeout() {},
    console,
  });
  vm.runInContext(page.match(/<script>([\s\S]*?)<\/script>/u)[1], context);
  assert.deepEqual(calls, []);
  assert.deepEqual(copies, []);
  assert.equal(node("modelBadge").textContent, "Fixture model");
  node("srcText").value = "Hello";
  await node("translateBtn").events.click();
  assert.match(calls[0], /to Chinese \(Simplified\)/u);
  assert.ok(calls[0].endsWith("Hello"));
  assert.equal(node("outputText").textContent, "你好");
  node("copyBtn").events.click();
  await Promise.resolve();
  assert.deepEqual(copies, ["你好"]);
  node("clearBtn").events.click();
  assert.equal(node("srcText").value, "");
  assert.equal(node("outputText").textContent, "");
  node("srcText").value = "Second";
  fail = true;
  await node("translateBtn").events.click();
  assert.match(node("statusMsg").textContent, /fixture failure/u);
  assert.equal(node("translateBtn").disabled, false);
});
