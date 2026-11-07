#!/bin/sh
# Start the passport relay at boot if it is not already running.
# This file does not install itself. The operator's own init must invoke it.
# Example crontab line: @reboot /path/to/relay-boot.sh
# Do not run under sh -x or bash -x.
set -eu
dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
if pgrep -f "$dir/fe2vi passport relay" >/dev/null 2>&1; then
  exit 0
fi
nohup "$dir/relay-supervisor.sh" >/dev/null 2>&1 &
echo $! > "${FE2VI_RELAY_PIDFILE:-$dir/relay.pid}"
