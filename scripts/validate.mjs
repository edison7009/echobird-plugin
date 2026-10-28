import { lstatSync, readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import semver from "semver";

export const root = fileURLToPath(new URL("../", import.meta.url));
const schemaBase =
  "https://raw.githubusercontent.com/edison7009/echobird-plugin/main/schemas/";
const ajv = new Ajv2020({ allErrors: true });
ajv.addFormat("semver", (value) => semver.valid(value) === value);
ajv.addFormat("public-https", (value) => {
  try {
    const url = new URL(value);
    return (
      url.protocol === "https:" &&
      !url.username &&
      !url.password &&
      !url.search &&
      !url.hash
    );
  } catch {
    return false;
  }
});
export function portablePath(value) {
  return (
    value.length > 0 &&
    value.length <= 240 &&
    value.split("/").every((part) => {
      const stem = part.split(".")[0];
      return (
        part &&
        part !== "." &&
        part !== ".." &&
        !/[\\\s\x00-\x1f:<>"|?*%#]/u.test(part) &&
        !/[. ]$/u.test(part) &&
        !/^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])$/iu.test(stem)
      );
    })
  );
}
ajv.addFormat("relative-asset", portablePath);
ajv.addFormat(
  "relative-html",
  (value) => portablePath(value) && value.endsWith(".html"),
);

export function readJson(filename) {
  if (statSync(filename).size > 1024 * 1024)
    throw new Error(`${path.basename(filename)} exceeds 1 MiB`);
  return JSON.parse(readFileSync(filename, "utf8").replace(/^\uFEFF/, ""));
}

for (const name of ["plugin-manifest", "registry-entry", "index"]) {
  ajv.addSchema(readJson(path.join(root, "schemas", `${name}.schema.json`)));
}

function checkSchema(name, value) {
  const validate = ajv.getSchema(`${schemaBase}${name}.schema.json`);
  if (!validate(value)) {
    throw new Error(ajv.errorsText(validate.errors, { separator: "; " }));
  }
}

export function validateManifest(manifest) {
  checkSchema("plugin-manifest", manifest);
  const worker = manifest.echobird.worker;
  if (worker && worker.runtime !== "native" && !worker.entry.endsWith(".mjs"))
    throw new Error("Node account code entry must end in .mjs");
  const pages = manifest.echobird.contributes.pages.map((page) => page.id);
  if (!pages.length && !manifest.echobird.worker)
    throw new Error("A plugin must declare a page or an account code module");
  if (new Set(pages).size !== pages.length)
    throw new Error("Duplicate page id");
  const providers = manifest.echobird.contributes.providers ?? [];
  if (
    worker?.runtime === "native" &&
    (providers.length !== 1 || providers[0].applications.length !== 1)
  )
    throw new Error("Native account modules must declare one application");
  const providerIds = new Set();
  for (const provider of providers) {
    if (providerIds.has(provider.id)) throw new Error("Duplicate provider id");
    providerIds.add(provider.id);
    if (provider.pageId !== undefined && !pages.includes(provider.pageId))
      throw new Error("Provider page id does not exist");
    if (provider.pageId === undefined && !provider.operations?.length)
      throw new Error("Provider needs a page or declared operations");
    if (provider.operations?.length && !manifest.echobird.worker)
      throw new Error("Account operations require a code module");
  }
  if (
    manifest.echobird.worker &&
    (!manifest.echobird.permissions.includes("account.code") ||
      !providers.some((provider) => provider.operations?.length))
  )
    throw new Error("Account code requires permission and declared operations");
  return manifest;
}

export function validateEntry(entry) {
  checkSchema("registry-entry", entry);
  const versions = [];
  for (const release of entry.releases) {
    const manifest = validateManifest(release.manifest);
    if (`${manifest.publisher}.${manifest.name}` !== entry.id)
      throw new Error("Plugin id does not match manifest");
    if (manifest.repository !== entry.repository)
      throw new Error("Repository does not match manifest");
    if (
      versions.some(
        (version) => semver.compare(version, manifest.version) === 0,
      )
    )
      throw new Error("Duplicate release version");
    versions.push(manifest.version);
  }
  return entry;
}

export function validateIndex(index) {
  checkSchema("index", index);
  const ids = new Set();
  for (const entry of index.plugins) {
    validateEntry(entry);
    if (ids.has(entry.id)) throw new Error("Duplicate plugin id");
    ids.add(entry.id);
  }
  return index;
}

export function loadRegistry(directory = path.join(root, "registry")) {
  const entries = [];
  for (const filename of readdirSync(directory).sort()) {
    if (filename === "README.md") continue;
    const fullPath = path.join(directory, filename);
    if (!filename.endsWith(".json") || !lstatSync(fullPath).isFile())
      throw new Error(`Unexpected registry file: ${filename}`);
    const entry = validateEntry(readJson(fullPath));
    if (filename !== `${entry.id}.json`)
      throw new Error("Registry filename must match plugin id");
    if (
      entry.id.startsWith("example.") ||
      entry.releases.some(
        (release) => /^0+$/.test(release.commit) || /^0+$/.test(release.sha256),
      )
    ) {
      throw new Error("Example records cannot enter the public registry");
    }
    entries.push(entry);
  }
  return entries;
}

export function validateLocalManifest(filename) {
  const manifest = validateManifest(readJson(filename));
  for (const page of manifest.echobird.contributes.pages) {
    const file = path.join(path.dirname(filename), page.entry);
    if (!lstatSync(file).isFile())
      throw new Error(`Missing regular page file: ${page.entry}`);
  }
  if (
    manifest.echobird.worker &&
    !lstatSync(
      path.join(path.dirname(filename), manifest.echobird.worker.entry),
    ).isFile()
  )
    throw new Error("Missing regular account code module");
  return manifest;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  try {
    const args = process.argv.slice(2);
    if (args.length === 2 && args[0] === "--manifest") {
      validateLocalManifest(path.resolve(args[1]));
      console.log("Plugin manifest and local page files are valid.");
    } else if (args.length === 0) {
      const entries = loadRegistry();
      validateLocalManifest(
        path.join(root, "templates/hello-plugin/package.json"),
      );
      for (const directory of readdirSync(path.join(root, "plugins"))) {
        validateLocalManifest(
          path.join(root, "plugins", directory, "package.json"),
        );
      }
      validateEntry(readJson(path.join(root, "examples/registry-entry.json")));
      validateIndex(readJson(path.join(root, "index.json")));
      console.log(
        `Validated ${entries.length} published plugins, the draft template and the record example.`,
      );
    } else {
      throw new Error(
        "Usage: node scripts/validate.mjs [--manifest /path/to/package.json]",
      );
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
