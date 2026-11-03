import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { unzipSync, strFromU8 } from "fflate";
import { packageNativePlugin } from "../scripts/package-native-plugins.mjs";

const root = path.resolve(import.meta.dirname, "..");
const plugin = path.join(root, "plugins", "claude-code-accounts");
const binary = path.join(plugin, "bin", "echobird-account-engine.exe");

for (const platform of ["windows", "macos", "linux"]) {
  test(`native ${platform} package has one matching worker entry`, () => {
    const packaged = packageNativePlugin(plugin, platform, binary);
    const files = unzipSync(packaged.bytes);
    const manifest = JSON.parse(strFromU8(files["package.json"]));
    const entry =
      platform === "windows"
        ? "bin/echobird-account-engine.exe"
        : "bin/echobird-account-engine";
    assert.equal(manifest.version, `0.1.0-preview.4.${platform}.x64`);
    assert.deepEqual(manifest.echobird.platforms, [platform]);
    assert.deepEqual(manifest.architectures, ["x64"]);
    assert.equal(manifest.echobird.manifestVersion, 2);
    assert.equal(manifest.echobird.worker.entry, entry);
    assert.ok(Buffer.from(files[entry]).equals(readFileSync(binary)));
    assert.deepEqual(
      Object.keys(files).filter((name) => name.startsWith("bin/")),
      [entry],
    );
  });
}
