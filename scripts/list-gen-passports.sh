#!/bin/sh
# Mint gen_001..gen_050 unbound on the recorder passport and list each for 1 NEAR.
# The signer must be the registry and the token owner. Do not put the key in the repo
# or on the command line. Export FE2VI_SIGNER_SECRET_KEY in the shell first.
set -eu
if [ -z "${FE2VI_SIGNER_SECRET_KEY:-}" ]; then
  echo "Export FE2VI_SIGNER_SECRET_KEY in this shell. The script does not read or print it." >&2
  exit 1
fi
export FE2VI_ACCOUNT_ID="${FE2VI_ACCOUNT_ID:-fe2virecdb1e91.testnet}"
export FE2VI_PASSPORT_ID="${FE2VI_PASSPORT_ID:-passport.fe2virecdb1e91.testnet}"
export FE2VI_PASSPORT_MARKET_ID="${FE2VI_PASSPORT_MARKET_ID:-market.fe2virecdb1e91.testnet}"
export FE2VI_NETWORK=testnet
export FE2VI_RPC_URL="${FE2VI_RPC_URL:-https://rpc.testnet.fastnear.com}"
exec fe2vi passport seed --count 50 --price 1 --prefix gen --start 1 --owner "$FE2VI_ACCOUNT_ID"
