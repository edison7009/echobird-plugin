import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { test } from "node:test";
import { root } from "../scripts/validate.mjs";

test("account module dispatch returns structured data and keeps logs off its response channel", () => {
  const sdk = pathToFileURL(path.join(root, "sdk/account-worker.mjs")).href;
  const code = `import {serveAccountOperations} from ${JSON.stringify(sdk)};
await serveAccountOperations({
  accounts: async (_, context) => { console.log("diagnostic"); return [{id:context.providerId}]; },
  "account.refresh": async () => { throw new Error("private-session-secret"); }
});`;
  const request = (id, method) =>
    JSON.stringify({
      jsonrpc: "2.0",
      id,
      method,
      params: { providerId: "fixture", input: {} },
    });
  const child = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", code],
    {
      input:
        [
          request("one", "accounts"),
          request("two", "account.refresh"),
          request("three", "toString"),
        ].join("\n") + "\n",
      encoding: "utf8",
      timeout: 5000,
    },
  );
  assert.equal(child.status, 0, child.stderr);
  const responses = child.stdout
    .trim()
    .split("\n")
    .map((line) => JSON.parse(line));
  assert.deepEqual(responses[0].result, [{ id: "fixture" }]);
  assert.equal(responses[1].error.code, -32000);
  assert.equal(responses[2].error.code, -32000);
  assert.ok(!child.stdout.includes("private-session-secret"));
  assert.equal(child.stderr.trim(), "diagnostic");
});
