import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { test } from "node:test";
import { unzipSync } from "fflate";
import { packagePlugin } from "../scripts/package-plugin.mjs";
import { root, validateManifest } from "../scripts/validate.mjs";

test("the Reversi package is reproducible and contains the manifest and local runtime files", () => {
  const first = packagePlugin(path.join(root, "plugins/reversi"));
  const second = packagePlugin(path.join(root, "plugins/reversi"));
  assert.deepEqual(first.bytes, second.bytes);
  const entries = unzipSync(first.bytes);
  assert.equal(
    validateManifest(
      JSON.parse(new TextDecoder().decode(entries["package.json"])),
    ).publisher,
    "echobird",
  );
  assert.ok(entries["ui/index.html"]);
  assert.ok(entries["ui/icon.svg"]);
  assert.ok(entries.LICENSE);
  const committed = readFileSync(
    path.join(root, "packages/echobird.reversi-1.0.0.zip"),
  );
  assert.deepEqual(new Uint8Array(committed), first.bytes);
});

test("the sample starts without inference and uses only the declared host API for a legal AI move", async () => {
  const page = readFileSync(
    path.join(root, "plugins/reversi/ui/index.html"),
    "utf8",
  );
  assert.ok(!page.includes("__MODEL_CONFIG__"));
  assert.ok(!page.includes("apiKey"));
  assert.ok(!page.includes("fetch("));
  assert.ok(!page.includes("cursor: pointer"));
  const nodes = new Map();
  const element = () => ({
    textContent: "",
    innerHTML: "",
    className: "",
    style: {},
    classList: { add() {} },
    appendChild() {},
    remove() {},
    scrollTop: 0,
    scrollHeight: 0,
  });
  const node = (id) => {
    if (!nodes.has(id)) nodes.set(id, element());
    return nodes.get(id);
  };
  const calls = [];
  const timers = [];
  const context = vm.createContext({
    window: {
      echobird: {
        context: { locale: "en", modelName: "Fixture model" },
        generateText: async (prompt) => {
          calls.push(prompt);
          const move = prompt.match(/valid moves: (\d,\d)/u)?.[1];
          assert.ok(move);
          return `${move}\nA move`;
        },
      },
    },
    document: {
      getElementById: node,
      querySelector: node,
      createElement: element,
    },
    navigator: { language: "en" },
    setTimeout: (callback) => {
      timers.push(callback);
    },
    console,
  });
  vm.runInContext(page.match(/<script>([\s\S]*?)<\/script>/u)[1], context);
  assert.equal(calls.length, 0);
  assert.equal(node("aiName").textContent, "Fixture model");
  vm.runInContext("playerMove(2, 3)", context);
  assert.equal(calls.length, 0);
  for (const callback of timers) await callback();
  assert.equal(calls.length, 1);
  assert.equal(vm.runInContext("currentPlayer", context), 1);
  assert.equal(vm.runInContext("aiThinking", context), false);
});
