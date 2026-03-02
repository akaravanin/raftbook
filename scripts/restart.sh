#!/usr/bin/env bash
# Restart the full RaftBook stack
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Restarting RaftBook stack..."
"$SCRIPT_DIR/stop.sh"
"$SCRIPT_DIR/start.sh"
