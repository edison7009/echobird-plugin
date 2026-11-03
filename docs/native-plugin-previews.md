# Native account plugin previews

GitHub Actions **Native account module builds** compiles the account engine on
Windows, macOS and Linux, then packages all eight native account plugins in the
`echobird-native-account-plugin-previews` artifact. GitCode and CNB mirror the
validated source through **Validate registry**.

The artifact contains `windows-x64`, `macos-arm64`, `macos-x64`, `linux-arm64` and
`linux-x64` folders. Use the folder matching the test computer. Versions append
the platform and architecture, for example `0.1.0-preview.11.macos.arm64`.
Each manifest declares only its target OS. Windows uses
`bin/echobird-account-engine.exe`; macOS and Linux use
`bin/echobird-account-engine`.

The universal macOS executable currently exceeds the client's 20 MiB expanded
package limit, so previews contain separate Apple silicon and Intel binaries.
The client registry currently filters by OS, not CPU architecture. Do not merge
both architectures into the public registry until selection is supported.

## Test on a Mac

1. In **About This Mac**, identify Apple silicon (`macos-arm64`) or Intel
   (`macos-x64`). Download the artifact from the matching GitHub Actions run.
2. Extract the artifact, then extract each matching plugin ZIP into its own folder.
3. In EchoBird's plugin center, use **Local folder** to select the extracted folder
   containing `package.json`. Review and install the plugin. Enable developer
   preview if the account extension host requires it.
4. For each installed native client, check saved account loading, explicit login,
   refresh, switching and deletion. Verify the selected account in the native
   client. Only test operations declared by that plugin and supported by the
   native client on macOS. Record unavailable clients separately.
5. Check that entering a tool or changing tabs does not start login, refresh quota,
   switch accounts or show a blocking error. Cancel a login and leave a pending
   login to confirm cleanup. Keep existing accounts and credentials private.
6. Record the GitHub run, source commit, plugin version, Mac architecture and
   results. Repeat on Linux before claiming Linux runtime verification.

`candidate-index.json` is a review artifact, not a live installation source. Its
URLs describe the intended `packages/` location; those files are not committed by
the build. After native verification, commit the immutable ZIPs, pin the GitHub
download URLs and release commits to that package commit, merge the reviewed
records into `registry/`, and regenerate `index.json`. Run the repository checks
before pushing. The validated commit is then mirrored to GitCode and CNB.
