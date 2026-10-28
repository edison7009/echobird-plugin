import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { test } from "node:test";
import { root, readJson, validateManifest } from "../scripts/validate.mjs";
import { packagePlugin } from "../scripts/package-plugin.mjs";
const theme = () =>
  readJson(path.join(root, "plugins/ocean-theme/package.json"));
const tools = () =>
  readJson(path.join(root, "plugins/text-tools/package.json"));
test("declarative themes require approved colors and reject duplicate identities", () => {
  assert.doesNotThrow(() => validateManifest(theme()));
  for (const change of [
    (m) => {
      m.echobird.contributes.themes[0].light.css = "#123456";
    },
    (m) => {
      m.echobird.contributes.themes[0].dark.accent = "red";
    },
    (m) => {
      m.echobird.contributes.themes.push(m.echobird.contributes.themes[0]);
    },
  ]) {
    const m = theme();
    change(m);
    assert.throws(() => validateManifest(m));
  }
});
test("background code and text hooks are bounded to declared permission and entry contracts", () => {
  assert.doesNotThrow(() => validateManifest(tools()));
  for (const permission of [
    "extension.background",
    "configuration",
    "commands",
    "text.process",
  ]) {
    const m = tools();
    m.echobird.permissions = m.echobird.permissions.filter(
      (p) => p !== permission,
    );
    assert.throws(() => validateManifest(m));
  }
  for (const change of [
    (m) => {
      m.echobird.background = "../outside.mjs";
    },
    (m) => {
      delete m.echobird.background;
    },
    (m) => {
      m.echobird.contributes.commands[0].pageId = "missing";
    },
    (m) => {
      m.echobird.contributes.textProcessors[0].stage = "audio.raw";
    },
    (m) => {
      m.echobird.contributes.configuration[0].default = "yes";
    },
    (m) => {
      m.echobird.contributes.configuration[0].options = [false];
    },
  ]) {
    const m = tools();
    change(m);
    assert.throws(() => validateManifest(m));
  }
});
test("product examples package reproducibly using only local portable assets", () => {
  for (const name of ["ocean-theme", "text-tools"]) {
    const folder = path.join(root, "plugins", name);
    assert.deepEqual(packagePlugin(folder).bytes, packagePlugin(folder).bytes);
  }
});
test("text example registers without side effects and executes explicit commands and opted-in processors", async () => {
  const commands = new Map(),
    processors = new Map(),
    data = new Map(),
    events = [];
  let writes = 0;
  const settings = { "collapse-spaces": true, prefix: "" };
  let dispose;
  const host = {
    configuration: { get: async () => ({ ...settings }) },
    storage: {
      get: async (key) => data.get(key),
      set: async (key, value) => {
        data.set(key, value);
        writes++;
      },
    },
    commands: {
      register: (id, handler) => {
        commands.set(id, handler);
        return { dispose: () => commands.delete(id) };
      },
    },
    text: {
      register: (id, handler) => {
        processors.set(id, handler);
        return { dispose: () => processors.delete(id) };
      },
    },
    events: { emit: async (...event) => events.push(event) },
    lifecycle: {
      onDispose: (handler) => {
        dispose = handler;
      },
    },
  };
  vm.runInNewContext(
    readFileSync(path.join(root, "plugins/text-tools/background.mjs"), "utf8"),
    { window: { echobird: host } },
  );
  assert.equal(writes, 0);
  assert.equal(events.length, 0);
  const context = { signal: new AbortController().signal };
  assert.equal(
    await commands.get("format")({ text: "  hello    world  " }, context),
    "hello world",
  );
  assert.equal(data.get("invocations"), 1);
  assert.equal(events[0][0], "formatted");
  settings.prefix = "> ";
  assert.equal(
    await processors.get("clean-output")(
      { text: "  speech  output " },
      context,
    ),
    "> speech output",
  );
  assert.equal(writes, 1);
  const cancelled = new AbortController();
  cancelled.abort();
  await assert.rejects(
    commands.get("format")({ text: "no write" }, { signal: cancelled.signal }),
    /Cancelled/,
  );
  assert.equal(writes, 1);
  dispose();
  assert.equal(commands.size, 0);
  assert.equal(processors.size, 0);
});

test("published product examples match immutable archives and registry checksums", () => {
  for (const name of ["ocean-theme", "text-tools"]) {
    const packaged = packagePlugin(path.join(root, "plugins", name));
    const record = readJson(
      path.join(root, "registry", `echobird.${name}.json`),
    );
    assert.equal(record.releases[0].sha256, packaged.sha256);
    assert.deepEqual(record.releases[0].manifest, packaged.manifest);
    assert.deepEqual(
      new Uint8Array(
        readFileSync(path.join(root, "packages", `echobird.${name}-0.1.0.zip`)),
      ),
      packaged.bytes,
    );
  }
});
