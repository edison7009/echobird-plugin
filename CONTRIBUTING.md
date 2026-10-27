# Submit a plugin

Submit a PR to GitHub `main`, or use the **Plugin submission** Issue form. Authors who use GitCode may open an Issue there; maintainers move the reviewed change into the GitHub main line. Do not merge independent registry changes into the China mirror.

## First submission

1. Develop in your own public Git repository. Public HTTPS repository URLs are supported across Git hosts.
2. Replace the template publisher, name and repository. The stable plugin id is `<publisher>.<name>`.
3. Prepare a ZIP package with `package.json` at its root. The package must contain its page files and all runtime assets; installation must not depend on package-manager lifecycle scripts.
4. Publish the same package bytes to both distribution regions. Compute SHA-256 from the final archive.
5. Copy `examples/registry-entry.json` to `registry/<publisher>.<name>.json`. Replace every example value, including the full immutable source commit, embedded manifest, download URLs and checksum.
6. Run `npm run build:index` and `npm run check`. Submit the record and generated index together.

The current manifest/API is a draft. A schema-valid submission is not a claim that EchoBird can load it today. Maintainers review draft submissions before accepting any public listing.

## Review

Review publisher ownership, source availability, license, manifest identity, compatibility declarations and requested capabilities. Download both regional packages separately, verify their SHA-256 and inspect their contents. CI validates the JSON structure and local template; it does not download, execute or certify third-party packages.

Do not place credentials, signed temporary URLs, tracking query parameters or install instructions that require user secrets in registry records. The example publisher and zero-filled commits/checksums are refused by publication validation.

## Updates

Append a release with a new SemVer version. Published versions, source commits and archive checksums must remain unchanged. Reviewers check this against existing records. Publisher transfers, removal of a listing and changes of source repository need explicit maintainer review.

An Issue is a submission request, not an automatic listing. Repository topics provide discovery only. Validation never runs a submitted plugin or installs its dependencies.
