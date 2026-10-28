# Native account extensions

This module contains the account login, switch, quota, proxy, speech and conversation migration implementations used by the eight community packages. EchoBird supplies the installed application paths and an authenticated local transport. The native module restores the existing account controls when installed and enabled; it contributes no application page or manual launch entry.

The module preserves the existing `~/.echobird` account files and encryption format. Account secrets remain inside the extension. EchoBird sends saved account IDs, request data and audio; account list replies contain display summaries. Installation does not sign in, query quota, apply an account, send audio or merge history. Login and mutation operations require an explicit user action. Disabling the extension cancels its process tree and login windows, while keeping saved account data.

## Build and checks

Install Rust, the platform's Tauri build prerequisites and Node.js 24 for registry tooling. From this repository:

```sh
cargo fmt --manifest-path native/account-engine/Cargo.toml --check
cargo clippy --manifest-path native/account-engine/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path native/account-engine/Cargo.toml
cargo build --release --manifest-path native/account-engine/Cargo.toml
node scripts/package-accounts.mjs /absolute/path/to/echobird-account-engine.exe trae-accounts trae-cn-accounts
npm run check
```

The first archives are Windows x64 previews, requiring the unreleased EchoBird native extension host and the system WebView2 runtime. Source branches for macOS and Linux are retained, but those binaries and native client behavior are not verified. Desktop applications unavailable from their vendor on an OS do not become available through this module. Do not publish an OS declaration until its binary and requested operations have been verified.

One package declares one application. `bin/echobird-account-engine.exe` is built from this directory. Each package carries the original application icon and license notices. Any code or icon changes require a new immutable package version. Review and index generation follow the ordinary Hub submission process.

## Transport

`worker.runtime: "native"` uses a persistent host-owned process. The host supplies one JSON configuration line on stdin: `{provider,secret,tools}`. The module prints `{protocol:1,port}` on stdout after binding only `127.0.0.1` to an ephemeral port. Diagnostic output must stay on stderr. The host holds stdin open for the session; EOF exits the module and its browser windows.

Authenticated `POST /operation` takes `{operation,params}`. Public operations must also be declared in the installed manifest. `accounts.saved`, `accounts.options`, `connection` and `metadata` are internal host calls. `GET /voice/{accountId}` upgrades to an authenticated WebSocket for supported real-time PCM input, emitting normalized `{text,finalResult}` events. Stream failures use standard error status, model scope and retry metadata; vendor error decoding stays in this module.

This is trusted native code with the current user's file, process and network access. The process boundary is for integration, lifecycle and cancellation; it is not a security sandbox. Maintainer review is required before indexing an extension. A repository topic does not grant publication rights.

Protocol implementations and storage utilities retain their existing MIT notices. See [the WorkBuddy notice](licenses/workbuddy-switch-LICENSE.txt) and [repository license](../../LICENSE).

Pass package folder names after the binary to package only those new releases. Omitting them processes all native packages; previously published versions remain immutable.
