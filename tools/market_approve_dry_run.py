#!/usr/bin/env python3
"""Print the nft_approve call for a Paras-style listing. Does not sign or broadcast.

The message matches market.joespano.testnet and nftmarket.cookon.near:
{"market_type":"sale","ft_token_id":"near","price":"<yoctoNEAR>"}.

--rpc reads get_transaction_fee from the market. It does not send a transaction.
"""

import argparse
import json
import sys
import urllib.request

DEFAULT_MARKET = "market.joespano.testnet"
DEFAULT_NFT = "skin.fe2vi.testnet"
FEE_BPS = 200
CREATOR_BPS = 200
PLATFORM_BPS = 300


def fee_bps(market: str, rpc: str) -> int:
    body = {
        "jsonrpc": "2.0",
        "id": "dry-run",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "final",
            "account_id": market,
            "method_name": "get_transaction_fee",
            "args_base64": "e30=",
        },
    }
    request = urllib.request.Request(
        rpc,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        payload = json.loads(response.read().decode())
    if "error" in payload:
        raise SystemExit(f"view failed: {payload['error']}")
    raw = bytes(payload["result"]["result"]).decode()
    return int(json.loads(raw)["current_fee"])


def main() -> None:
    parser = argparse.ArgumentParser(description="Dry-run a skin listing approval. No broadcast.")
    parser.add_argument("--token-id", default="1")
    parser.add_argument("--owner", default="alice.fe2vi.testnet")
    parser.add_argument("--nft-contract", default=DEFAULT_NFT)
    parser.add_argument("--market", default=DEFAULT_MARKET)
    parser.add_argument("--price-yocto", default="1000000000000000000000000")
    parser.add_argument("--rpc", help="Read-only RPC. Omit to use the documented 200 bps fee.")
    args = parser.parse_args()
    if not args.price_yocto.isdigit() or int(args.price_yocto) == 0:
        raise SystemExit("price-yocto must be a positive integer string")
    market_bps = fee_bps(args.market, args.rpc) if args.rpc else FEE_BPS
    price = int(args.price_yocto)
    creator = price * CREATOR_BPS // 10_000
    platform = price * PLATFORM_BPS // 10_000
    market_fee = price * market_bps // 10_000
    seller = price - creator - platform - market_fee
    message = json.dumps(
        {"market_type": "sale", "ft_token_id": "near", "price": args.price_yocto},
        separators=(",", ":"),
    )
    call = {
        "signer_id": args.owner,
        "receiver_id": args.nft_contract,
        "actions": [
            {
                "FunctionCall": {
                    "method_name": "nft_approve",
                    "args": {
                        "token_id": args.token_id,
                        "account_id": args.market,
                        "msg": message,
                    },
                    "gas": "100000000000000",
                    "deposit": "10000000000000000000000",
                }
            }
        ],
    }
    print(json.dumps({"broadcast": False, "call": call, "msg": json.loads(message)}, indent=2))
    print(
        json.dumps(
            {
                "estimate_yocto": {
                    "buyer_pays": str(price),
                    "creator_2pct": str(creator),
                    "platform_3pct": str(platform),
                    "market_fee_bps": market_bps,
                    "market_fee": str(market_fee),
                    "seller_receives": str(seller),
                }
            },
            indent=2,
        ),
        file=sys.stdout,
    )


if __name__ == "__main__":
    main()
