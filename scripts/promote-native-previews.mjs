import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { format } from "prettier";
import { validateIndex, validateEntry } from "./validate.mjs";

// Use only after the native client installation checks have passed.
for (const target of [
  "windows-x64",
  "macos-arm64",
  "macos-x64",
  "linux-arm64",
  "linux-x64",
]) {
  const index = validateIndex(
    JSON.parse(
      readFileSync(path.join("previews", target, "index.json"), "utf8"),
    ),
  );
  for (const plugin of index.plugins) {
    const filename = path.join("registry", `${plugin.id}.json`);
    const record = JSON.parse(readFileSync(filename, "utf8"));
    for (const release of plugin.releases) {
      const bytes = readFileSync(
        path.join("packages", `${plugin.id}-${release.manifest.version}.zip`),
      );
      if (createHash("sha256").update(bytes).digest("hex") !== release.sha256)
        throw new Error(`Package checksum mismatch: ${plugin.id}`);
      const existing = record.releases.find(
        (item) => item.manifest.version === release.manifest.version,
      );
      if (existing) {
        if (JSON.stringify(existing) !== JSON.stringify(release))
          throw new Error(
            `Release already exists with different contents: ${plugin.id}`,
          );
      } else record.releases.unshift(release);
    }
    validateEntry(record);
    writeFileSync(
      filename,
      await format(JSON.stringify(record), { parser: "json" }),
    );
  }
}
