#!/bin/sh
# Play consecutive Vault matches and settle each with the local restricted key.
# FE2VI_SETTLER_SECRET_KEY, FE2VI_SETTLER_KEY, FE2VI_LOOP_TOKEN_A, and
# FE2VI_LOOP_TOKEN_B are read by fe2vi and are not printed.
set -eu
exec fe2vi vault-loop "$@"
