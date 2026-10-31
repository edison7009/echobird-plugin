// Application service: EchoBird starts this module, writes one configuration
// line to stdin, and calls it over an authenticated loopback port. EOF on
// stdin ends it. Diagnostics must never go to stdout.
import { createHash, randomBytes, timingSafeEqual } from "node:crypto";
import {
  chmodSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";

const API =
  process.env.ECHOBIRD_COMMANDCODE_API ?? "https://api.commandcode.ai";
const STUDIO = "https://commandcode.ai";
const ORIGINS = [STUDIO, "https://staging.commandcode.ai"];
const LOGIN_MS = 10 * 60 * 1000;
const PROVIDER = "echobird";

const native = join(homedir(), ".commandcode");
const paths = {
  settings: join(native, "settings.json"),
  providers: join(native, "providers.json"),
  auth: join(native, "auth.json"),
  accounts: join(homedir(), ".echobird", "commandcode-accounts.json"),
  key: join(homedir(), ".echobird", "commandcode-model-key"),
  previous: join(homedir(), ".echobird", "commandcode-model-previous.json"),
};

class Failure extends Error {}

// Plan names as the Command Code app shows them, longest id first.
const PLAN_NAMES = [
  ["individual-provider", "Provider"],
  ["individual-goat", "GOAT"],
  ["individual-ultra", "Ultra"],
  ["individual-max", "Max"],
  ["individual-pro", "Pro"],
  ["individual-go", "Go"],
  ["teams-pro", "Teams Pro"],
];

function planName(id) {
  if (!id) return null;
  const plan = String(id).toLowerCase().replaceAll("_", "-");
  const known = PLAN_NAMES.find(([prefix]) => plan.startsWith(prefix));
  if (!known) return id;
  const tier = plan.match(/-(\d+)x\b/u);
  return tier ? `${known[1]} ${tier[1]}×` : known[1];
}

function read(path, fallback) {
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch (error) {
    if (error.code === "ENOENT") return fallback;
    throw error;
  }
  try {
    return JSON.parse(text);
  } catch {
    throw new Failure("accountError.failed");
  }
}

function write(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.${randomBytes(6).toString("hex")}.tmp`;
  const text =
    typeof value === "string" ? value : JSON.stringify(value, null, 2);
  writeFileSync(temporary, text, { mode: 0o600 });
  renameSync(temporary, path);
  chmodSync(path, 0o600);
}

const accounts = () => read(paths.accounts, { accounts: [] }).accounts;
const save = (list) => write(paths.accounts, { accounts: list });
const summary = ({ key, ...entry }, active = read(paths.auth, {}).apiKey) => ({
  ...entry,
  plan: planName(entry.plan),
  active: key === active,
});

function saved(id) {
  const entry = accounts().find((candidate) => candidate.id === id);
  if (!entry) throw new Failure("accountError.invalidAccount");
  return entry;
}

async function account(path, key, timeout = 15_000) {
  const response = await fetch(API + path, {
    headers: { Authorization: `Bearer ${key}`, Accept: "application/json" },
    signal: AbortSignal.timeout(timeout),
  });
  if (response.status === 401 || response.status === 403)
    throw new Failure("accountError.loginRequired");
  if (!response.ok) throw new Failure("accountError.failed");
  return response.json();
}

async function details(key) {
  const user = (await account("/alpha/whoami", key))?.user ?? {};
  const [subscription, credits] = await Promise.allSettled([
    // This endpoint can take over 15 seconds.
    account("/alpha/billing/subscriptions", key, 30_000),
    account("/alpha/billing/credits", key),
  ]);
  const plan = subscription.value?.data;
  const left = credits.value?.credits;
  return {
    id: createHash("sha256").update(key).digest("hex"),
    key,
    email: user.email || user.userName || user.id || "Command Code",
    plan: planName(
      (plan?.status !== "canceled" &&
        plan?.status !== "incomplete_expired" &&
        plan?.planId) ||
        left?.planId,
    ),
    remaining: left
      ? (left.monthlyCredits ?? 0) +
        (left.purchasedCredits ?? 0) +
        (left.freeCredits ?? 0)
      : null,
  };
}

function store(entry) {
  save([...accounts().filter((other) => other.id !== entry.id), entry]);
  return entry;
}

const logins = new Map();

async function startLogin() {
  const loginId = randomBytes(16).toString("hex");
  const state = randomBytes(24).toString("base64url");
  const login = { result: undefined, error: undefined };
  const server = createServer(async (request, response) => {
    const origin = ORIGINS.includes(request.headers.origin)
      ? request.headers.origin
      : ORIGINS[0];
    response.setHeader("Access-Control-Allow-Origin", origin);
    response.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
    response.setHeader("Access-Control-Allow-Headers", "Content-Type");
    if (request.headers["access-control-request-private-network"] === "true")
      response.setHeader("Access-Control-Allow-Private-Network", "true");
    if (request.method === "OPTIONS") return response.writeHead(204).end();
    const path = new URL(request.url, "http://127.0.0.1").pathname;
    if (request.method !== "POST" || path !== "/callback")
      return response.writeHead(404).end();
    let raw = "";
    for await (const chunk of request) {
      raw += chunk;
      if (raw.length > 10_000) return response.writeHead(413).end();
    }
    const json = String(request.headers["content-type"])
      .toLowerCase()
      .startsWith("application/json");
    let answer = {};
    try {
      answer = json
        ? JSON.parse(raw)
        : Object.fromEntries(new URLSearchParams(raw));
    } catch {
      /* Rejected by the state check below. */
    }
    if (answer.state !== state) return response.writeHead(403).end();
    if (answer.apiKey) {
      try {
        login.result = store(await details(answer.apiKey));
      } catch (error) {
        login.error = error;
      }
    } else login.error = new Failure("accountError.failed");
    response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
    response.end(
      "<!doctype html><meta charset=utf-8><p>You can close this tab.</p>",
    );
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const callback = `http://127.0.0.1:${server.address().port}/callback`;
  const query = new URLSearchParams({ callback, state, mode: "redirect" });
  login.close = () => {
    clearTimeout(login.timer);
    server.close();
    logins.delete(loginId);
  };
  login.timer = setTimeout(login.close, LOGIN_MS);
  logins.set(loginId, login);
  return {
    loginId,
    authorizationUrl: `${STUDIO}/studio/auth/cli?${query}`,
    expiresAt: Math.floor((Date.now() + LOGIN_MS) / 1000),
  };
}

function pollLogin(id) {
  const login = logins.get(id);
  if (!login) throw new Failure("accountError.cancelled");
  if (login.error) {
    login.close();
    throw login.error;
  }
  if (!login.result) return null;
  login.close();
  return summary(login.result);
}

function restoreModel() {
  const settings = read(paths.settings, {});
  const providers = read(paths.providers, {});
  const previous = read(paths.previous, null);
  if (providers.provider?.[PROVIDER]) {
    delete providers.provider[PROVIDER];
    write(paths.providers, providers);
  }
  if (
    settings.modelProvider === PROVIDER ||
    String(settings.model).startsWith(`${PROVIDER}/`)
  ) {
    for (const field of ["model", "modelProvider"]) {
      if (previous?.[field] === undefined) delete settings[field];
      else settings[field] = previous[field];
    }
    write(paths.settings, settings);
  }
  rmSync(paths.previous, { force: true });
  rmSync(paths.key, { force: true });
}

function applyAccount(id) {
  const { key, email } = saved(id);
  const original = read(paths.auth, undefined);
  write(paths.auth, { ...original, apiKey: key, userName: email });
  try {
    restoreModel();
  } catch (error) {
    if (original === undefined) rmSync(paths.auth, { force: true });
    else write(paths.auth, original);
    throw error;
  }
  return summary(saved(id), key);
}

function applyModel({ name, model, baseUrl, anthropicUrl, apiKey, protocol }) {
  const anthropic = protocol === "anthropic";
  const url = anthropic ? anthropicUrl || baseUrl : baseUrl;
  if (!model || !url) throw new Failure("accountError.failed");
  const settings = read(paths.settings, {});
  const providers = read(paths.providers, {});
  if (settings.modelProvider !== PROVIDER)
    write(paths.previous, {
      model: settings.model,
      modelProvider: settings.modelProvider,
    });
  if (apiKey) write(paths.key, apiKey);
  else rmSync(paths.key, { force: true });
  // Command Code refuses a key written in providers.json, so it runs a command.
  const command =
    `!node -p "require('fs').readFileSync(require('path').join(` +
    `require('os').homedir(),'.echobird','commandcode-model-key'),'utf8')"`;
  write(paths.providers, {
    ...providers,
    provider: {
      ...providers.provider,
      [PROVIDER]: {
        name: "EchoBird",
        api: anthropic ? "anthropic-messages" : "openai-completions",
        baseURL: url,
        apiKey: apiKey ? command : false,
        models: { [model]: { name: name || model } },
      },
    },
  });
  write(paths.settings, {
    ...settings,
    model: `${PROVIDER}/${model}`,
    modelProvider: PROVIDER,
  });
  return {};
}

function readModel() {
  const settings = read(paths.settings, {});
  if (typeof settings.model !== "string") return null;
  const entry = read(paths.providers, {}).provider?.[PROVIDER];
  if (settings.modelProvider !== PROVIDER || !entry)
    return { model: settings.model };
  const model = settings.model.slice(PROVIDER.length + 1);
  const anthropic = entry.api === "anthropic-messages";
  return {
    name: entry.models?.[model]?.name ?? model,
    model,
    baseUrl: anthropic ? undefined : entry.baseURL,
    anthropicUrl: anthropic ? entry.baseURL : undefined,
    protocol: anthropic ? "anthropic" : "openai",
  };
}

const headers = (key) => ({
  Authorization: `Bearer ${key}`,
  "x-api-key": key,
  "Content-Type": "application/json",
});

async function models(id) {
  const { key } = saved(id);
  const response = await fetch(`${API}/provider/v1/models`, {
    headers: headers(key),
    signal: AbortSignal.timeout(15_000),
  });
  if (response.status === 401) throw new Failure("accountError.loginRequired");
  if (!response.ok) throw new Failure("accountError.failed");
  return ((await response.json())?.data ?? [])
    .filter((model) => {
      const routes = (model.supported_endpoints ?? []).map(String);
      return routes.length
        ? routes.some((route) => route.includes("/chat/completions"))
        : !model.id.startsWith("claude-");
    })
    .map((model) => model.id);
}

async function operation(name, input) {
  switch (name) {
    case "accounts":
    case "accounts.saved":
      return accounts().map((entry) => summary(entry));
    case "accounts.options":
      return accounts().map((entry) => ({
        source: { provider: "plugin:commandcode", accountId: entry.id },
        label: entry.email,
        plan: planName(entry.plan),
        modelLabels: {},
        freeModelIds: [],
      }));
    case "login.start":
      return startLogin();
    case "login.poll":
      return pollLogin(input.loginId);
    case "login.cancel":
      logins.get(input.loginId)?.close();
      return null;
    case "account.refresh":
      return summary(store(await details(saved(input.accountId).key)));
    case "account.apply":
      return applyAccount(input.accountId);
    case "account.remove":
      save(accounts().filter((entry) => entry.id !== input.accountId));
      return null;
    case "models":
      return models(input.accountId);
    case "connection":
      return `${API}/provider/v1`;
    case "metadata":
      return { labels: {}, freeModelIds: [] };
    case "model.read":
      return readModel();
    case "model.apply":
      return applyModel(input);
    case "model.restore":
      restoreModel();
      return {};
    default:
      throw new Failure("accountError.failed");
  }
}

async function proxy({ accountId, model, body }, response) {
  const upstream = await fetch(`${API}/provider/v1/chat/completions`, {
    method: "POST",
    headers: headers(saved(accountId).key),
    body: JSON.stringify({ ...body, model }),
  });
  if (!upstream.ok) {
    const message = (await upstream.text()).slice(0, 500);
    response.writeHead(upstream.status, { "Content-Type": "application/json" });
    return response.end(
      JSON.stringify({ error: { message, type: "upstream_error" } }),
    );
  }
  response.writeHead(200, {
    "Content-Type": upstream.headers.get("content-type") ?? "application/json",
    "Cache-Control": "no-cache",
  });
  Readable.fromWeb(upstream.body).pipe(response);
}

const reply = (response, status, value) => {
  response.writeHead(status, { "Content-Type": "application/json" });
  response.end(JSON.stringify(value ?? null));
};

function matches(actual = "", expected) {
  const [a, b] = [actual, expected].map((value) =>
    createHash("sha256").update(value).digest(),
  );
  return timingSafeEqual(a, b);
}

function serve({ secret }) {
  const server = createServer(async (request, response) => {
    if (
      request.method !== "POST" ||
      request.url !== "/operation" ||
      !matches(request.headers.authorization, secret)
    )
      return reply(response, 401, {
        error: { message: "accountError.failed" },
      });
    let raw = "";
    for await (const chunk of request) raw += chunk;
    try {
      const { operation: name, params } = JSON.parse(raw);
      if (name === "proxy") return await proxy(params, response);
      reply(response, 200, await operation(name, params ?? {}));
    } catch (error) {
      const message =
        error instanceof Failure ? error.message : "accountError.failed";
      if (!response.headersSent) reply(response, 400, { error: { message } });
      else response.destroy();
    }
  });
  server.listen(0, "127.0.0.1", () => {
    const ready = { protocol: 1, port: server.address().port };
    process.stdout.write(`${JSON.stringify(ready)}\n`);
  });
}

let input = "";
let started = false;
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  input += chunk;
  if (started || !input.includes("\n")) return;
  started = true;
  serve(JSON.parse(input.slice(0, input.indexOf("\n"))));
});
process.stdin.on("end", () => process.exit(0));
