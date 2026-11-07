#!/usr/bin/env python3
"""Queue a passport bind and run the registry relay once.

The signer must be the configured registry. The secret is read from
FE2VI_SIGNER_SECRET_KEY or --key-file and is never printed.

  python3 tools/accept_passport.py --api http://127.0.0.1:8787 \\
    --token-id disp_001 --owner fe2vidisp7a3c.testnet \\
    --key-file ~/.near-credentials/testnet/registry.fe2vidisp7a3c.testnet.json

A wrong owner is queued first. The relay must fail that job and must not
change the chain owner. The correct job then binds and stores the chain career.
"""

import argparse
import json
import os
import subprocess
import urllib.error
import urllib.request
import uuid


def call(api, method, path, body=None, token=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(api + path, data=data, method=method)
    req.add_header("content-type", "application/json")
    if token:
        req.add_header("authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=30) as res:
            raw = res.read().decode()
            return res.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as err:
        raw = err.read().decode()
        try:
            parsed = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            parsed = {"raw": raw}
        return err.code, parsed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--api", default="http://127.0.0.1:8787")
    parser.add_argument("--token-id", default="disp_001")
    parser.add_argument("--owner", required=True)
    parser.add_argument("--key-file", required=True)
    parser.add_argument("--signer", default="registry.fe2vidisp7a3c.testnet")
    parser.add_argument("--rpc", default="https://rpc.testnet.near.org")
    args = parser.parse_args()
    api = args.api.rstrip("/")
    key = json.loads(open(os.path.expanduser(args.key_file), encoding="utf-8").read())
    secret = key.get("private_key")
    if not secret:
        raise SystemExit("key file has no private_key")
    stamp = uuid.uuid4().hex[:8]
    handle = f"test-pass-{stamp}"
    status, body = call(
        api,
        "POST",
        "/v1/agents/register",
        {
            "handle": handle,
            "name": handle,
            "description": "disposable passport acceptance agent",
            "ownerContact": "test@fe2vi.invalid",
        },
    )
    if status == 429:
        raise SystemExit("register is rate limited. Use a fresh address or wait for the published hour window. No agent was changed.")
    if status != 201:
        raise SystemExit(f"register failed {status}")
    token = body["apiToken"]
    wrong, wrong_body = call(
        api,
        "POST",
        "/v1/agents/me/passport/bind",
        {"tokenId": args.token_id, "ownerNear": "not-the-owner.testnet", "agentNear": handle + ".testnet"},
        token,
    )
    if wrong != 202:
        raise SystemExit(f"wrong-owner bind queue failed {wrong} {wrong_body}")
    env = os.environ.copy()
    env.update(
        {
            "FE2VI_API_URL": api,
            "FE2VI_RPC_URL": args.rpc,
            "FE2VI_ACCOUNT_ID": args.signer,
            "FE2VI_SIGNER_SECRET_KEY": secret,
            "FE2VI_PASSPORT_RELAY_KEY": env.get("FE2VI_PASSPORT_RELAY_KEY", ""),
        }
    )
    if not env["FE2VI_PASSPORT_RELAY_KEY"]:
        raise SystemExit("Set FE2VI_PASSPORT_RELAY_KEY to the local worker secret. Do not commit it.")
    relay = subprocess.run(["fe2vi", "passport", "relay", "--once"], env=env, text=True, capture_output=True)
    if relay.returncode != 0:
        raise SystemExit("relay failed for the wrong owner without printing the signer")
    status, me = call(api, "GET", "/v1/agents/me/passport", token=token)
    if (me.get("passport") or {}).get("status") == "active":
        raise SystemExit("wrong owner was activated")
    status, body = call(
        api,
        "POST",
        "/v1/agents/me/passport/bind",
        {"tokenId": args.token_id, "ownerNear": args.owner, "agentNear": handle + ".testnet"},
        token,
    )
    if status not in (202, 409):
        raise SystemExit(f"bind queue failed {status}")
    relay = subprocess.run(["fe2vi", "passport", "relay", "--once"], env=env, text=True, capture_output=True)
    if relay.returncode != 0:
        raise SystemExit("relay failed for the owner")
    status, me = call(api, "GET", "/v1/agents/me/passport", token=token)
    passport = me.get("passport") or {}
    if passport.get("status") != "active":
        raise SystemExit(f"passport did not activate {me}")
    if passport.get("ownerNear") != args.owner:
        raise SystemExit("owner was not stored")
    if not isinstance(passport.get("career"), dict):
        raise SystemExit("career was not synced from the chain")
    print(json.dumps({"ok": True, "handle": handle, "status": passport.get("status"), "lastSyncSeq": passport.get("lastSyncSeq")}, indent=2))


if __name__ == "__main__":
    main()
