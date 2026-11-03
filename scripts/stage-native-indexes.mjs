import { readFileSync, readdirSync, mkdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { format } from "prettier";
import { buildIndex } from "./build-index.mjs";

// Run after the reviewed CI ZIPs have been committed under packages/.
const [directory, commit] = process.argv.slice(2);
if (!directory || !/^[a-f0-9]{40}$/.test(commit ?? ""))
  throw new Error(
    "Usage: node scripts/stage-native-indexes.mjs <artifact-directory> <package-commit>",
  );
for (const target of readdirSync(directory).sort()) {
  if (!/^(?:windows-x64|(?:macos|linux)-(?:arm64|x64))$/.test(target))
    throw new Error(`Unexpected preview target: ${target}`);
  const index = JSON.parse(
    readFileSync(path.join(directory, target, "candidate-index.json"), "utf8"),
  );
  for (const plugin of index.plugins) {
    for (const release of plugin.releases) {
      const filename = `${plugin.id}-${release.manifest.version}.zip`;
      const bytes = readFileSync(path.join("packages", filename));
      if (createHash("sha256").update(bytes).digest("hex") !== release.sha256)
        throw new Error(`Package checksum mismatch: ${filename}`);
      release.commit = commit;
      release.downloads.international = `https://raw.githubusercontent.com/edison7009/echobird-plugin/${commit}/packages/${filename}`;
    }
  }
  const output = path.join("previews", target, "index.json");
  mkdirSync(path.dirname(output), { recursive: true });
  writeFileSync(
    output,
    await format(JSON.stringify(buildIndex(index.plugins)), { parser: "json" }),
  );
  console.log(output);
}
