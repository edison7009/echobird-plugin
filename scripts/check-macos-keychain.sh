#!/usr/bin/env bash
set -euo pipefail

fixture_keychain="$RUNNER_TEMP/echobird-plugin-test.keychain-db"
original_keychains=()
while IFS= read -r item; do
  item="${item#*\"}"
  original_keychains+=("${item%\"*}")
done < <(security list-keychains -d user)
security create-keychain -p 'fixture-keychain-password' "$fixture_keychain"
trap 'security list-keychains -d user -s "${original_keychains[@]}"; security delete-keychain "$fixture_keychain" >/dev/null 2>&1' EXIT
security unlock-keychain -p 'fixture-keychain-password' "$fixture_keychain"
security list-keychains -d user -s "$fixture_keychain" "${original_keychains[@]}"
for name in 'EchoBird Native Storage Test' 'EchoBird Native Storage Alternate Test'; do
  account="$name"
  if [[ "$name" == *Alternate* ]]; then account="$name Key"; fi
  security add-generic-password -s "$name Safe Storage" -a "$account" -w 'test-safe-storage-password' -T /usr/bin/security "$fixture_keychain"
done
fixture_test='services::electron_storage::tests::reads_native_os_credential_fixture'
cargo +1.93 test --locked --manifest-path native/account-engine/Cargo.toml -- --list --ignored | grep -Fx "$fixture_test: test"
ECHOBIRD_NATIVE_CREDENTIAL_TEST=1 cargo +1.93 test --locked --manifest-path native/account-engine/Cargo.toml "$fixture_test" -- --ignored --exact
