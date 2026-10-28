import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { unzipSync } from "fflate";
import { packagePlugin } from "../scripts/package-plugin.mjs";
import {
  root,
  validateLocalManifest,
  validateManifest,
} from "../scripts/validate.mjs";

const applications = {
  "claude-desktop-accounts": "claudedesktop",
  "claude-code-accounts": "claudecode",
  "doubao-work-accounts": "doubaowork",
  "dola-accounts": "dola",
  "workbuddy-accounts": "workbuddy",
  "workbuddy-ai-accounts": "workbuddyai",
};

test("native modules require one application and explicit operations", () => {
  const manifest = validateLocalManifest(
    path.join(root, "plugins/claude-code-accounts/package.json"),
  );
  manifest.echobird.contributes.providers[0].applications.push("other");
  assert.throws(() => validateManifest(manifest));
  manifest.echobird.contributes.providers[0].applications.pop();
  manifest.echobird.worker.runtime = "node";
  assert.throws(() => validateManifest(manifest));
});

for (const [folder, application] of Object.entries(applications)) {
  test(`${application} ships its original controls as an immutable native package`, () => {
    const directory = path.join(root, "plugins", folder);
    const { manifest, bytes } = packagePlugin(directory);
    assert.equal(manifest.category, "account");
    assert.deepEqual(manifest.echobird.platforms, ["windows"]);
    assert.deepEqual(manifest.echobird.contributes.pages, []);
    assert.deepEqual(manifest.echobird.contributes.providers[0].applications, [
      application,
    ]);
    assert.equal(manifest.echobird.worker.runtime, "native");
    const archive = readFileSync(
      path.join(
        root,
        "packages",
        `${manifest.publisher}.${manifest.name}-${manifest.version}.zip`,
      ),
    );
    assert.deepEqual(Buffer.from(bytes), archive);
    const files = unzipSync(archive);
    assert.deepEqual(
      JSON.parse(Buffer.from(files["package.json"]).toString()),
      manifest,
    );
    assert.equal(
      Buffer.from(files[manifest.echobird.worker.entry])
        .subarray(0, 2)
        .toString(),
      "MZ",
    );
    assert.ok(files[manifest.icon].length > 100);
    assert.ok(files["NOTICE.txt"].length > 100);
    const operations = manifest.echobird.contributes.providers[0].operations;
    for (const operation of [
      "accounts",
      "login.start",
      "login.cancel",
      "account.apply",
      "account.remove",
    ])
      assert.ok(operations.includes(operation));
    if (application === "dola") assert.ok(!operations.includes("voice"));
  });
}
