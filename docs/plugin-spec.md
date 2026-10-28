# Plugin manifest — draft 1

Status: `0.1-draft`. This document defines the experimental page runtime being integrated into an unreleased EchoBird development build. It does not promise compatibility with existing released clients. [Reversi](../plugins/reversi/README.md) exercises this interface; the template also works as a standalone browser page without host APIs.

The root `package.json` uses ordinary package fields and an `echobird` namespace. Its machine-readable definition is [plugin-manifest.schema.json](../schemas/plugin-manifest.schema.json). See the [template manifest](../templates/hello-plugin/package.json).

| Field                        | Meaning                                                                                              |
| ---------------------------- | ---------------------------------------------------------------------------------------------------- |
| `name`                       | Lowercase plugin name, using letters, digits and single hyphens.                                     |
| `publisher`                  | Stable lowercase publisher name. Ownership is reviewed for a Hub listing.                            |
| `version`                    | Complete SemVer version, without a `v` prefix.                                                       |
| `displayName`, `description` | Short user-facing metadata.                                                                          |
| `license`                    | The plugin's own license declaration.                                                                |
| `repository`                 | Public HTTPS Git repository URL without credentials, query or fragment.                              |
| `echobird.manifestVersion`   | `1`, the structure version.                                                                          |
| `echobird.apiVersion`        | `0.1-draft`, a draft API identifier rather than an application version.                              |
| `echobird.platforms`         | Explicit subset of `windows`, `macos`, `linux`.                                                      |
| `echobird.permissions`       | Capability names the author declares; the template requests none. Declaration does not grant access. |
| `echobird.contributes.pages` | Page declarations: a unique local `id`, a short `title` and a relative HTML `entry`.                 |

The plugin id is `<publisher>.<name>`. A page entry stays inside the package, such as `ui/index.html`; absolute paths, backslashes and `..` segments are rejected. Local validation checks that declared page files exist as regular files. Standard package metadata such as `keywords` can coexist with this manifest.

An optional `icon` is a relative SVG, PNG or WebP file inside the package. Package paths use portable names: no traversal, absolute paths, backslashes, URL escapes, whitespace, Windows reserved device names or trailing dots/spaces. Links and special files are rejected. Packages contain at most 2,000 files/directories and 20 MiB of uncompressed content. Local directories are copied as installation snapshots; source edits require reinstallation.

An optional root `category` selects one Hub filter: `application` (应用), `account` (账号登录), `appearance` (美化), `feature` (功能), or `other` (其他). Without this field, plugins contributing pages are classified as applications. This keeps existing Reversi and AI Translate releases in Apps without changing their published manifests or archives. A category is display metadata; it does not grant capabilities or change maintainer review.

Account integrations may declare `echobird.contributes.providers`. Each record identifies supported application IDs, optional search terms, and either a page ID or code operations. This metadata lets an application's account entry open the Account login category with a targeted search. The page API retains its existing restricted permissions; account operations use the separately declared code module described below.

## Page host

Each installed, enabled page contributes a separate application-desktop entry. The user selects a model and explicitly launches it. The host opens a dedicated private WebView, serves only that installed package, denies navigation to other origins and new windows, and applies a host CSP. Scripts, styles, fonts and images must be packaged locally; inline script/style and data images are supported. Direct network requests, nested frames and form submissions are blocked. This is a WebView/IPC boundary, not a native process sandbox or a guarantee against system-WebView vulnerabilities. No general filesystem, shell, clipboard, account or arbitrary application-command API is available.

The injected `window.echobird` object is described by [the type declarations](../sdk/echobird.d.ts):

```js
const { context, generateText } = window.echobird;
const answer = await generateText("A short prompt");
```

`context` contains only the app locale, plugin id and selected model's display name. `generateText` requires the reviewed `model.request` permission and uses the user's launch-time selection. It returns final text from an OpenAI-compatible or Anthropic text endpoint. Keys remain in the host; the page cannot enumerate models or substitute endpoints. Prompts are limited to 32 KiB, final responses to 1 MiB, with one pending request per window and a 60-second deadline. Closing, disabling, updating or uninstalling cancels that plugin's pending requests. Installation and passive navigation never request a model. Providers may charge for explicitly requested inference.

`close()` and `minimize()` affect only the current plugin window. `writeClipboardText(text)` requires the separately reviewed `clipboard.write` permission and writes up to 1 MiB of text to the system clipboard. It never reads clipboard contents; calls after window closure, revocation or developer-preview pause are rejected. The browser clipboard API remains disabled. Other permissions are rejected by this first runtime. Changes to permissions require a new installation review. No plugin lifecycle scripts or package-manager commands execute during import or installation. There is no persistent host-data API in this draft; private WebView storage is not a persistence contract.

## Account code modules

Experimental modules declare an `echobird.worker.entry`, the `account.code` permission, and a provider's `operations`. They restore capabilities through the application host; users do not launch a plugin window to invoke them. The default `runtime: "node"` requires an existing Node.js executable and an entry ending in `.mjs`. `runtime: "native"` declares a packaged native executable and exactly one provider with one application. The host never downloads a language runtime or installs dependencies automatically.

Code-only modules declare `pages: []` and omit the provider's `pageId`. They contribute no application-desktop entry. A plugin with a management page can continue to declare pages and a matching page ID. The `sdk/account-worker.mjs` helper dispatches requests to an operation map and keeps diagnostic logs on stderr; copy it into the plugin package rather than importing outside the installed package.

The host starts the entry in its package directory and writes one JSON-RPC 2.0 request to stdin, then closes stdin. The module returns one JSON response on stdout and exits. Request shape: `{jsonrpc:"2.0",id,method,params:{providerId,input}}`; response shape: `{jsonrpc:"2.0",id,result}` or `{jsonrpc:"2.0",id,error}`. IDs must match. Logging must not go to stdout. Input and output are limited to 1 MiB each. The host permits one active operation per plugin, applies a 120-second deadline, and cancels calls when the plugin is disabled, removed, or Developer Preview is paused.

Declared operations are `accounts`, `login.start`, `login.complete`, `login.poll`, `login.cancel`, `account.refresh`, `account.claim`, `account.apply`, `account.remove`, `models`, `proxy`, `voice`, and `history.merge`. Declare only the operations implemented by the package. A transport declaration alone does not establish compatibility with an application's account controls.

The native account host keeps a module alive for login sessions, streaming text and audio operations. Its stdin lifetime, authenticated loopback handshake, normalized failure events and internal display-directory calls are specified in [the native module documentation](../native/account-engine/README.md). EchoBird sends account IDs rather than provider credentials. The community modules retain the existing local encryption and account formats. Removing a plugin preserves its saved accounts and cancels active capability requests. The first six packages require an unreleased host build and currently provide Windows x64 preview archives; other platform binaries and live client checks remain pending.

Account code runs as the current local user and can access that user's files, network and processes. The process boundary is not a security sandbox. Installation review explicitly shows this permission. Sensitive inherited environment variables are omitted; system paths and user-directory environment variables are retained. Only trusted, reviewed modules should receive this permission. Isolated page plugins retain their existing restricted IPC and CSP.

Stable compatibility ranges will be added with a released host API.

Platform declarations are author claims. Validate each supported OS before publishing that claim; registry CI verifies schema/tooling portability, not a third-party plugin's native behavior.
