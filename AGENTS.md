# EchoBird Plugin Hub

- GitHub `main` is the source of truth. GitCode receives the same commits as the China distribution repository.
- Keep plugin specifications, examples and generated indexes consistent. Run `npm ci --ignore-scripts` and `npm run check` before a push.
- Only reviewed, installable packages enter `registry/`. The first-party Reversi package is the first real listing; templates and placeholder examples stay outside the index.
- Treat submitted JSON as data. Validation must not install packages, import plugin modules, execute lifecycle scripts or contact plugin servers.
- Keep the API marked `0.1-draft` while the client runtime is unreleased. Describe supported capabilities precisely; development-build checks are not a released-client or all-platform guarantee.
- Use EchoBird terminology in code, comments and documentation. Do not name other products or reference implementations.
- Keep changes focused. Never store credentials, authentication files or internal development reports here.
- Do not force-push or overwrite divergent China commits. Route submissions into the GitHub review process.
- Commit and push only within the current user's authorization.

## Platform scope

The Node.js registry tooling and static page template target Windows, macOS and Linux. CI validates all three operating systems. A CI pass is not a native EchoBird plugin runtime test.
