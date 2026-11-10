#!/bin/sh
# Deploy market.fe2virecdb1e91.testnet from the existing passport-market wasm.
# The signer is the recorder account, which owns that subaccount. Export
# FE2VI_SIGNER_SECRET_KEY in the shell first. This script does not print it.
set -eu
if [ -z "${FE2VI_SIGNER_SECRET_KEY:-}" ]; then
  echo "Export FE2VI_SIGNER_SECRET_KEY in this shell. The script does not read or print it." >&2
  exit 1
fi
command -v near >/dev/null 2>&1 || {
  echo "near-cli-rs is required (the near binary). The key stays in the environment." >&2
  exit 1
}
ACCOUNT="${FE2VI_ACCOUNT_ID:-fe2virecdb1e91.testnet}"
MARKET="${FE2VI_RECORDER_MARKET_ID:-market.fe2virecdb1e91.testnet}"
PASSPORT="${FE2VI_PASSPORT_ID:-passport.fe2virecdb1e91.testnet}"
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
WASM="$ROOT/target/wasm32-unknown-unknown/release/passport_market.wasm"
if [ ! -f "$WASM" ]; then
  cargo build -p passport-market --target wasm32-unknown-unknown --release --manifest-path "$ROOT/Cargo.toml"
fi
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
umask 077
mkdir -p "$home/.near-credentials/testnet"
HOME_DIR="$home" ACCOUNT_ID="$ACCOUNT" python3 - << 'PY'
import json, os
home = os.environ["HOME_DIR"]
account = os.environ["ACCOUNT_ID"]
path = os.path.join(home, ".near-credentials", "testnet", account + ".json")
with open(path, "w") as handle:
    json.dump(
        {"account_id": account, "private_key": os.environ["FE2VI_SIGNER_SECRET_KEY"]},
        handle,
    )
os.chmod(path, 0o600)
PY
export HOME="$home"
echo "Creating $MARKET with 4 NEAR, then deploying the existing market wasm."
near account create-account fund-myself "$MARKET" '4 NEAR' autogenerate-new-keypair save-to-legacy-keychain sign-as "$ACCOUNT" network-config testnet sign-with-keychain send
near contract deploy "$MARKET" use-file "$WASM" without-init-call network-config testnet sign-with-keychain send
near contract call-function as-transaction "$MARKET" new json-args "{\"passport\":\"$PASSPORT\"}" prepaid-gas '30 Tgas' attached-deposit '0 NEAR' sign-as "$MARKET" network-config testnet sign-with-keychain send
echo "Market $MARKET initialized for $PASSPORT."
