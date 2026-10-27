import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import semver from "semver";
import {
  loadRegistry,
  root,
  validateEntry,
  validateIndex,
} from "./validate.mjs";

export function buildIndex(entries) {
  const plugins = entries
    .map((entry) => ({
      ...validateEntry(entry),
      releases: [...entry.releases].sort((a, b) =>
        semver.rcompare(a.manifest.version, b.manifest.version),
      ),
    }))
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  return validateIndex({
    $schema:
      "https://raw.githubusercontent.com/edison7009/echobird-plugin/main/schemas/index.schema.json",
    schemaVersion: 1,
    name: "EchoBird Plugin Hub",
    plugins,
  });
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  try {
    const args = process.argv.slice(2);
    if (args.length > 1 || (args.length === 1 && args[0] !== "--check"))
      throw new Error("Usage: node scripts/build-index.mjs [--check]");
    const output = JSON.stringify(buildIndex(loadRegistry()), null, 2) + "\n";
    const filename = path.join(root, "index.json");
    if (args[0] === "--check") {
      if (readFileSync(filename, "utf8") !== output)
        throw new Error("index.json is outdated; run npm run build:index");
      console.log("The committed index is up to date.");
    } else {
      writeFileSync(filename, output);
      console.log("Built index.json from reviewed registry records.");
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
