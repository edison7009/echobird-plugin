# EchoBird Plugin Hub

[简体中文](README.zh-CN.md)

Plugin specifications, a development template and the reviewed plugin index for EchoBird.

**Status:** the manifest and API are a draft (`0.1-draft`). EchoBird does not yet implement this plugin runtime. The template can be previewed in a browser; it does not establish compatibility with a released desktop client. The public index starts empty.

## Regions

| Region                          | Repository                                                |
| ------------------------------- | --------------------------------------------------------- |
| International — source of truth | [GitHub](https://github.com/edison7009/echobird-plugin)   |
| China — distribution mirror     | [GitCode](https://gitcode.com/edison7009/echobird-plugin) |

Both repositories publish the same `index.json`, plugin identities and versions. Each indexed release supplies international and China download URLs with one shared SHA-256 checksum. Git synchronization does not copy release attachments, Issues or Pull Requests.

## Start here

- [Plugin manifest specification](docs/plugin-spec.md)
- [Registry and index specification](docs/registry-spec.md)
- [Hello Plugin template](templates/hello-plugin/README.md)
- [Submit or update a plugin](CONTRIBUTING.md)
- [Regional synchronization](docs/mirroring.md)

## Validate locally

Use Node.js 24 or later:

```sh
npm ci --ignore-scripts
npm run check
```

After changing reviewed records in `registry/`, run `npm run build:index` and commit the resulting `index.json` in the same PR. Index generation is deterministic. Examples remain under `examples/` and `templates/`; they are never catalog listings.

GitHub CI checks formatting, JSON Schemas, the template, deterministic output and regression cases on Windows, macOS and Linux. Validation treats plugin content as data and never installs or runs submitted plugins. After all checks pass on `main`, CI pushes that exact commit to GitCode without forcing divergent history.

Authors may add the repository topic `echobird-plugin` for discovery. A topic does not grant a publisher identity or publish a plugin in this Hub. Publisher ownership and the first listing require review.

Repository tooling and the template use the [MIT license](LICENSE). Plugins retain their own declared licenses.
