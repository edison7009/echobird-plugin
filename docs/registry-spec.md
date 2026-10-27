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

The same index bytes are distributed from both official repositories. Future clients select the archive URL for their chosen region and compare the final package hash before loading. Region selection, installation, archive extraction and plugin execution are not implemented in this repository.
