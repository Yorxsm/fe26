#!/bin/sh
# Upgrade passport.fe2virecdb1e91.testnet without resetting state.
# The signer key is read from a file path. This script never prints the key.
# It refuses *.fe2vi.testnet and the disposable passport.
#
# Required:
#   FE2VI_SIGNER_KEY_FILE   credentials json, or a file whose first line is ed25519:...
#   FE2VI_ACCOUNT_ID        account that is a recorder on the passport
#   FE2VI_PASSPORT_ID       passport.fe2virecdb1e91.testnet
#   FE2VI_MEDIA_TOKENS      gen_001..gen_050, or the tokens to upload in set mode
# Deploy signer, when it is not the recorder (the contract account deploys its own code):
#   FE2VI_DEPLOY_ACCOUNT    defaults to FE2VI_PASSPORT_ID
#   FE2VI_DEPLOY_KEY_FILE   defaults to FE2VI_SIGNER_KEY_FILE when the accounts match
# Optional:
#   FE2VI_MEDIA_MODE        rehash (default) or set
#                           rehash deploys code and calls rehash_token_media. No image bytes
#                           are uploaded and no storage is written.
#                           set uploads assets/passport-stills/<token>.webp
#   FE2VI_SET_TOKENS        after rehash, also upload these stills (for example rec_001)
#   FE2VI_MEDIA_DEPOSIT     attached NEAR for each set_token_media. When unset, each call
#                           attaches that still's storage cost plus 0.001 NEAR. Excess is refunded.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
KEY_FILE=${FE2VI_SIGNER_KEY_FILE:?Set FE2VI_SIGNER_KEY_FILE to the signer key path}
ACCOUNT=${FE2VI_ACCOUNT_ID:?Set FE2VI_ACCOUNT_ID}
PASSPORT=${FE2VI_PASSPORT_ID:?Set FE2VI_PASSPORT_ID}
TOKENS=${FE2VI_MEDIA_TOKENS:?Set FE2VI_MEDIA_TOKENS}
MODE=${FE2VI_MEDIA_MODE:-rehash}
case "$MODE" in
  rehash|set) ;;
  *) echo "FE2VI_MEDIA_MODE must be rehash or set" >&2; exit 1 ;;
esac
DEPLOY_ACCOUNT=${FE2VI_DEPLOY_ACCOUNT:-$PASSPORT}
if [ "$DEPLOY_ACCOUNT" = "$ACCOUNT" ]; then
  DEPLOY_KEY_FILE=${FE2VI_DEPLOY_KEY_FILE:-$KEY_FILE}
else
  DEPLOY_KEY_FILE=${FE2VI_DEPLOY_KEY_FILE:?Set FE2VI_DEPLOY_KEY_FILE for the contract account}
fi
if [ -n "${FE2VI_MEDIA_DEPOSIT:-}" ]; then
  case "$FE2VI_MEDIA_DEPOSIT" in
    *[!0-9.]*) echo "FE2VI_MEDIA_DEPOSIT must be a NEAR amount" >&2; exit 1 ;;
  esac
fi
command -v near >/dev/null 2>&1 || {
  echo "near-cli-rs is required. The key stays in the file you named." >&2
  exit 1
}
command -v python3 >/dev/null 2>&1 || {
  echo "python3 is required." >&2
  exit 1
}
[ -f "$KEY_FILE" ] || { echo "Signer key file is missing." >&2; exit 1; }
[ -f "$DEPLOY_KEY_FILE" ] || { echo "Deploy key file is missing." >&2; exit 1; }

home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
umask 077
mkdir -p "$home/.near-credentials/testnet"
export HOME_DIR="$home" ROOT ACCOUNT PASSPORT DEPLOY_ACCOUNT KEY_FILE DEPLOY_KEY_FILE TOKENS MODE
python3 - << 'PY'
import os, re, sys
accounts = [os.environ["ACCOUNT"], os.environ["PASSPORT"], os.environ["DEPLOY_ACCOUNT"]]
spec = os.environ["TOKENS"]
ids = []
for part in spec.split(","):
    part = part.strip()
    if not part:
        continue
    if ".." in part:
        left, right = part.split("..", 1)
        a = re.fullmatch(r"([A-Za-z][A-Za-z0-9_-]*?)(\d+)", left)
        b = re.fullmatch(r"([A-Za-z][A-Za-z0-9_-]*?)(\d+)", right)
        if not a or not b or a.group(1) != b.group(1) or len(a.group(2)) != len(b.group(2)):
            sys.exit("bad token range")
        start, end = int(a.group(2)), int(b.group(2))
        if end < start or end - start > 200:
            sys.exit("bad token range")
        width = len(a.group(2))
        for n in range(start, end + 1):
            ids.append(f"{a.group(1)}{n:0{width}d}")
    else:
        ids.append(part)
if not ids:
    sys.exit("no tokens")
original = re.compile(r"(^|\.)fe2vi\.testnet$")
for account in accounts:
    if original.search(account) or "fe2vidisp" in account:
        sys.exit("refusing an original or disposable passport account")
for token in ids:
    if not re.fullmatch(r"[A-Za-z0-9_-]{3,64}", token):
        sys.exit("bad token id")
open(os.path.join(os.environ["HOME_DIR"], "tokens"), "w").write("\n".join(ids) + "\n")
print(f"{len(ids)} tokens on {os.environ['PASSPORT']}")
PY
python3 - << 'PY'
import json, os, sys
home = os.environ["HOME_DIR"]

def load(path):
    raw = open(path, "r").read().strip()
    if not raw:
        sys.exit("key file is empty")
    if raw.startswith("{"):
        data = json.loads(raw)
        key = data.get("private_key") or data.get("secret_key")
        account = data.get("account_id")
    else:
        key = raw.splitlines()[0].strip()
        account = None
    if not isinstance(key, str) or not key.startswith("ed25519:"):
        sys.exit("key file has no ed25519 key")
    return account, key

def write(account, key):
    path = os.path.join(home, ".near-credentials", "testnet", account + ".json")
    with open(path, "w") as handle:
        json.dump({"account_id": account, "private_key": key}, handle)
    os.chmod(path, 0o600)

signer_account, signer_key = load(os.environ["KEY_FILE"])
if signer_account and signer_account != os.environ["ACCOUNT"]:
    sys.exit("signer key file account does not match FE2VI_ACCOUNT_ID")
write(os.environ["ACCOUNT"], signer_key)
if os.environ["DEPLOY_ACCOUNT"] != os.environ["ACCOUNT"]:
    deploy_account, deploy_key = load(os.environ["DEPLOY_KEY_FILE"])
    if deploy_account and deploy_account != os.environ["DEPLOY_ACCOUNT"]:
        sys.exit("deploy key file account does not match FE2VI_DEPLOY_ACCOUNT")
    write(os.environ["DEPLOY_ACCOUNT"], deploy_key)
PY
export HOME="$home"

echo "Building passport wasm."
cargo build -p passport --lib --target wasm32-unknown-unknown --release --manifest-path "$ROOT/Cargo.toml"
WASM="$ROOT/target/wasm32-unknown-unknown/release/passport.wasm"
[ -f "$WASM" ] || { echo "passport wasm was not produced" >&2; exit 1; }

echo "Deploying code to $PASSPORT without init. State stays."
near contract deploy "$PASSPORT" use-file "$WASM" without-init-call network-config testnet sign-as "$DEPLOY_ACCOUNT" sign-with-keychain send

set_one() {
  token=$1
  still="$ROOT/assets/passport-stills/$token.webp"
  [ -f "$still" ] || { echo "Missing still $still" >&2; exit 1; }
  arg="$home/$token.json"
  TOKEN_ID="$token" STILL="$still" ARG="$arg" python3 - << 'PY'
import base64, hashlib, json, os
raw = open(os.environ["STILL"], "rb").read()
if not (8 * 1024 <= len(raw) <= 20 * 1024):
    raise SystemExit(f"still {os.environ['TOKEN_ID']} is {len(raw)} bytes, expected 8-20 KB")
media = "data:image/webp;base64," + base64.b64encode(raw).decode("ascii")
digest = base64.b64encode(hashlib.sha256(raw).digest()).decode("ascii")
with open(os.environ["ARG"], "w") as handle:
    json.dump(
        {"token_id": os.environ["TOKEN_ID"], "media": media, "media_hash": digest},
        handle,
        separators=(",", ":"),
    )
open(os.environ["ARG"] + ".near", "w").write(media)
PY
  if [ -n "${FE2VI_MEDIA_DEPOSIT:-}" ]; then
    deposit="$FE2VI_MEDIA_DEPOSIT"
  else
    deposit=$(python3 - "$arg.near" "$token" << 'PY'
import sys
media = open(sys.argv[1], "r").read()
token = sys.argv[2]
record = 1 + 4 + len(token) + 4 + len(media) + 40
print(f"{record / 100000 + 0.001:.5f}")
PY
)
  fi
  rm -f "$arg.near"
  echo "Setting media on $token with $deposit NEAR attached"
  near contract call-function as-transaction "$PASSPORT" set_token_media \
    json-args "$(cat "$arg")" prepaid-gas '120 Tgas' attached-deposit "$deposit NEAR" \
    sign-as "$ACCOUNT" network-config testnet sign-with-keychain send
}

if [ "$MODE" = "rehash" ]; then
  python3 - "$home/tokens" "$home/rehash.json" << 'PY'
import json, sys
ids = [line.strip() for line in open(sys.argv[1]) if line.strip()]
with open(sys.argv[2], "w") as handle:
    json.dump({"token_ids": ids}, handle, separators=(",", ":"))
print(f"rehash {len(ids)} tokens, no storage write")
PY
  near contract call-function as-transaction "$PASSPORT" rehash_token_media \
    json-args "$(cat "$home/rehash.json")" prepaid-gas '250 Tgas' attached-deposit '0 NEAR' \
    sign-as "$ACCOUNT" network-config testnet sign-with-keychain send
  if [ -n "${FE2VI_SET_TOKENS:-}" ]; then
    old=$IFS
    IFS=,
    for token in $FE2VI_SET_TOKENS; do
      token=$(printf '%s' "$token" | tr -d ' ')
      [ -n "$token" ] || continue
      set_one "$token"
    done
    IFS=$old
  fi
else
  while IFS= read -r token; do
    [ -n "$token" ] || continue
    set_one "$token"
  done < "$home/tokens"
fi
echo "Done on $PASSPORT."
