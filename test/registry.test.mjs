import assert from "node:assert/strict";
import {
  mkdtempSync,
  writeFileSync,
  mkdirSync,
  unlinkSync,
  rmdirSync,
  readFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { parseDocument } from "yaml";
import { buildIndex } from "../scripts/build-index.mjs";
import {
  readJson,
  root,
  validateEntry,
  validateIndex,
  validateLocalManifest,
  validateManifest,
  loadRegistry,
} from "../scripts/validate.mjs";

const example = readJson(path.join(root, "examples/registry-entry.json"));
const copy = () => structuredClone(example);

test("the template has a valid manifest and a local page", () => {
  assert.equal(
    validateLocalManifest(
      path.join(root, "templates/hello-plugin/package.json"),
    ).echobird.apiVersion,
    "0.1-draft",
  );
});

test("the public index contains only reviewed records and no placeholder examples", () => {
  const entries = loadRegistry();
  assert.ok(entries.every((entry) => !entry.id.startsWith("example.")));
  assert.deepEqual(
    readJson(path.join(root, "index.json")),
    buildIndex(entries),
  );
});

test("the record example is structurally valid without publishing it", () => {
  assert.equal(validateEntry(copy()).id, "example.hello-plugin");
});

for (const [name, mutate] of [
  [
    "an invalid version",
    (entry) => {
      entry.releases[0].manifest.version = "latest";
    },
  ],
  [
    "an identity mismatch",
    (entry) => {
      entry.id = "another.hello-plugin";
    },
  ],
  [
    "a repository mismatch",
    (entry) => {
      entry.repository = "https://gitcode.com/example/other";
    },
  ],
  [
    "a moving branch instead of a commit",
    (entry) => {
      entry.releases[0].commit = "main";
    },
  ],
  [
    "a missing checksum",
    (entry) => {
      delete entry.releases[0].sha256;
    },
  ],
  [
    "a missing China artifact",
    (entry) => {
      delete entry.releases[0].downloads.china;
    },
  ],
  [
    "an insecure download",
    (entry) => {
      entry.releases[0].downloads.china = "http://example.cn/plugin.zip";
    },
  ],
  [
    "credentials in a URL",
    (entry) => {
      entry.releases[0].downloads.international =
        "https://user:secret@example.com/plugin.zip";
    },
  ],
  [
    "an expiring query URL",
    (entry) => {
      entry.releases[0].downloads.china =
        "https://example.cn/plugin.zip?token=secret";
    },
  ],
  [
    "an unknown platform",
    (entry) => {
      entry.releases[0].manifest.echobird.platforms = ["unknown"];
    },
  ],
  [
    "an unknown API version",
    (entry) => {
      entry.releases[0].manifest.echobird.apiVersion = "stable";
    },
  ],
  [
    "a traversal path",
    (entry) => {
      entry.releases[0].manifest.echobird.contributes.pages[0].entry =
        "../outside.html";
    },
  ],
  [
    "a Windows absolute path",
    (entry) => {
      entry.releases[0].manifest.echobird.contributes.pages[0].entry =
        "C:\\outside.html";
    },
  ],
  [
    "an absolute page path",
    (entry) => {
      entry.releases[0].manifest.echobird.contributes.pages[0].entry =
        "/outside.html";
    },
  ],
  [
    "duplicate page ids",
    (entry) => {
      const pages = entry.releases[0].manifest.echobird.contributes.pages;
      pages.push(structuredClone(pages[0]));
    },
  ],
  [
    "duplicate release versions",
    (entry) => {
      entry.releases.push(structuredClone(entry.releases[0]));
    },
  ],
]) {
  test(`rejects ${name}`, () => {
    const entry = copy();
    mutate(entry);
    assert.throws(() => validateEntry(entry));
  });
}

test("the index rejects duplicate plugin identities", () => {
  const index = buildIndex([copy()]);
  index.plugins.push(copy());
  assert.throws(() => validateIndex(index), /Duplicate plugin id/);
});

test("index output is deterministic and sorts versions without mutating input", () => {
  const entry = copy();
  const newer = structuredClone(entry.releases[0]);
  newer.manifest.version = "0.2.0";
  entry.releases.push(newer);
  const input = structuredClone(entry);
  const built = buildIndex([entry]);
  assert.equal(built.plugins[0].releases[0].manifest.version, "0.2.0");
  assert.deepEqual(entry, input);
  assert.deepEqual(buildIndex([entry]), built);
});

test("arbitrary public Git hosts use the same schema", () => {
  for (const host of [
    "github.com",
    "gitcode.com",
    "cnb.cool",
    "git.example.org",
  ]) {
    const entry = copy();
    entry.repository = `https://${host}/someone/plugin`;
    entry.releases[0].manifest.repository = entry.repository;
    assert.doesNotThrow(() => validateEntry(entry));
  }
});

test("registry publication refuses example records", (t) => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "echobird-registry-"));
  const filename = path.join(directory, `${example.id}.json`);
  t.after(() => {
    unlinkSync(filename);
    rmdirSync(directory);
  });
  writeFileSync(filename, JSON.stringify(example));
  assert.throws(() => loadRegistry(directory), /Example records/);
});

test("publication accepts a valid record and rejects a mismatched filename", (t) => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "echobird-publish-"));
  const entry = copy();
  entry.id = "fixture.hello-plugin";
  entry.releases[0].manifest.publisher = "fixture";
  entry.releases[0].commit = "a".repeat(40);
  entry.releases[0].sha256 = "b".repeat(64);
  const filename = path.join(directory, `${entry.id}.json`);
  const wrongFilename = path.join(directory, "wrong.json");
  t.after(() => {
    unlinkSync(filename);
    unlinkSync(wrongFilename);
    rmdirSync(directory);
  });
  writeFileSync(filename, JSON.stringify(entry));
  assert.equal(loadRegistry(directory).length, 1);
  writeFileSync(wrongFilename, JSON.stringify(entry));
  assert.throws(() => loadRegistry(directory), /filename must match/);
});

test("a local manifest refuses a missing page without running plugin code", (t) => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "echobird-manifest-"));
  mkdirSync(path.join(directory, "ui"));
  const filename = path.join(directory, "package.json");
  t.after(() => {
    unlinkSync(filename);
    rmdirSync(path.join(directory, "ui"));
    rmdirSync(directory);
  });
  writeFileSync(filename, JSON.stringify(example.releases[0].manifest));
  assert.throws(() => validateLocalManifest(filename), /ENOENT/);
});

test("the draft permits standard package metadata", () => {
  const manifest = structuredClone(example.releases[0].manifest);
  manifest.keywords = ["echobird-plugin"];
  manifest.scripts = { build: "not-executed-by-validator" };
  assert.doesNotThrow(() => validateManifest(manifest));
});

test("CI uses read-only fork checks and a main-only mirror job", () => {
  const workflow = parseDocument(
    readFileSync(path.join(root, ".github/workflows/check.yml"), "utf8"),
  );
  assert.deepEqual(workflow.errors, []);
  const value = workflow.toJS();
  assert.equal(value.permissions.contents, "read");
  assert.deepEqual(value.jobs.validate.strategy.matrix.os, [
    "ubuntu-latest",
    "windows-latest",
    "macos-latest",
  ]);
  assert.match(value.jobs.mirror.if, /github.event_name != 'pull_request'/);
  assert.match(value.jobs.mirror.if, /refs\/heads\/main/);
  const commands = value.jobs.mirror.steps
    .map((step) => step.run ?? "")
    .join("\n");
  assert.ok(!commands.includes("--force"));
  assert.match(commands, /refs\/heads\/main/);
});
