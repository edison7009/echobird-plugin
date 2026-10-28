Trae account extension
=====================

安装并启用后，原应用中的账号按钮、账号切换和已声明的账号能力恢复可用。无需手动启动插件。

This is a trusted native code extension. It uses the existing local account storage format and operates as the current user. Disabling or uninstalling removes capability availability and cancels active operations; saved account data is retained.

This preview archive supports Windows x64. The shared source retains platform-specific macOS and Linux branches; native builds and live client verification for those systems are pending. A platform declaration is not a live integration test.

Shared native source and reproducible build instructions: [account engine](../../native/account-engine/README.md).

Requires the unreleased EchoBird account extension host. Older clients cannot restore these capabilities by installing this package.
