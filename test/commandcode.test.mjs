import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const entry = path.join(root, "plugins/commandcode/service.mjs");
const key = "fixture-key";

async function start(t) {
  const home = mkdtempSync(path.join(tmpdir(), "commandcode-"));
  const requests = [];
  const upstream = createServer(async (request, response) => {
    let body = "";
    for await (const chunk of request) body += chunk;
    requests.push({ url: request.url, body, headers: request.headers });
    const send = (value, status = 200) => {
      response.writeHead(status, { "Content-Type": "application/json" });
      response.end(JSON.stringify(value));
    };
    if (request.headers.authorization !== `Bearer ${key}`) return send({}, 401);
    if (request.url === "/alpha/whoami")
      return send({ user: { email: "a@example.test" } });
    if (request.url === "/alpha/billing/subscriptions")
      return send({ data: { planId: "individual-pro-v1", status: "active" } });
    if (request.url === "/alpha/billing/credits")
      return send({ credits: { monthlyCredits: 3, freeCredits: 2 } });
    if (request.url === "/provider/v1/models")
      return send({
        data: [
          { id: "open", supported_endpoints: ["/v1/chat/completions"] },
          { id: "claude-x", supported_endpoints: ["/v1/messages"] },
          { id: "plain" },
        ],
      });
    if (request.url === "/provider/v1/chat/completions")
      return send({ id: "done" });
    send({}, 404);
  });
  await new Promise((resolve) => upstream.listen(0, "127.0.0.1", resolve));
  const child = spawn(process.execPath, [entry], {
    env: {
      ...process.env,
      HOME: home,
      USERPROFILE: home,
      ECHOBIRD_COMMANDCODE_API: `http://127.0.0.1:${upstream.address().port}`,
    },
    stdio: ["pipe", "pipe", "inherit"],
  });
  t.after(() => {
    child.stdin.end();
    upstream.close();
  });
  child.stdin.write(
    `${JSON.stringify({ provider: "commandcode", secret: "s" })}\n`,
  );
  const line = await new Promise((resolve) =>
    child.stdout.once("data", (data) => resolve(String(data))),
  );
  const { protocol, port } = JSON.parse(line);
  assert.equal(protocol, 1);
  const call = async (operation, params = {}, secret = "s") => {
    const response = await fetch(`http://127.0.0.1:${port}/operation`, {
      method: "POST",
      headers: { authorization: secret },
      body: JSON.stringify({ operation, params }),
    });
    return { status: response.status, value: await response.json() };
  };
  const signIn = async () => {
    const { value } = await call("login.start");
    const url = new URL(value.authorizationUrl);
    const answer = await fetch(url.searchParams.get("callback"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        state: url.searchParams.get("state"),
        apiKey: key,
      }),
    });
    assert.equal(answer.status, 200);
    return (await call("login.poll", { loginId: value.loginId })).value;
  };
  const file = (...parts) =>
    JSON.parse(readFileSync(path.join(home, ...parts), "utf8"));
  return { call, signIn, file, home, requests };
}

test("requests without the session secret are refused", async (t) => {
  const { call } = await start(t);
  assert.equal((await call("accounts", {}, "wrong")).status, 401);
});

test("login saves an account and never returns its key", async (t) => {
  const { call, signIn } = await start(t);
  assert.equal((await call("accounts")).value.length, 0);
  const pending = (await call("login.start")).value;
  assert.ok(pending.expiresAt < Date.now() / 100);
  await call("login.cancel", { loginId: pending.loginId });
  const account = await signIn();
  assert.equal(account.email, "a@example.test");
  assert.equal(account.plan, "Pro");
  assert.equal(account.remaining, 5);
  assert.equal(account.active, false);
  assert.ok(!JSON.stringify((await call("accounts")).value).includes(key));
  const [option] = (await call("accounts.options")).value;
  assert.equal(option.source.provider, "plugin:commandcode");
  assert.deepEqual(option.modelLabels, {});
  assert.deepEqual(option.freeModelIds, []);
});

test("a forged login callback is refused", async (t) => {
  const { call } = await start(t);
  const { value } = await call("login.start");
  const callback = new URL(value.authorizationUrl).searchParams.get("callback");
  const answer = await fetch(callback, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ state: "forged", apiKey: key }),
  });
  assert.equal(answer.status, 403);
  assert.equal(
    (await call("login.poll", { loginId: value.loginId })).value,
    null,
  );
});

test("model apply keeps user data, hides the key and restores", async (t) => {
  const { call, file, home } = await start(t);
  mkdirSync(path.join(home, ".commandcode"));
  writeFileSync(
    path.join(home, ".commandcode/settings.json"),
    JSON.stringify({ model: "own/model", theme: "dark" }),
  );
  writeFileSync(
    path.join(home, ".commandcode/providers.json"),
    JSON.stringify({ provider: { own: { baseURL: "https://own.test" } } }),
  );
  const model = {
    name: "Fixture",
    model: "m1",
    baseUrl: "https://up.test/v1",
    apiKey: "secret-value",
    protocol: "openai",
  };
  assert.equal((await call("model.apply", model)).status, 200);
  assert.equal(
    (await call("model.apply", { ...model, model: "m2" })).status,
    200,
  );
  const providers = file(".commandcode", "providers.json");
  assert.ok(providers.provider.own);
  assert.equal(providers.provider.echobird.baseURL, "https://up.test/v1");
  assert.ok(providers.provider.echobird.apiKey.startsWith("!node -p"));
  assert.ok(!JSON.stringify(providers).includes("secret-value"));
  assert.equal(file(".commandcode", "settings.json").model, "echobird/m2");
  assert.equal(file(".commandcode", "settings.json").theme, "dark");
  const read = (await call("model.read")).value;
  assert.equal(read.model, "m2");
  assert.equal(read.apiKey, undefined);
  await call("model.restore");
  assert.deepEqual(file(".commandcode", "settings.json"), {
    model: "own/model",
    theme: "dark",
  });
  assert.equal(
    file(".commandcode", "providers.json").provider.echobird,
    undefined,
  );
});

test("a keyless model is written without a key command", async (t) => {
  const { call, file } = await start(t);
  await call("model.apply", {
    model: "m",
    baseUrl: "http://127.0.0.1:1/v1",
    protocol: "openai",
  });
  const { echobird } = file(".commandcode", "providers.json").provider;
  assert.equal(echobird.apiKey, false);
});

test("broken user settings are refused without being rewritten", async (t) => {
  const { call, home } = await start(t);
  const settings = path.join(home, ".commandcode/settings.json");
  mkdirSync(path.dirname(settings));
  writeFileSync(settings, "{broken");
  const result = await call("model.apply", {
    model: "m",
    baseUrl: "https://up.test/v1",
  });
  assert.equal(result.status, 400);
  assert.equal(readFileSync(settings, "utf8"), "{broken");
});

test("applying an account signs in the client and leaves model selection", async (t) => {
  const { call, signIn, file } = await start(t);
  const account = await signIn();
  await call("model.apply", { model: "m", baseUrl: "https://up.test/v1" });
  const applied = (await call("account.apply", { accountId: account.id }))
    .value;
  assert.equal(applied.active, true);
  assert.equal(file(".commandcode", "auth.json").apiKey, key);
  assert.equal(file(".commandcode", "settings.json").model, undefined);
});

test("models and proxy use the saved account", async (t) => {
  const { call, signIn, requests } = await start(t);
  const account = await signIn();
  const list = await call("models", { accountId: account.id });
  assert.deepEqual(list.value, ["open", "plain"]);
  const reply = await call("proxy", {
    accountId: account.id,
    model: "open",
    body: { messages: [] },
  });
  assert.deepEqual(reply.value, { id: "done" });
  const sent = requests.at(-1);
  assert.equal(JSON.parse(sent.body).model, "open");
  assert.equal(sent.headers["x-api-key"], key);
  assert.equal(
    (await call("models", { accountId: "missing" })).value.error.message,
    "accountError.invalidAccount",
  );
});

test("plans use the names Command Code shows", async (t) => {
  const { call, signIn, home } = await start(t);
  const account = await signIn();
  const file = path.join(home, ".echobird/commandcode-accounts.json");
  const stored = JSON.parse(readFileSync(file, "utf8"));
  for (const [id, name] of [
    ["individual-go-v1", "Go"],
    ["individual_goat", "GOAT"],
    ["individual-pro", "Pro"],
    ["individual-max", "Max"],
    ["individual-max-10x", "Max 10×"],
    ["individual-max-20x", "Max 20×"],
    ["individual-ultra", "Ultra"],
    ["teams-pro", "Teams Pro"],
  ]) {
    stored.accounts[0].plan = id;
    writeFileSync(file, JSON.stringify(stored));
    const [row] = (await call("accounts")).value;
    assert.equal(row.id, account.id);
    assert.equal(row.plan, name);
  }
});
