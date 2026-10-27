import {
  readdirSync,
  readFileSync,
  mkdirSync,
  writeFileSync,
  lstatSync,
  existsSync,
} from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { zipSync } from "fflate";
import { validateLocalManifest, portablePath } from "./validate.mjs";

export function packagePlugin(directory) {
  const manifest = validateLocalManifest(path.join(directory, "package.json"));
  const files = {};
  let total = 0;
  const walk = (folder, prefix = "") => {
    for (const name of readdirSync(folder).sort()) {
      if (name === ".git" || name === "node_modules") continue;
      const filename = path.join(folder, name);
      const relative = prefix + name;
      if (!portablePath(relative)) throw new Error("Non-portable package path");
      const stat = lstatSync(filename);
      if (stat.isSymbolicLink())
        throw new Error("Plugin packages cannot contain links");
      if (stat.isDirectory()) walk(filename, relative + "/");
      else if (stat.isFile()) {
        let bytes = new Uint8Array(readFileSync(filename));
        if (
          /\.(?:html|css|js|mjs|json|svg|md|txt|ts)$/iu.test(name) ||
          name === "LICENSE"
        ) {
          const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
          bytes = new TextEncoder().encode(text.replace(/\r\n/gu, "\n"));
        }
        total += bytes.length;
        if (total > 20 * 1024 * 1024 || Object.keys(files).length >= 2000)
          throw new Error("Plugin package exceeds runtime limits");
        files[relative] = [bytes, { mtime: new Date(2020, 0, 1) }];
      } else throw new Error("Unsupported package file");
    }
  };
  walk(directory);
  const bytes = zipSync(files, { level: 9 });
  return {
    manifest,
    bytes,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const directory = path.resolve(process.argv[2] ?? "plugins/reversi");
  const { manifest, bytes, sha256 } = packagePlugin(directory);
  const output = path.resolve(
    "packages",
    `${manifest.publisher}.${manifest.name}-${manifest.version}.zip`,
  );
  mkdirSync(path.dirname(output), { recursive: true });
  if (existsSync(output) && !readFileSync(output).equals(Buffer.from(bytes))) {
    throw new Error(
      "This package version already exists with different contents; increment the plugin version.",
    );
  }
  writeFileSync(output, bytes);
  console.log(
    JSON.stringify({
      file: path.relative(process.cwd(), output),
      bytes: bytes.length,
      sha256,
    }),
  );
}
