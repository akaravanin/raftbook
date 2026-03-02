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
title "Starting backend..."
info "Logs → logs/backend.log"
info "First run compiles the binary — this may take a minute."

cd "$ROOT"
DATABASE_URL=postgres://raftbook:raftbook@localhost:5432/raftbook \
  cargo run -p engined >> "$LOG_DIR/backend.log" 2>&1 &
BACKEND_PID=$!
echo "$BACKEND_PID" > "$PID_DIR/backend.pid"
info "Backend PID: $BACKEND_PID"

# Wait briefly and confirm it didn't crash immediately
sleep 2
if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
  die "Backend exited immediately. Check logs/backend.log"
fi

# ── 4. Start frontend ───────────────────────────────────────────────────────────
title "Starting frontend..."

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

# ── Done ────────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}${BOLD}Stack is running!${NC}"
echo -e "  ${BOLD}UI${NC}       → http://localhost:3000"
echo -e "  ${BOLD}GraphQL${NC}  → http://localhost:8080/graphiql"
echo -e "  ${BOLD}gRPC${NC}     → localhost:50051"
echo ""
echo -e "  Tail backend:  ${YELLOW}tail -f $LOG_DIR/backend.log${NC}"
echo -e "  Tail frontend: ${YELLOW}tail -f $LOG_DIR/frontend.log${NC}"
echo -e "  Stop:          ${YELLOW}./scripts/stop.sh${NC}"
