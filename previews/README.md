# Native account plugin preview indexes

These indexes reference CI-built ZIPs committed in `packages/`. They are for
native client verification and are not part of the public `index.json` catalog.
Each target has its own index, pinned to the package commit and SHA-256.

The packages require EchoBird's manifest version 2 architecture selection
support (6.1.8). Confirm native account, model and voice operations on the target
client before promoting reviewed releases to `registry/` and regenerating the
public index. Build or fixture checks alone do not establish native verification.
