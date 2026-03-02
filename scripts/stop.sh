#!/usr/bin/env bash
# Stop the full RaftBook stack
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PID_DIR="$ROOT/.pids"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BOLD='\033[1m'; NC='\033[0m'
info() { echo -e "${GREEN}▸${NC} $*"; }
warn() { echo -e "${YELLOW}▸${NC} $*"; }

# Kill a process by PID file, also kills its children
kill_proc() {
  local name="$1"
  local pid_file="$PID_DIR/$name.pid"

  if [ ! -f "$pid_file" ]; then
    warn "$name: no PID file found (already stopped?)"
    return
  fi

  local pid
  pid=$(cat "$pid_file")

  if ! kill -0 "$pid" 2>/dev/null; then
    warn "$name (PID $pid): not running."
    rm -f "$pid_file"
    return
  fi

  info "Stopping $name (PID $pid)..."

  # Kill child processes first (e.g. compiled binary spawned by cargo run, vite spawned by npm)
  pkill -TERM -P "$pid" 2>/dev/null || true
  kill -TERM "$pid" 2>/dev/null || true

  # Wait up to 5s for graceful exit, then force-kill
  for i in $(seq 1 5); do
    if ! kill -0 "$pid" 2>/dev/null; then
      break
    fi
    sleep 1
  done

  if kill -0 "$pid" 2>/dev/null; then
    warn "$name did not exit cleanly — sending SIGKILL."
    pkill -KILL -P "$pid" 2>/dev/null || true
    kill -KILL "$pid" 2>/dev/null || true
  fi

  rm -f "$pid_file"
  info "$name stopped."
}

echo -e "\n${BOLD}Stopping RaftBook stack...${NC}"

# Frontend may be a local process or a Docker container
FRONTEND_PID_FILE="$PID_DIR/frontend.pid"
if [ -f "$FRONTEND_PID_FILE" ]; then
  FRONTEND_PID_VAL=$(cat "$FRONTEND_PID_FILE")
  if [[ "$FRONTEND_PID_VAL" == docker-container:* ]]; then
    CONTAINER="${FRONTEND_PID_VAL#docker-container:}"
    info "Stopping frontend container ($CONTAINER)..."
    docker rm -f "$CONTAINER" &>/dev/null || true
    rm -f "$FRONTEND_PID_FILE"
  else
    kill_proc frontend
  fi
else
  warn "frontend: no PID file found (already stopped?)"
fi

# Backend may be a local process or a Docker container
BACKEND_PID_FILE="$PID_DIR/backend.pid"
if [ -f "$BACKEND_PID_FILE" ] && [ "$(cat "$BACKEND_PID_FILE")" = "docker" ]; then
  info "Stopping engine container..."
  cd "$ROOT" && docker compose stop engine
  rm -f "$BACKEND_PID_FILE"
else
  kill_proc backend
fi

info "Stopping docker infra..."
cd "$ROOT"
docker compose down

echo ""
echo -e "${GREEN}${BOLD}All services stopped.${NC}"
