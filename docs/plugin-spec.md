# Plugin manifest — draft 1

Status: `0.1-draft`. This document defines metadata and a minimal page declaration. Host loading, lifecycle, isolation and permission enforcement are not implemented by the current EchoBird client. This draft does not promise compatibility with a released client version.

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

The template runs as a standalone static browser page. It makes no host API calls and carries its own Content Security Policy. That policy is not an EchoBird sandbox implementation. Do not infer file, network, account, model, clipboard or process access from a declaration.

The first draft describes pages only. Future commands, tools, providers, panels, lifecycle hooks and executable entry points need an implemented and documented host API before they become part of this specification. Stable host compatibility ranges will be added with that API; no speculative minimum application version is declared here.

Platform declarations are author claims. Validate each supported OS before publishing that claim; registry CI verifies schema/tooling portability, not a third-party plugin's native behavior.
