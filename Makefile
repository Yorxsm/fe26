# One local check: toolchain, tests, and read-only testnet pings.
# Run from the repo root: make verify

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.ONESHELL:
.PHONY: verify

ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))

verify:
	cd "$(ROOT)"
	echo "== toolchain =="
	rustc --version
	cargo --version
	node --version
	rustc --version | grep -q '1\.93\.0' || { echo "rustc must be 1.93.0 (rust-toolchain.toml)"; exit 1; }
	echo "== cargo test --workspace =="
	cargo test --workspace
	echo "== cargo test -p fe2vi-api --lib =="
	cargo test -p fe2vi-api --lib
	echo "== web node tests =="
	cd apps/web && node --test lib/*.test.mjs
	cd "$(ROOT)"
	echo "== web typecheck =="
	cd apps/web && npm run check:type
	cd "$(ROOT)"
	echo "== live pings =="
	python3 -c 'import base64; exec(base64.b64decode("aW1wb3J0IGpzb24sc3lzLHVybGxpYi5yZXF1ZXN0CmRlZiBmZXRjaCh1cmwsIGRhdGE9Tm9uZSwgaGVhZGVycz1Ob25lKToKICAgIHJlcT11cmxsaWIucmVxdWVzdC5SZXF1ZXN0KHVybCwgZGF0YT1kYXRhLCBoZWFkZXJzPWhlYWRlcnMgb3Ige30pCiAgICByZXNwPXVybGxpYi5yZXF1ZXN0LnVybG9wZW4ocmVxLCB0aW1lb3V0PTMwKQogICAgcmV0dXJuIHJlc3Auc3RhdHVzLCByZXNwLnJlYWQoKS5kZWNvZGUoKQpzdGF0dXMsIHJhdyA9IGZldGNoKCJodHRwczovL2FwaS5mZTJ2aS5zcGFjZS9oZWFsdGgiLCBoZWFkZXJzPXsidXNlci1hZ2VudCI6Ik1vemlsbGEvNS4wIiwiYWNjZXB0IjoiYXBwbGljYXRpb24vanNvbiJ9KQpib2R5PWpzb24ubG9hZHMocmF3KQppZiBzdGF0dXMhPTIwMCBvciBib2R5LmdldCgib2siKSBpcyBub3QgVHJ1ZSBvciBib2R5LmdldCgibmV0d29yayIpIT0idGVzdG5ldCI6CiAgICBzeXMuZXhpdCgiaGVhbHRoIGlzIG5vdCBhIHRlc3RuZXQgb2sgcmVzcG9uc2UiKQpwcmludCgiaGVhbHRoIiwgYm9keS5nZXQoIm5ldHdvcmsiKSwgYm9keS5nZXQoImJ1aWxkIikpCmVtcHR5PSIxMTExMTExMTExMTExMTExMTExMTExMTExMTExMTExMSIKcnBjPSJodHRwczovL3JwYy50ZXN0bmV0LmZhc3RuZWFyLmNvbSIKYWNjb3VudHM9WyJwYXNzcG9ydC5mZTJ2aWRpc3A3YTNjLnRlc3RuZXQiLCJtYXJrZXQuZmUydmlkaXNwN2EzYy50ZXN0bmV0IiwicmVnaXN0cnkuZmUydmlkaXNwN2EzYy50ZXN0bmV0IiwiZmFjdG9yeS5mZTJ2aWRpc3A3YTNjLnRlc3RuZXQiLCJoZWlzdC5mZTJ2aWRpc3A3YTNjLnRlc3RuZXQiXQpmb3IgYWNjdCBpbiBhY2NvdW50czoKICAgIHBheWxvYWQ9anNvbi5kdW1wcyh7Impzb25ycGMiOiIyLjAiLCJpZCI6InYiLCJtZXRob2QiOiJxdWVyeSIsInBhcmFtcyI6eyJyZXF1ZXN0X3R5cGUiOiJ2aWV3X2FjY291bnQiLCJmaW5hbGl0eSI6ImZpbmFsIiwiYWNjb3VudF9pZCI6YWNjdH19KS5lbmNvZGUoKQogICAgc3RhdHVzLCByYXcgPSBmZXRjaChycGMsIGRhdGE9cGF5bG9hZCwgaGVhZGVycz17ImNvbnRlbnQtdHlwZSI6ImFwcGxpY2F0aW9uL2pzb24ifSkKICAgIGRhdGE9anNvbi5sb2FkcyhyYXcpCiAgICBjb2RlPShkYXRhLmdldCgicmVzdWx0Iikgb3Ige30pLmdldCgiY29kZV9oYXNoIikgb3IgIiIKICAgIGlmIGRhdGEuZ2V0KCJlcnJvciIpIG9yIGNvZGU9PSIiIG9yIGNvZGU9PWVtcHR5OgogICAgICAgIHN5cy5leGl0KGFjY3QrIiBoYXMgbm8gY29udHJhY3QgY29kZSIpCiAgICBwcmludCgiY29udHJhY3QiLCBhY2N0KQo="))'
	echo "verify: ok"
