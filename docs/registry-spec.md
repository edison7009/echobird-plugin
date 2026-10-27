# Registry and index — version 1

`registry/<publisher>.<name>.json` is the reviewed source record. `index.json` is generated from these records for Hub readers. It must not contain unreviewed examples.

| Record field                         | Meaning                                                                                     |
| ------------------------------------ | ------------------------------------------------------------------------------------------- |
| `id`                                 | Stable `<publisher>.<name>`, matching every embedded manifest.                              |
| `repository`                         | Public HTTPS source repository, matching the embedded manifests.                            |
| `releases[].manifest`                | The complete package manifest for this release.                                             |
| `releases[].commit`                  | Full 40- or 64-character lowercase Git commit id. A branch or moving label is insufficient. |
| `releases[].sha256`                  | SHA-256 of the final ZIP archive, shared by both regions.                                   |
| `releases[].downloads.international` | Persistent public HTTPS archive URL for the international region.                           |
| `releases[].downloads.china`         | Persistent public HTTPS archive URL for the China region.                                   |

See [the entry schema](../schemas/registry-entry.schema.json) and [example](../examples/registry-entry.json). Examples contain placeholders and are deliberately excluded from `registry/`.

Every archive contains `package.json` at the archive root and all required runtime files. Its manifest must equal the embedded release manifest. Source commit, manifest, archive contents and both download checksums are checked in maintainer review; the initial validator does not fetch archives or execute code. Declared checksums provide a content comparison value, not publisher verification or a security certificate.

The generated index has `schemaVersion: 1`, `name: "EchoBird Plugin Hub"` and `plugins: [...]`. Plugins sort by id; releases sort by descending SemVer precedence. Equivalent duplicate versions and duplicate plugin identities are rejected. No build timestamp is inserted, so identical records produce identical index bytes.

Generation reads only reviewed JSON records. Templates, examples and Issues are not discovery inputs. CI checks that the committed index matches generation; it does not silently commit new catalog content.

The same index bytes are distributed from both official repositories. The development client selects the archive URL for the user's region, compares the package hash before extraction, and compares the extracted manifest with this reviewed record. Plugin code runs only after explicit launch. The client implementation lives in EchoBird; this repository supplies packages and metadata.

First-party versioned ZIPs live in `packages/` and are mirrored with Git. Build Reversi with `npm run pack:reversi`. Commit its source and final archive first, then record that immutable source commit, archive digest and regional URLs in a registry change. Never replace an existing version's archive. GitCode's public raw-file API is used for file bytes; its ordinary repository page is not a package endpoint. These archives require no separate Release-attachment mirroring.

Optional registry `icons` supplies `international` and `china` public HTTPS image URLs for discovery before installation. Use versioned assets and the same original icon in both regions. Installed plugins use their local manifest icon. Omit the field when no icon is available; the client uses its generic fallback.
