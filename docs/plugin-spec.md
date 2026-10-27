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

## Page host

Each installed, enabled page contributes a separate application-desktop entry. The user selects a model and explicitly launches it. The host opens a dedicated private WebView, serves only that installed package, denies navigation to other origins and new windows, and applies a host CSP. Scripts, styles, fonts and images must be packaged locally; inline script/style and data images are supported. Direct network requests, nested frames and form submissions are blocked. This is a WebView/IPC boundary, not a native process sandbox or a guarantee against system-WebView vulnerabilities. No general filesystem, shell, clipboard, account or arbitrary application-command API is available.

The injected `window.echobird` object is described by [the type declarations](../sdk/echobird.d.ts):

```js
const { context, generateText } = window.echobird;
const answer = await generateText("A short prompt");
```

`context` contains only the app locale, plugin id and selected model's display name. `generateText` requires the reviewed `model.request` permission and uses the user's launch-time selection. It returns final text from an OpenAI-compatible or Anthropic text endpoint. Keys remain in the host; the page cannot enumerate models or substitute endpoints. Prompts are limited to 32 KiB, final responses to 1 MiB, with one pending request per window and a 60-second deadline. Closing, disabling, updating or uninstalling cancels that plugin's pending requests. Installation and passive navigation never request a model. Providers may charge for explicitly requested inference.

`close()` and `minimize()` affect only the current plugin window. Other permissions are rejected by this first runtime. Changes to permissions require a new installation review. No plugin lifecycle scripts or package-manager commands execute during import or installation. There is no persistent host-data API in this draft; private WebView storage is not a persistence contract.

The first draft describes pages only. Commands, tools, providers, panels, lifecycle hooks and executable entry points need an implemented and documented host API before they become part of this specification. Stable compatibility ranges will be added with a released host API.

Platform declarations are author claims. Validate each supported OS before publishing that claim; registry CI verifies schema/tooling portability, not a third-party plugin's native behavior.
