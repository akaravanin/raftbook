#!/usr/bin/env bash
# Start the full RaftBook stack: infra → backend → frontend
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PID_DIR="$ROOT/.pids"
LOG_DIR="$ROOT/logs"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BOLD='\033[1m'; NC='\033[0m'
info()  { echo -e "${GREEN}▸${NC} $*"; }
warn()  { echo -e "${YELLOW}▸${NC} $*"; }
die()   { echo -e "${RED}✗${NC} $*" >&2; exit 1; }
title() { echo -e "\n${BOLD}$*${NC}"; }

mkdir -p "$PID_DIR" "$LOG_DIR"

# ── Source rustup env (not always in PATH in non-interactive shells) ───────────
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

# ── Pre-flight checks ──────────────────────────────────────────────────────────
command -v docker &>/dev/null || die "'docker' not found. Install: https://docs.docker.com/engine/install/"

# Determine backend mode: local cargo or Docker
if command -v cargo &>/dev/null; then
  BACKEND_MODE=local
else
  warn "cargo not found — running backend in Docker instead."
  warn "To run locally, install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  BACKEND_MODE=docker
fi

# Determine frontend mode: local npm or Docker node container
if command -v npm &>/dev/null; then
  FRONTEND_MODE=local
else
  warn "npm not found — running frontend in a Docker node container instead."
  warn "To run locally, install Node.js: https://nodejs.org/"
  FRONTEND_MODE=docker
fi

# ── Check if already running ───────────────────────────────────────────────────
if [ -f "$PID_DIR/backend.pid" ] && kill -0 "$(cat "$PID_DIR/backend.pid")" 2>/dev/null; then
  warn "Backend is already running (PID $(cat "$PID_DIR/backend.pid")). Run ./scripts/restart.sh to restart."
  exit 0
fi

# ── 1. Start infra ─────────────────────────────────────────────────────────────
title "Starting infra (postgres + redis)..."
cd "$ROOT"
docker compose up -d postgres redis

# ── 2. Wait for postgres ────────────────────────────────────────────────────────
info "Waiting for postgres..."
for i in $(seq 1 30); do
  if docker compose exec -T postgres pg_isready -U raftbook -d raftbook &>/dev/null; then
    info "Postgres ready."
    break
  fi
  if [ "$i" -eq 30 ]; then
    die "Postgres did not become ready after 30s. Check: docker compose logs postgres"
  fi
  sleep 1
done

# ── 3. Start backend ────────────────────────────────────────────────────────────
title "Starting backend ($BACKEND_MODE mode)..."
cd "$ROOT"

if [ "$BACKEND_MODE" = "local" ]; then
  info "Logs → logs/backend.log"
  info "First run compiles the binary — this may take a minute."
  DATABASE_URL=postgres://raftbook:raftbook@localhost:5433/raftbook \
    HTTP_ADDR=0.0.0.0:8081 \
    cargo run -p engined >> "$LOG_DIR/backend.log" 2>&1 &
  BACKEND_PID=$!
  echo "$BACKEND_PID" > "$PID_DIR/backend.pid"
  info "Backend PID: $BACKEND_PID"

  sleep 2
  if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
    die "Backend exited immediately. Check logs/backend.log"
  fi
else
  # Docker mode: build + start the engine container
  info "Building and starting engine container (first build may take a few minutes)..."
  docker compose up -d --build engine
  # Use a sentinel PID of 0 to signal Docker mode to stop.sh
  echo "docker" > "$PID_DIR/backend.pid"
  info "Engine running in Docker (logs: docker compose logs -f engine)"
fi

# ── 4. Start frontend ───────────────────────────────────────────────────────────
title "Starting frontend ($FRONTEND_MODE mode)..."
cd "$ROOT"

if [ "$FRONTEND_MODE" = "local" ]; then
  if [ ! -d "$ROOT/frontend/node_modules" ]; then
    info "Installing frontend dependencies (first time)..."
    cd "$ROOT/frontend" && npm install
  fi
  info "Logs → logs/frontend.log"
  cd "$ROOT/frontend"
  npm run dev >> "$LOG_DIR/frontend.log" 2>&1 &
  FRONTEND_PID=$!
  echo "$FRONTEND_PID" > "$PID_DIR/frontend.pid"
  info "Frontend PID: $FRONTEND_PID"

  sleep 1
  if ! kill -0 "$FRONTEND_PID" 2>/dev/null; then
    die "Frontend exited immediately. Check logs/frontend.log"
  fi
else
  # Docker mode: use a node:20-alpine container with host networking so the
  # Vite proxy (localhost:8081) reaches the backend on the host.
  docker rm -f raftbook-frontend-dev &>/dev/null || true
  info "Starting frontend dev container (first run downloads node:20-alpine + installs deps)..."
  docker run -d --rm \
    --name raftbook-frontend-dev \
    --network host \
    -v "$ROOT/frontend:/app" \
    -w /app \
    node:20-alpine \
    sh -c "npm install && npm run dev -- --host 0.0.0.0"
  echo "docker-container:raftbook-frontend-dev" > "$PID_DIR/frontend.pid"
  info "Frontend running in Docker (logs: docker logs -f raftbook-frontend-dev)"
fi

# ── Done ────────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Stack is running!${NC}"
echo -e "  ${BOLD}UI${NC}       → http://localhost:3000"
echo -e "  ${BOLD}GraphQL${NC}  → http://localhost:8081/graphiql"
echo -e "  ${BOLD}gRPC${NC}     → localhost:50051"
echo ""
echo -e "  Tail backend:  ${YELLOW}tail -f $LOG_DIR/backend.log${NC}"
echo -e "  Tail frontend: ${YELLOW}tail -f $LOG_DIR/frontend.log${NC}"
echo -e "  Stop:          ${YELLOW}./scripts/stop.sh${NC}"
