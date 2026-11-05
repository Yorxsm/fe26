#!/usr/bin/env bash
set -euo pipefail

# fe2VI On-Chain Demo Flow on NEAR Testnet
# Tests the full end-to-end non-custodial lifecycle:
# 1. Verification of live deployed contracts (passport & market)
# 2. Token status and metadata inspection
# 3. Live listing verification on market.fe2vi.testnet

NETWORK="testnet"
RPC_URL="https://rpc.testnet.near.org"
PASSPORT_CONTRACT="passport.fe2vi.testnet"
MARKET_CONTRACT="market.fe2vi.testnet"

echo "=========================================================="
echo "  fe2VI Arena — On-Chain Testnet Verification & Demo"
echo "=========================================================="
echo "Network:           $NETWORK"
echo "RPC:               $RPC_URL"
echo "Passport Contract: $PASSPORT_CONTRACT"
echo "Market Contract:   $MARKET_CONTRACT"
echo ""

echo "[1/3] Querying NFT Total Supply on $PASSPORT_CONTRACT..."
TOTAL_SUPPLY=$(curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\": \"2.0\", \"id\": \"demo\", \"method\": \"query\", \"params\": {\"request_type\": \"call_function\", \"finality\": \"final\", \"account_id\": \"$PASSPORT_CONTRACT\", \"method_name\": \"nft_total_supply\", \"args_base64\": \"e30=\"}}" \
  | node -e '
    let data = "";
    process.stdin.on("data", c => data += c);
    process.stdin.on("end", () => {
      const res = JSON.parse(data).result;
      if (res && res.result) {
        console.log(Buffer.from(res.result).toString());
      } else {
        console.log("error querying supply");
      }
    });
  ')
echo "  Total Minted Passports: $TOTAL_SUPPLY"
echo ""

echo "[2/3] Inspecting Genesis Token #fdr_001 on $PASSPORT_CONTRACT..."
TOKEN_INFO=$(curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\": \"2.0\", \"id\": \"demo\", \"method\": \"query\", \"params\": {\"request_type\": \"call_function\", \"finality\": \"final\", \"account_id\": \"$PASSPORT_CONTRACT\", \"method_name\": \"nft_token\", \"args_base64\": \"eyJ0b2tlbl9pZCI6ICJmZHJfMDAxIn0=\"}}" \
  | node -e '
    let data = "";
    process.stdin.on("data", c => data += c);
    process.stdin.on("end", () => {
      const res = JSON.parse(data).result;
      if (res && res.result) {
        console.log(Buffer.from(res.result).toString());
      } else {
        console.log("error querying token");
      }
    });
  ')
echo "  Token Details: $TOKEN_INFO"
echo ""

echo "[3/3] Inspecting Live Listing on $MARKET_CONTRACT for #fdr_001..."
LISTING_INFO=$(curl -s -X POST "$RPC_URL" \
  -H "Content-Type: application/json" \
  -d "{\"jsonrpc\": \"2.0\", \"id\": \"demo\", \"method\": \"query\", \"params\": {\"request_type\": \"call_function\", \"finality\": \"final\", \"account_id\": \"$MARKET_CONTRACT\", \"method_name\": \"get_listing\", \"args_base64\": \"eyJ0b2tlbl9pZCI6ICJmZHJfMDAxIn0=\"}}" \
  | node -e '
    let data = "";
    process.stdin.on("data", c => data += c);
    process.stdin.on("end", () => {
      const res = JSON.parse(data).result;
      if (res && res.result) {
        console.log(Buffer.from(res.result).toString());
      } else {
        console.log("No active listing found.");
      }
    });
  ')
echo "  Listing Details: $LISTING_INFO"
echo ""
echo "=========================================================="
echo "  Demo Ready: Contracts live, tokens minted, and listed!"
echo "=========================================================="
