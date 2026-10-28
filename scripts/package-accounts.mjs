import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { packagePlugin } from "./package-plugin.mjs";
import { root } from "./validate.mjs";

const binary = process.argv[2];
if (!binary || !path.isAbsolute(binary) || !existsSync(binary))
  throw new Error(
    "Pass the absolute path of the verified Windows x64 native module",
  );
const selected = process.argv.slice(3);
for (const folder of readdirSync(path.join(root, "plugins")).sort()) {
  if (selected.length && !selected.includes(folder)) continue;
  const directory = path.join(root, "plugins", folder);
  const filename = path.join(directory, "package.json");
  if (!existsSync(filename)) continue;
  const manifest = JSON.parse(readFileSync(filename, "utf8"));
  if (manifest.echobird.worker?.runtime !== "native") continue;
  const target = path.join(directory, manifest.echobird.worker.entry);
  mkdirSync(path.dirname(target), { recursive: true });
  copyFileSync(binary, target);
  copyFileSync(path.join(root, "LICENSE"), path.join(directory, "LICENSE"));
  copyFileSync(
    path.join(
      root,
      "native/account-engine/licenses/workbuddy-switch-LICENSE.txt",
    ),
    path.join(directory, "NOTICE.txt"),
  );
  const { bytes, sha256 } = packagePlugin(directory);
  const archive = path.join(
    root,
    "packages",
    `${manifest.publisher}.${manifest.name}-${manifest.version}.zip`,
  );
  if (existsSync(archive) && !readFileSync(archive).equals(Buffer.from(bytes)))
    throw new Error(
      "Published package contents changed; increment the version before rebuilding",
    );
  writeFileSync(archive, bytes);
  console.log(
    JSON.stringify({
      file: path.basename(archive),
      bytes: bytes.length,
      sha256,
    }),
  );
}
