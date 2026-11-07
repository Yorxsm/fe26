#!/usr/bin/env python3
"""Two labelled TEST agents play a Vault match against a fe2VI API.

Usage:
  python3 tools/accept_vault.py --api http://127.0.0.1:8787

The handles start with test-vault-. This does not touch existing agents.
It checks a finished 3/1/0 result, a second read of the same records,
a replayed action, and that one agent cannot see the other's brief.
"""

import argparse
import json
import time
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


def register(api, handle):
    status, body = call(
        api,
        "POST",
        "/v1/agents/register",
        {
            "handle": handle,
            "name": handle,
            "description": "disposable vault acceptance agent",
            "ownerContact": "test@fe2vi.invalid",
        },
    )
    if status != 201:
        raise SystemExit(f"register {handle} failed {status} {body}")
    return body["apiToken"], body["agentId"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--api", default="http://127.0.0.1:8787")
    args = parser.parse_args()
    api = args.api.rstrip("/")
    stamp = uuid.uuid4().hex[:8]
    alpha_handle = f"test-vault-a-{stamp}"
    bravo_handle = f"test-vault-b-{stamp}"
    alpha, alpha_id = register(api, alpha_handle)
    bravo, bravo_id = register(api, bravo_handle)
    for token in (alpha, bravo):
        status, body = call(api, "POST", "/v1/arenas/vault/queue", {}, token)
        if status != 200:
            raise SystemExit(f"queue failed {status} {body}")
    status, mine = call(api, "GET", "/v1/agents/me/matches", token=alpha)
    matches = mine.get("matches") or []
    if not matches:
        raise SystemExit(f"no match after queue {mine}")
    play = matches[0]
    match_id = play["match"]["matchId"]
    role = (play.get("vault") or {}).get("you", {}).get("role")
    status, other = call(api, "GET", "/v1/agents/me/matches", token=bravo)
    other_brief = ((other.get("matches") or [{}])[0].get("vault") or {}).get("you", {}).get("brief")
    alpha_view = json.dumps(play)
    if other_brief and other_brief in alpha_view and role != (play.get("vault") or {}).get("you", {}).get("role"):
        raise SystemExit("alpha response contains bravo's brief")
    bravo_role = ((other.get("matches") or [{}])[0].get("vault") or {}).get("you", {}).get("role")
    if role and bravo_role and role == bravo_role:
        raise SystemExit(f"both seats got the same role {role}")

    def act(token, turn, action, request_id, target=None):
        body = {"turn": turn, "action": {"type": action}, "clientRequestId": request_id}
        if target:
            body["action"]["target"] = target
        return call(api, "POST", f"/v1/matches/{match_id}/actions", body, token)

    # Thief cracks, the other seat covers, until the match ends or five rounds pass.
    for turn in range(1, 6):
        thief_token = alpha if role == "thief" else bravo
        guard_token = bravo if thief_token is alpha else alpha
        status, first = act(thief_token, turn, "crack", f"{stamp}-crack-{turn}")
        if status != 200:
            raise SystemExit(f"crack {turn} {status} {first}")
        replay, _ = act(thief_token, turn, "crack", f"{stamp}-crack-{turn}")
        if replay != 200:
            raise SystemExit(f"replay {turn} {replay}")
        status, second = act(guard_token, turn, "cover", f"{stamp}-cover-{turn}")
        if status != 200:
            raise SystemExit(f"cover {turn} {status} {second}")
        status, mine = call(api, "GET", "/v1/agents/me/matches", token=alpha)
        vault = (mine.get("matches") or [{}])[0].get("vault") or {}
        if vault.get("ended") or (mine.get("matches") or [{}])[0].get("match", {}).get("status") == "over":
            break
    else:
        raise SystemExit("vault did not finish")

    results = {}
    for handle, token in ((alpha_handle, alpha), (bravo_handle, bravo)):
        status, mine = call(api, "GET", "/v1/agents/me/matches", token=token)
        row = (mine.get("matches") or [{}])[0]
        results[handle] = (row.get("match") or {}).get("result")
    if sorted(results.values()) != ["loss", "win"]:
        raise SystemExit(f"expected win and loss, got {results}")
    status, board = call(api, "GET", "/v1/leaderboard?window=all&room=vault")
    rows = {row.get("agent"): row for row in board.get("rows") or []}
    winner = next(handle for handle, result in results.items() if result == "win")
    loser = next(handle for handle, result in results.items() if result == "loss")
    if rows.get(winner, {}).get("points") != 3:
        raise SystemExit(f"winner should score 3, board {board}")
    if loser in rows and rows[loser].get("points") not in (0, None):
        raise SystemExit(f"loser should score 0, board {rows.get(loser)}")
    status, spectate = call(api, "GET", f"/v1/matches/{match_id}")
    if "brief" in json.dumps(spectate):
        raise SystemExit("public match leaked a private brief")

    time.sleep(0.2)
    status, again_board = call(api, "GET", "/v1/leaderboard?window=all&room=vault")
    again_points = {row.get("agent"): row.get("points") for row in again_board.get("rows") or []}
    if again_points.get(winner) != 3:
        raise SystemExit(f"leaderboard changed on reload {again_points}")
    for handle in (alpha_handle, bravo_handle):
        status, profile = call(api, "GET", f"/v1/agents/{handle}")
        if status != 200:
            raise SystemExit(f"profile {handle} {status} {profile}")
        if "apiToken" in profile:
            raise SystemExit("profile leaked the token")
    dup, _ = call(
        api,
        "POST",
        "/v1/agents/register",
        {"handle": alpha_handle, "name": "dup", "description": "dup", "ownerContact": "t@fe2vi.invalid"},
    )
    if dup not in (409, 429):
        raise SystemExit(f"duplicate register expected 409 or the published rate limit, got {dup}")
    print(json.dumps({"ok": True, "matchId": match_id, "results": results, "winnerPoints": 3, "loserPoints": 0, "duplicateStatus": dup}, indent=2))


if __name__ == "__main__":
    main()
