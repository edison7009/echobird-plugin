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

test("optional categories accept the five Hub groups and reject unsupported values", () => {
  for (const category of [
    "application",
    "account",
    "appearance",
    "feature",
    "other",
  ]) {
    const manifest = copy().releases[0].manifest;
    manifest.category = category;
    assert.doesNotThrow(() => validateManifest(manifest));
  }
  const manifest = copy().releases[0].manifest;
  manifest.category = "unsupported";
  assert.throws(() => validateManifest(manifest));
});

test("provider contributions target an existing page and unique provider IDs", () => {
  const manifest = copy().releases[0].manifest;
  manifest.category = "account";
  manifest.echobird.contributes.providers = [
    {
      id: "claude-code",
      title: "Claude Code accounts",
      applications: ["claudecode"],
      pageId: "hello",
      searchTerms: ["Claude Code"],
    },
  ];
  assert.doesNotThrow(() => validateManifest(manifest));
  manifest.echobird.contributes.providers[0].pageId = "missing";
  assert.throws(() => validateManifest(manifest), /page id does not exist/);
  manifest.echobird.contributes.providers[0].pageId = "hello";
  manifest.echobird.contributes.providers.push(
    structuredClone(manifest.echobird.contributes.providers[0]),
  );
  assert.throws(() => validateManifest(manifest), /Duplicate provider id/);
});

test("account code requires an explicit permission and declared operations", () => {
  const manifest = copy().releases[0].manifest;
  manifest.category = "account";
  manifest.echobird.worker = { entry: "account.mjs" };
  manifest.echobird.contributes.providers = [
    {
      id: "fixture-account",
      title: "Fixture account",
      applications: ["fixture"],
      pageId: "hello",
      operations: ["accounts", "login.start"],
    },
  ];
  assert.throws(() => validateManifest(manifest), /permission/);
  manifest.echobird.permissions.push("account.code");
  assert.doesNotThrow(() => validateManifest(manifest));
  manifest.echobird.contributes.pages = [];
  delete manifest.echobird.contributes.providers[0].pageId;
  assert.doesNotThrow(() => validateManifest(manifest));
  manifest.echobird.contributes.providers[0].operations = [
    "undeclared-operation",
  ];
  assert.throws(() => validateManifest(manifest));
  manifest.echobird.contributes.providers[0].operations = ["accounts"];
  manifest.echobird.worker.entry = "../outside.mjs";
  assert.throws(() => validateManifest(manifest));
});

test("discovery icons require both public HTTPS regional URLs", () => {
  const entry = copy();
  entry.icons = {
    international: "https://example.com/icon.svg",
    china: "https://example.cn/icon.svg",
  };
  assert.doesNotThrow(() => validateEntry(entry));
  entry.icons.china = "file:///private/icon.svg";
  assert.throws(() => validateEntry(entry));
  delete entry.icons.china;
  assert.throws(() => validateEntry(entry));
});

test("source descriptions are optional display metadata and preserve release manifests", () => {
  const entry = copy();
  const releases = structuredClone(entry.releases);
  entry.descriptions = {
    international: "English source description",
    china: "中文来源说明",
  };
  assert.doesNotThrow(() => validateEntry(entry));
  assert.deepEqual(entry.releases, releases);
  for (const invalid of [
    { international: "English only" },
    { international: "English", china: "中文", other: "extra" },
    { international: "", china: "中文" },
    { international: "English", china: "x".repeat(501) },
    { international: 42, china: "中文" },
  ]) {
    entry.descriptions = invalid;
    assert.throws(() => validateEntry(entry));
  }
  delete entry.descriptions;
  assert.doesNotThrow(() => validateEntry(entry));
});

test("client icon hints opt into a safe built-in filename without requiring remote icons", () => {
  const entry = copy();
  delete entry.icons;
  for (const file of ["workbuddy.png", "claudecode.svg"]) {
    entry.clientIcon = file;
    assert.doesNotThrow(() => validateEntry(entry));
  }
  for (const file of [
    "../private.png",
    "/icons/tools/a.svg",
    "https://example.com/a.png",
    "a.exe",
    "a.png?token=1",
    "",
  ]) {
    entry.clientIcon = file;
    assert.throws(() => validateEntry(entry));
  }
  delete entry.clientIcon;
  assert.doesNotThrow(() => validateEntry(entry));
});

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
