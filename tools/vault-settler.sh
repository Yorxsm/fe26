#!/bin/sh
# One Vault settlement. Does not install itself and does not print secrets.
# The parent generates the limited key locally. It must be a function-call key on
# fe2vidisp7a3c.testnet for factory.fe2vidisp7a3c.testnet method settle_vault only.
# Do not reuse the passport bind key. Do not run under sh -x or bash -x.
set -eu
cd "$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
binary="./fe2vi"
key_file="${FE2VI_SETTLER_KEY_FILE:-/etc/fe2vi/settler-key.json}"
bearer_file="${FE2VI_SETTLER_BEARER_FILE:-/etc/fe2vi/settler-bearer}"
match_id="${1:-}"
if [ -z "$match_id" ]; then
  echo "usage: vault-settler.sh <match-id>" >&2
  exit 1
fi
if [ ! -x "$binary" ] || [ ! -f "$key_file" ] || [ ! -f "$bearer_file" ]; then
  echo "binary, settler key file, or bearer file is missing" >&2
  exit 1
fi
key_mode=$(stat -c %a "$key_file")
bearer_mode=$(stat -c %a "$bearer_file")
if [ "$key_mode" != "600" ] || [ "$bearer_mode" != "600" ]; then
  echo "settler key file and bearer file must be mode 600" >&2
  exit 1
fi
export FE2VI_API_URL="${FE2VI_API_URL:-https://api.fe2vi.space}"
export FE2VI_RPC_URL="${FE2VI_RPC_URL:-https://rpc.testnet.fastnear.com}"
export FE2VI_ACCOUNT_ID="${FE2VI_ACCOUNT_ID:-fe2vidisp7a3c.testnet}"
export FE2VI_FACTORY_ID="${FE2VI_FACTORY_ID:-factory.fe2vidisp7a3c.testnet}"
FE2VI_SETTLER_SECRET_KEY=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["private_key"], end="")' "$key_file")
export FE2VI_SETTLER_SECRET_KEY
FE2VI_SETTLER_KEY=$(tr -d '\n' < "$bearer_file")
export FE2VI_SETTLER_KEY
exec "$binary" settle-vault --match-id "$match_id"
