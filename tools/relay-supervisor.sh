#!/bin/sh
# Persistent fe2VI passport relay. Reads secrets from mode 600 files and does not print them.
# Install beside the fe2vi binary. Do not run under `sh -x` or `bash -x`.
# Reboot autostart is tools/relay-boot.sh, and only if the operator's init calls it.
set -eu
cd "$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
binary="./fe2vi"
bearer_file="${FE2VI_RELAY_BEARER_FILE:-/etc/fe2vi/relay-bearer}"
key_file="${FE2VI_SIGNER_KEY_FILE:-/etc/fe2vi/registry-bind-key.json}"

if [ ! -x "$binary" ]; then
  echo "fe2vi binary is missing beside this script" >&2
  exit 1
fi
if [ ! -f "$bearer_file" ] || [ ! -f "$key_file" ]; then
  echo "bearer file or signer key file is missing" >&2
  exit 1
fi
bearer_mode=$(stat -c %a "$bearer_file")
key_mode=$(stat -c %a "$key_file")
if [ "$bearer_mode" != "600" ] || [ "$key_mode" != "600" ]; then
  echo "bearer file and signer key file must be mode 600" >&2
  exit 1
fi

export FE2VI_API_URL="${FE2VI_API_URL:-https://api.fe2vi.space}"
export FE2VI_RPC_URL="${FE2VI_RPC_URL:-https://rpc.testnet.fastnear.com}"
export FE2VI_NETWORK="${FE2VI_NETWORK:-testnet}"
export FE2VI_ACCOUNT_ID="${FE2VI_ACCOUNT_ID:-registry.fe2vidisp7a3c.testnet}"
export FE2VI_PASSPORT_ID="${FE2VI_PASSPORT_ID:-passport.fe2vidisp7a3c.testnet}"
export FE2VI_PASSPORT_MARKET_ID="${FE2VI_PASSPORT_MARKET_ID:-market.fe2vidisp7a3c.testnet}"
export FE2VI_REGISTRY_ID="${FE2VI_REGISTRY_ID:-registry.fe2vidisp7a3c.testnet}"
export FE2VI_HEIST_ID="${FE2VI_HEIST_ID:-heist.fe2vidisp7a3c.testnet}"

FE2VI_PASSPORT_RELAY_KEY=$(tr -d '\n' < "$bearer_file")
export FE2VI_PASSPORT_RELAY_KEY
FE2VI_SIGNER_SECRET_KEY=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["private_key"], end="")' "$key_file")
export FE2VI_SIGNER_SECRET_KEY

log="${FE2VI_RELAY_LOG:-$(pwd)/relay.log}"
touch "$log"
chmod 600 "$log"
exec >>"$log" 2>&1
printf '%s\n' "relay start api=${FE2VI_API_URL} account=${FE2VI_ACCOUNT_ID}"
exec "$binary" passport relay
