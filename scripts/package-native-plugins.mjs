import { readFileSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { packagePlugin } from "./package-plugin.mjs";
import { validateManifest } from "./validate.mjs";
import { buildIndex } from "./build-index.mjs";

const platforms = new Set(["windows", "macos", "linux"]);

export function packageNativePlugin(
  directory,
  platform,
  binary,
  architecture = "x64",
) {
  if (!platforms.has(platform))
    throw new Error(`Unsupported platform: ${platform}`);
  const source = JSON.parse(
    readFileSync(path.join(directory, "package.json"), "utf8"),
  );
  if (source.echobird.worker?.runtime !== "native")
    throw new Error("Plugin does not contain a native account worker");
  const entry = `bin/echobird-account-engine${platform === "windows" ? ".exe" : ""}`;
  const manifest = structuredClone(source);
  if (!["x64", "arm64"].includes(architecture))
    throw new Error("Unsupported architecture");
  manifest.version = `${source.version}.${platform}.${architecture}`;
  manifest.echobird.platforms = [platform];
  manifest.architectures = [architecture];
  manifest.echobird.manifestVersion = 2;
  manifest.echobird.worker.entry = entry;
  validateManifest(manifest);
  return packagePlugin(directory, {
    manifest,
    omitted: new Set(["bin/echobird-account-engine.exe"]),
    replacements: new Map([
      [
        "package.json",
        new TextEncoder().encode(`${JSON.stringify(manifest, null, 2)}\n`),
      ],
      [entry, new Uint8Array(readFileSync(binary))],
    ]),
  });
}

export function packageNativePlugins({
  platform,
  binary,
  architecture,
  output = "out/native-previews",
}) {
  const plugins = path.resolve("plugins");
  const results = [];
  const records = [];
  const commit = process.env.GITHUB_SHA;
  if (!/^[a-f0-9]{40}$/.test(commit ?? ""))
    throw new Error("GITHUB_SHA is required for the candidate index");
  for (const directory of [
    "claude-code-accounts",
    "claude-desktop-accounts",
    "dola-accounts",
    "doubao-work-accounts",
    "trae-accounts",
    "trae-cn-accounts",
    "workbuddy-accounts",
    "workbuddy-ai-accounts",
  ]) {
    const packaged = packageNativePlugin(
      path.join(plugins, directory),
      platform,
      binary,
      architecture,
    );
    const filename = `${packaged.manifest.publisher}.${packaged.manifest.name}-${packaged.manifest.version}.zip`;
    const target = path.resolve(output, filename);
    mkdirSync(path.dirname(target), { recursive: true });
    if (
      existsSync(target) &&
      !readFileSync(target).equals(Buffer.from(packaged.bytes))
    )
      throw new Error(
        "This package version already exists with different contents; increment the plugin version.",
      );
    writeFileSync(target, packaged.bytes);
    results.push({ file: target, sha256: packaged.sha256 });
    const id = `${packaged.manifest.publisher}.${packaged.manifest.name}`;
    records.push({
      id,
      repository: packaged.manifest.repository,
      releases: [
        {
          manifest: packaged.manifest,
          commit,
          sha256: packaged.sha256,
          downloads: {
            international: `https://raw.githubusercontent.com/edison7009/echobird-plugin/${commit}/packages/${filename}`,
            china: `https://api.gitcode.com/api/v5/repos/edison7009/echobird-plugin/raw/packages/${filename}`,
          },
        },
      ],
    });
  }
  writeFileSync(
    path.resolve(output, "candidate-index.json"),
    `${JSON.stringify(buildIndex(records), null, 2)}\n`,
  );
  return results;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const args = process.argv.slice(2);
  if (
    args.length !== 6 ||
    args[0] !== "--platform" ||
    args[2] !== "--binary" ||
    args[4] !== "--architecture"
  )
    throw new Error(
      "Usage: node scripts/package-native-plugins.mjs --platform <windows|macos|linux> --binary <file> --architecture <x64|arm64>",
    );
  console.log(
    JSON.stringify(
      packageNativePlugins({
        platform: args[1],
        binary: path.resolve(args[3]),
        architecture: args[5],
        output: `out/native-previews/${args[1]}-${args[5]}`,
      }),
      null,
      2,
    ),
  );
}
