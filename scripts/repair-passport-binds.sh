#!/bin/sh
# Bind recorder-passport tokens to the agents that already registered them.
# Idempotent: a token already bound to that agent is left alone.
# The signer must be the contract registry. Keys are read from a file and never printed.
# Refuses *.fe2vi.testnet and passport.fe2vidisp7a3c.testnet.
#
# Required:
#   FE2VI_SIGNER_KEY_FILE   credentials json, or a file whose first line is ed25519:...
#   FE2VI_ACCOUNT_ID        registry account, fe2virecdb1e91.testnet for the recorder passport
#   FE2VI_PASSPORT_ID       passport.fe2virecdb1e91.testnet
#   FE2VI_BINDS             gen_007:agt_ec94c30ee4928ade,gen_008:agt_7855cb2f0d5fd82f
set -eu
KEY_FILE=${FE2VI_SIGNER_KEY_FILE:?Set FE2VI_SIGNER_KEY_FILE to the signer key path}
ACCOUNT=${FE2VI_ACCOUNT_ID:?Set FE2VI_ACCOUNT_ID}
PASSPORT=${FE2VI_PASSPORT_ID:?Set FE2VI_PASSPORT_ID}
BINDS=${FE2VI_BINDS:?Set FE2VI_BINDS to token:agent pairs}
case "$PASSPORT" in
  *.fe2vi.testnet|passport.fe2vidisp7a3c.testnet)
    echo "refusing $PASSPORT" >&2
    exit 1
    ;;
esac
command -v near >/dev/null 2>&1 || { echo "near-cli-rs is required." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "python3 is required." >&2; exit 1; }
[ -f "$KEY_FILE" ] || { echo "Signer key file is missing." >&2; exit 1; }
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT
umask 077
mkdir -p "$home/.near-credentials/testnet"
export HOME_DIR="$home" ACCOUNT KEY_FILE
python3 - << 'PY'
import json, os, sys
raw = open(os.environ["KEY_FILE"]).read().strip()
if raw.startswith("{"):
    data = json.loads(raw)
    key = data.get("private_key") or data.get("secret_key")
    named = data.get("account_id")
else:
    key = raw.splitlines()[0].strip()
    named = None
if not isinstance(key, str) or not key.startswith("ed25519:"):
    sys.exit("key file has no ed25519 key")
if named and named != os.environ["ACCOUNT"]:
    sys.exit("signer key file account does not match FE2VI_ACCOUNT_ID")
path = os.path.join(os.environ["HOME_DIR"], ".near-credentials", "testnet", os.environ["ACCOUNT"] + ".json")
with open(path, "w") as handle:
    json.dump({"account_id": os.environ["ACCOUNT"], "private_key": key}, handle)
os.chmod(path, 0o600)
PY
export HOME="$home"
rpc() {
  near contract call-function as-read-only "$PASSPORT" "$1" json-args "$2" network-config testnet now
}
old=$IFS
IFS=,
for pair in $BINDS; do
  token=${pair%%:*}
  agent=${pair#*:}
  token=$(printf '%s' "$token" | tr -d ' ')
  agent=$(printf '%s' "$agent" | tr -d ' ')
  [ -n "$token" ] && [ -n "$agent" ] || { echo "bad pair" >&2; exit 1; }
  case "$token" in
    *[!A-Za-z0-9_-]*) echo "bad token" >&2; exit 1 ;;
  esac
  case "$agent" in
    *[!A-Za-z0-9_-]*) echo "bad agent" >&2; exit 1 ;;
  esac
  current=$(rpc agent_token "{\"agent_id\":\"$agent\"}" | python3 -c '
import json, sys
text = sys.stdin.read()
for line in reversed([row.strip() for row in text.splitlines() if row.strip()]):
    try:
        value = json.loads(line)
    except json.JSONDecodeError:
        continue
    if value is None:
        print("")
        raise SystemExit(0)
    if isinstance(value, str):
        print(value)
        raise SystemExit(0)
sys.exit("could not read agent_token")
')
  if [ "$current" = "$token" ]; then
    echo "$token already bound to $agent"
    continue
  fi
  if [ -n "$current" ] && [ "$current" != "null" ]; then
    echo "$agent is already bound to a different token; left unchanged" >&2
    exit 1
  fi
  echo "Binding $token to $agent"
  near contract call-function as-transaction "$PASSPORT" bind \
    json-args "{\"token_id\":\"$token\",\"agent_id\":\"$agent\"}" \
    prepaid-gas '40 Tgas' attached-deposit '0 NEAR' \
    sign-as "$ACCOUNT" network-config testnet sign-with-keychain send
done
IFS=$old
echo "Bind repair finished on $PASSPORT."
