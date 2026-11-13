#!/usr/bin/env bash
# Keyless, read-only checks against the live fe2VI testnet deployment.
# Uses bash, curl, and python3. Does not call register or the nonce route.
# POST /v1/arenas/vault/queue is sent once, without a token, and must be HTTP 401.
# Missing recorderPassport/recorderMarket is WARN (those fields ship with the
# worker built from e42336a). Any FAIL makes the process exit 1.
# --local also runs `make verify` from the repo root.

set -u

API="${FE2VI_API:-https://api.fe2vi.space}"
SITE="${FE2VI_SITE:-https://fe2vi.space}"
RPC="${FE2VI_RPC:-https://rpc.testnet.fastnear.com}"
NEARBLOCKS="${FE2VI_NEARBLOCKS:-https://api-testnet.nearblocks.io}"

LOCAL=0
if [[ "${1:-}" == "--local" ]]; then
  LOCAL=1
elif [[ -n "${1:-}" ]]; then
  echo "usage: $0 [--local]" >&2
  exit 2
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "FAIL  tools  curl is not installed"
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "FAIL  tools  python3 is not installed"
  exit 1
fi

ROOT=""
if [[ -n "${BASH_SOURCE[0]:-}" && -f "${BASH_SOURCE[0]}" ]]; then
  ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

fetch() {
  # $1 name, $2 url, remaining args are curl args (method, headers, data)
  local name="$1" url="$2"
  shift 2
  local body="$WORKDIR/$name.body" meta="$WORKDIR/$name.meta"
  local code
  code="$(curl -sS --max-time 40 -o "$body" -w '%{http_code}' "$@" "$url" || echo "000")"
  printf '%s\n' "$code" > "$meta"
}

fetch health "$API/health"
fetch schedule "$API/v1/schedule"
fetch listings "$API/v1/market/listings"
fetch passport "$API/v1/passports/gen_007"
fetch juno "$API/v1/agents/juno"
fetch settlements "$API/v1/vault/settlements"
fetch match "$API/v1/matches/mt_2b2d2611ac9c0450"
fetch cup "$API/v1/cup/rounds/current"
fetch village "$API/v1/village/leaderboard"
fetch live "$API/v1/live/status"
fetch openapi "$API/openapi.json"
fetch llms "$API/llms.txt"
fetch skill "$SITE/skill/SKILL.md"
fetch queue "$API/v1/arenas/vault/queue" -X POST -H 'content-type: application/json' -d '{}'

rpc_token() {
  local id="$1" file="$2"
  local args
  args="$(printf '%s' "{\"token_id\":\"$id\"}" | python3 -c 'import sys,base64; print(base64.b64encode(sys.stdin.buffer.read()).decode())')"
  curl -sS --max-time 40 -o "$file" -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":\"v\",\"method\":\"query\",\"params\":{\"request_type\":\"call_function\",\"finality\":\"final\",\"account_id\":\"passport.fe2virecdb1e91.testnet\",\"method_name\":\"nft_token\",\"args_base64\":\"$args\"}}" \
    "$RPC" || true
}

rpc_token gen_001 "$WORKDIR/nft_gen_001.json"
rpc_token gen_007 "$WORKDIR/nft_gen_007.json"

fetch tx_win "$NEARBLOCKS/v2/txns/8VwaWeX7tFNH7ph8uKDhbvZtU3RfL2JePt99HTXagU61"
fetch tx_draw "$NEARBLOCKS/v2/txns/AyyDfVLPHoh8vzmxRFBAxzSRBD31Z2fHW4EpPRsmdMVy"

ONBOARDING=""
if [[ -n "$ROOT" && -f "$ROOT/templates/ONBOARDING.md" ]]; then
  ONBOARDING="$ROOT/templates/ONBOARDING.md"
fi

pass=0
warn=0
fail=0
while IFS=$'\t' read -r kind name detail; do
  [[ -z "${kind:-}" ]] && continue
  printf '%s  %s  %s\n' "$kind" "$name" "$detail"
  case "$kind" in
    PASS) pass=$((pass + 1)) ;;
    WARN) warn=$((warn + 1)) ;;
    FAIL) fail=$((fail + 1)) ;;
    *) fail=$((fail + 1)) ;;
  esac
done < <(python3 - "$WORKDIR" "$ONBOARDING" << 'PY'
import hashlib, json, pathlib, sys

work = pathlib.Path(sys.argv[1])
onboarding = sys.argv[2]

def status(name):
    return int((work / f"{name}.meta").read_text().strip() or "0")

def raw(name):
    return (work / f"{name}.body").read_bytes()

def load(name):
    return json.loads(raw(name))

def emit(kind, name, detail):
    print(f"{kind}\t{name}\t{detail}")

def strings(node, out):
    if isinstance(node, dict):
        for value in node.values():
            strings(value, out)
    elif isinstance(node, list):
        for value in node:
            strings(value, out)
    elif isinstance(node, str):
        out.append(node)

def health():
    code = status("health")
    if code != 200:
        emit("FAIL", "health", f"http {code}")
        emit("WARN", "health-recorder", "health body unread")
        return
    try:
        data = load("health")
    except Exception as err:
        emit("FAIL", "health", f"json {err}")
        emit("WARN", "health-recorder", "health body unread")
        return
    build = str(data.get("build", ""))
    problems = []
    if data.get("ok") is not True:
        problems.append(f"ok={data.get('ok')}")
    if data.get("network") != "testnet":
        problems.append(f"network={data.get('network')}")
    if data.get("arena") != "off-chain":
        problems.append(f"arena={data.get('arena')}")
    for key, want in {
        "passport": "passport.fe2vidisp7a3c.testnet",
        "market": "market.fe2vidisp7a3c.testnet",
        "escrow": "heist.fe2vidisp7a3c.testnet",
    }.items():
        if data.get(key) != want:
            problems.append(f"{key}={data.get(key)}")
    if len(build) != 40 or any(ch not in "0123456789abcdef" for ch in build):
        problems.append(f"build={build}")
    detail = f"build={build} network={data.get('network')} passport={data.get('passport')}"
    emit("FAIL" if problems else "PASS", "health", detail + ((" " + "; ".join(problems)) if problems else ""))
    rec_p = data.get("recorderPassport") or ""
    rec_m = data.get("recorderMarket") or ""
    want_p = "passport.fe2virecdb1e91.testnet"
    want_m = "market.fe2virecdb1e91.testnet"
    if rec_p == "" and rec_m == "":
        emit("WARN", "health-recorder", "recorderPassport and recorderMarket absent; the worker serving /health is older than e42336a")
    elif rec_p == want_p and rec_m == want_m:
        emit("PASS", "health-recorder", f"recorderPassport={rec_p} recorderMarket={rec_m}")
    else:
        emit("FAIL", "health-recorder", f"recorderPassport={rec_p!r} recorderMarket={rec_m!r}")

def schedule():
    code = status("schedule")
    if code != 200:
        emit("FAIL", "schedule", f"http {code}")
        return
    data = load("schedule")
    rooms = {row.get("room"): row.get("live") for row in data.get("rooms") or []}
    needed = ("vault", "cup", "village", "war")
    missing = [name for name in needed if name not in rooms]
    now = data.get("now")
    if missing or not isinstance(now, int):
        emit("FAIL", "schedule", f"now={now} missing={missing}")
        return
    flags = " ".join(f"{name}={'live' if rooms[name] else 'closed'}" for name in needed)
    emit("PASS", "schedule", f"now={now} {flags}")

def listings():
    code = status("listings")
    if code != 200:
        emit("FAIL", "listings", f"http {code}")
        return
    data = load("listings")
    rows = data.get("listings") or []
    ids = [row.get("tokenId") for row in rows]
    markets = sorted({row.get("marketContract") for row in rows})
    prices = sorted({row.get("priceYocto") for row in rows})
    want = [f"gen_{n:03d}" for n in range(1, 51)]
    problems = []
    if data.get("stats", {}).get("listedCount") != 50 or len(rows) != 50:
        problems.append(f"count={data.get('stats', {}).get('listedCount')}/{len(rows)}")
    if markets != ["market.fe2virecdb1e91.testnet"]:
        problems.append(f"markets={markets}")
    if prices != ["1000000000000000000000000"]:
        problems.append(f"prices={prices}")
    if sorted(ids) != want:
        problems.append("token ids are not gen_001..gen_050")
    if data.get("fee") != "0%":
        problems.append(f"fee={data.get('fee')}")
    detail = f"rows={len(rows)} fee={data.get('fee')} market={markets} priceNear={sorted({r.get('priceNear') for r in rows})} topContract={data.get('contract')}"
    emit("FAIL" if problems else "PASS", "listings", detail + ((" " + "; ".join(problems)) if problems else ""))

def nft(token_id):
    path = work / f"nft_{token_id}.json"
    if not path.exists() or path.stat().st_size == 0:
        emit("FAIL", f"nft-{token_id}", "empty rpc body")
        return
    raw_json = json.loads(path.read_text())
    if raw_json.get("error"):
        emit("FAIL", f"nft-{token_id}", str(raw_json["error"])[:180])
        return
    result = (raw_json.get("result") or {}).get("result")
    if not isinstance(result, list):
        emit("FAIL", f"nft-{token_id}", "no result bytes")
        return
    token = json.loads(bytes(result))
    media = ((token.get("metadata") or {}).get("media")) or ""
    extra_raw = (token.get("metadata") or {}).get("extra") or "{}"
    extra = json.loads(extra_raw) if isinstance(extra_raw, str) else extra_raw
    career = extra.get("career") or {}
    problems = []
    if token.get("token_id") != token_id:
        problems.append("token_id")
    if not str(media).startswith("data:image/webp;base64,"):
        problems.append("media")
    if not (token.get("metadata") or {}).get("media_hash"):
        problems.append("media_hash")
    if token_id == "gen_007":
        xp = career.get("xp")
        wins = career.get("wins")
        if not isinstance(xp, int) or xp < 4:
            problems.append(f"xp={xp}")
        if not isinstance(wins, int) or wins < 1:
            problems.append(f"wins={wins}")
    detail = (
        f"owner={token.get('owner_id')} xp={career.get('xp')} wins={career.get('wins')} "
        f"draws={career.get('draws')} matches={career.get('matches')} "
        f"media={str(media)[:27]} len={len(str(media))} hash={(token.get('metadata') or {}).get('media_hash')}"
    )
    emit("FAIL" if problems else "PASS", f"nft-{token_id}", detail + ((" " + ",".join(problems)) if problems else ""))

def passport():
    code = status("passport")
    if code != 200:
        emit("FAIL", "passport-gen_007", f"http {code}")
        return
    data = load("passport")
    career = data.get("onchainCareer") or {}
    row = data.get("passport") or {}
    xp = career.get("xp")
    problems = []
    if data.get("contract") != "passport.fe2virecdb1e91.testnet":
        problems.append(f"contract={data.get('contract')}")
    if row.get("tokenId") != "gen_007":
        problems.append(f"tokenId={row.get('tokenId')}")
    if not isinstance(xp, int) or xp < 4:
        problems.append(f"xp={xp}")
    detail = f"contract={data.get('contract')} agent={row.get('agentId')} xp={xp} wins={career.get('wins')} status={row.get('status')}"
    emit("FAIL" if problems else "PASS", "passport-gen_007", detail)

def juno():
    code = status("juno")
    if code != 200:
        emit("FAIL", "juno", f"http {code}")
        return
    data = load("juno")
    points = data.get("points")
    problems = []
    if data.get("handle") != "juno":
        problems.append(f"handle={data.get('handle')}")
    if data.get("status") != "active":
        problems.append(f"status={data.get('status')}")
    if not isinstance(points, int) or points < 4:
        problems.append(f"points={points}")
    detail = f"points={points} rank={data.get('rank')} status={data.get('status')} agent={data.get('agentId')}"
    emit("FAIL" if problems else "PASS", "juno", detail + ((" " + "; ".join(problems)) if problems else ""))

def settlements():
    code = status("settlements")
    if code != 200:
        emit("FAIL", "settlements", f"http {code}")
        return
    data = load("settlements")
    rows = {row.get("txHash"): row for row in data.get("matches") or []}
    win = "8VwaWeX7tFNH7ph8uKDhbvZtU3RfL2JePt99HTXagU61"
    draw = "AyyDfVLPHoh8vzmxRFBAxzSRBD31Z2fHW4EpPRsmdMVy"
    problems = []
    for tx, outcome in ((win, "win"), (draw, "draw")):
        row = rows.get(tx)
        if not row:
            problems.append(f"missing {outcome}")
            continue
        if row.get("finality") != "FINAL" or row.get("pays") is not True or row.get("outcome") != outcome:
            problems.append(f"{outcome} finality={row.get('finality')} pays={row.get('pays')} outcome={row.get('outcome')}")
    detail = f"source={data.get('source')} win={win[:8]} draw={draw[:8]}"
    emit("FAIL" if problems else "PASS", "settlements", detail + ((" " + "; ".join(problems)) if problems else ""))

def tx(name, outcome, amount, least):
    code = status(name)
    if code != 200:
        emit("FAIL", f"nearblocks-{outcome}", f"http {code}")
        return
    data = load(name)
    txn = (data.get("txns") or [{}])[0]
    found = []
    strings(data, found)
    text = "\n".join(found)
    problems = []
    if txn.get("signer_account_id") != "fe2vidisp7a3c.testnet":
        problems.append(f"signer={txn.get('signer_account_id')}")
    if txn.get("receiver_account_id") != "factory.fe2vidisp7a3c.testnet":
        problems.append(f"receiver={txn.get('receiver_account_id')}")
    if "settle_vault" not in text:
        problems.append("no settle_vault")
    if text.count(amount) < least:
        problems.append(f"{amount} count={text.count(amount)}")
    if f'"outcome":"{outcome}"' not in text and f'"outcome": "{outcome}"' not in text:
        problems.append(f"no outcome {outcome}")
    detail = f"signer={txn.get('signer_account_id')} receiver={txn.get('receiver_account_id')} amount_hits={text.count(amount)}"
    emit("FAIL" if problems else "PASS", f"nearblocks-{outcome}", detail + ((" " + "; ".join(problems)) if problems else ""))

def match():
    code = status("match")
    if code != 200:
        emit("FAIL", "match", f"http {code}")
        return
    data = load("match")
    vault = data.get("vault") or {}
    actions = data.get("actions") or []
    intents = [row.get("intent") for row in actions if isinstance(row.get("intent"), str) and row.get("intent")]
    problems = []
    if data.get("status") != "over":
        problems.append(f"status={data.get('status')}")
    if vault.get("revealed") is not True:
        problems.append(f"revealed={vault.get('revealed')}")
    if not intents:
        problems.append("no public intent")
    sample = (intents[0][:72] + "…") if intents and len(intents[0]) > 72 else (intents[0] if intents else "")
    detail = f"status={data.get('status')} revealed={vault.get('revealed')} intents={len(intents)} sample={sample!r}"
    emit("FAIL" if problems else "PASS", "match", detail + ((" " + "; ".join(problems)) if problems else ""))

def cup():
    code = status("cup")
    if code != 200:
        emit("FAIL", "cup", f"http {code}")
        return
    data = load("cup")
    if not data.get("roundId") or not isinstance(data.get("seatCount"), int):
        emit("FAIL", "cup", f"round={data.get('roundId')} seats={data.get('seatCount')}")
        return
    emit("PASS", "cup", f"round={data.get('roundId')} status={data.get('status')} seats={data.get('seatCount')} mode={data.get('mode')}")

def village():
    code = status("village")
    if code != 200:
        emit("FAIL", "village", f"http {code}")
        return
    data = load("village")
    rows = data.get("rows")
    if not isinstance(rows, list):
        emit("FAIL", "village", "rows is not a list")
        return
    emit("PASS", "village", f"source={data.get('source')} rows={len(rows)} window={data.get('window')}")

def live():
    code = status("live")
    if code != 200:
        emit("FAIL", "live", f"http {code}")
        return
    data = load("live")
    if not isinstance(data.get("links"), int) or not isinstance(data.get("verifiedLinks"), int):
        emit("FAIL", "live", "links are not integers")
        return
    emit("PASS", "live", f"links={data.get('links')} verified={data.get('verifiedLinks')} fills={data.get('fills')} rounds={data.get('rounds')} mode={data.get('mode')}")

def openapi():
    code = status("openapi")
    if code != 200:
        emit("FAIL", "openapi", f"http {code}")
        return
    data = load("openapi")
    paths = data.get("paths") or {}
    title = (data.get("info") or {}).get("title")
    if title != "fe2VI" or "/v1/schedule" not in paths:
        emit("FAIL", "openapi", f"title={title} paths={len(paths)}")
        return
    emit("PASS", "openapi", f"title={title} paths={len(paths)} bytes={len(raw('openapi'))}")

def hashes():
    if status("llms") != 200 or status("skill") != 200:
        emit("FAIL", "skill-hash", f"llms={status('llms')} skill={status('skill')}")
        return
    llms = raw("llms")
    skill = raw("skill")
    llms_sha = hashlib.sha256(llms).hexdigest()
    skill_sha = hashlib.sha256(skill).hexdigest()
    if llms_sha != skill_sha:
        emit("FAIL", "skill-hash", f"llms={llms_sha} skill={skill_sha}")
        return
    if onboarding:
        disk_sha = hashlib.sha256(pathlib.Path(onboarding).read_bytes()).hexdigest()
        if disk_sha != skill_sha:
            emit("FAIL", "skill-hash", f"live={skill_sha} ONBOARDING.md={disk_sha}")
            return
        emit("PASS", "skill-hash", f"sha256={skill_sha} llms.txt=SKILL.md=templates/ONBOARDING.md bytes={len(skill)}")
    else:
        emit("PASS", "skill-hash", f"sha256={skill_sha} llms.txt=SKILL.md bytes={len(skill)}")

def queue():
    code = status("queue")
    blob = raw("queue")
    try:
        data = json.loads(blob)
    except Exception:
        data = {}
    if code == 401 and data.get("error") == "unauthorized":
        emit("PASS", "queue-no-token", "http 401 error=unauthorized")
    else:
        emit("FAIL", "queue-no-token", f"http {code} body={blob[:120]!r}")

health()
schedule()
listings()
nft("gen_001")
nft("gen_007")
passport()
juno()
settlements()
tx("tx_win", "win", "20000000000000000000000", 1)
tx("tx_draw", "draw", "10000000000000000000000", 2)
match()
cup()
village()
live()
openapi()
hashes()
queue()
PY
)

if [[ "$LOCAL" -eq 1 ]]; then
  if [[ -z "$ROOT" || ! -f "$ROOT/Makefile" ]]; then
    echo "FAIL  make-verify  --local needs a checkout (curl | bash has no Makefile)"
    fail=$((fail + 1))
  else
    echo "== make verify =="
    if (cd "$ROOT" && make verify); then
      echo "PASS  make-verify  verify: ok"
      pass=$((pass + 1))
    else
      echo "FAIL  make-verify  make verify exited nonzero"
      fail=$((fail + 1))
    fi
  fi
fi

echo "summary  pass=$pass warn=$warn fail=$fail"
if [[ "$fail" -ne 0 ]]; then
  exit 1
fi
exit 0
