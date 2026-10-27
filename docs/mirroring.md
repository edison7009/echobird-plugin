# Regional synchronization

GitHub `main` is authoritative. GitCode `main` receives the same validated commit. Contributors can submit on either platform, while reviewed code and index changes enter the GitHub main line.

## Automatic main synchronization

The GitHub **Validate registry** workflow runs the full local checks on Windows, macOS and Linux. Its mirror job runs only for a trusted `main` push or a manual run on `main`, after every validation job succeeds. It checks out the validated commit and pushes it to:

```text
https://gitcode.com/edison7009/echobird-plugin.git
```

The repository secret `GITCODE_TOKEN` supplies GitCode write authentication. It is configured in GitHub **Settings → Secrets and variables → Actions**. Use a credential limited to this repository's required write access when the platform supports that scope. The workflow uses a temporary askpass helper; no credential is put into the Git URL, committed files or normal logs. Never include a real credential in a contribution.

There is no force push, branch deletion or reverse synchronization. Divergent GitCode commits cause an explicit failure for maintainers to reconcile. A pull request cannot run the mirror job or receive its credential.

## Manual synchronization

After the same checks pass, an authenticated maintainer can push the same commit:

```sh
git push origin main
git push gitcode main
```

Configure `origin` as GitHub and `gitcode` as the China repository. Git credentials must be configured outside this repository. Do not put tokens in remote URLs.

## Distribution limits

Git synchronization copies the main branch's Git files and history. It does not mirror Issues, Pull Requests, platform settings, Git tags or release attachments. Initial repository setup contains no released plugin packages. Future plugin releases need their regional artifacts uploaded separately, verified against the shared checksum, and reviewed before adding download URLs to the index. Tag and release synchronization is outside this initial workflow.

Two successful branch pushes establish repository synchronization; they do not prove China-region download speed, regional package availability or client plugin loading.
