# EchoBird Plugin Hub

- GitHub `main` is the source of truth. GitCode receives the same commits as the China distribution repository.
- Keep plugin specifications, examples and generated indexes consistent. Run `npm ci --ignore-scripts` and `npm run check` before a push.
- Never add demonstration plugins to `registry/`. Keep the initial public index empty until a real submission is reviewed.
- Treat submitted JSON as data. Validation must not install packages, import plugin modules, execute lifecycle scripts or contact plugin servers.
- Preserve the draft status of the manifest and API until a client runtime implements them. Do not claim native installation, isolation or permissions are implemented.
- Use EchoBird terminology in code, comments and documentation. Do not name other products or reference implementations.
- Keep changes focused. Never store credentials, authentication files or internal development reports here.
- Do not force-push or overwrite divergent China commits. Route submissions into the GitHub review process.
- Commit and push only within the current user's authorization.

## Platform scope

The Node.js registry tooling and static page template target Windows, macOS and Linux. CI validates all three operating systems. A CI pass is not a native EchoBird plugin runtime test.
