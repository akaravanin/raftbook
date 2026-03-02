# RaftBook dev workflow
# Requires: just (https://github.com/casey/just), docker, docker compose

# Show available recipes
default:
    @just --list

# ── Frontend (Node/npm required) ──────────────────────────────────────────────

# Install frontend dependencies
ui-install:
    cd frontend && npm install

# Run frontend dev server (proxies /graphql → localhost:8080)
ui-dev:
    cd frontend && npm run dev

# Build for production
ui-build:
    cd frontend && npm run build

# ── Local dev (requires Rust toolchain) ───────────────────────────────────────

# Run all unit + integration tests (no DB needed)
test:
    cargo test --workspace

# Lint
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format
fmt:
    cargo fmt --all

# Check compilation without producing binaries
check:
    cargo check --workspace

# ── Docker ────────────────────────────────────────────────────────────────────

# Start Postgres + Redis in the background
infra-up:
    docker compose up -d postgres redis

# Stop and remove containers (keeps volumes)
infra-down:
    docker compose down

# Build and run the full stack (engine + infra)
up:
    docker compose up --build

# Tear down everything including volumes (destructive — wipes DB)
down-clean:
    docker compose down -v

# Tail engine logs
logs:
    docker compose logs -f engine

# ── Database ──────────────────────────────────────────────────────────────────

# Open a psql shell against the running Postgres container
psql:
    docker compose exec postgres psql -U raftbook -d raftbook

# Dump the event log
events:
    docker compose exec postgres psql -U raftbook -d raftbook \
        -c "SELECT seq, created_at, event FROM event_log ORDER BY seq;"

# ── API smoke tests (requires grpcurl + curl) ─────────────────────────────────

# Place a limit ask via gRPC
grpc-ask:
    grpcurl -plaintext -d '{"command_id":"cmd-ask-1","order_id":1,"user_id":100,"side":"SIDE_ASK","price":100,"quantity":5}' \
        localhost:50051 exchange.v1.Exchange/PlaceOrder

# Place a crossing bid via gRPC (should generate a fill)
grpc-bid:
    grpcurl -plaintext -d '{"command_id":"cmd-bid-1","order_id":2,"user_id":200,"side":"SIDE_BID","price":105,"quantity":5}' \
        localhost:50051 exchange.v1.Exchange/PlaceOrder

# Stream all events from the beginning
grpc-stream:
    grpcurl -plaintext -d '{"from_seq":0}' \
        localhost:50051 exchange.v1.Exchange/StreamEvents

# Open GraphiQL in the browser
graphiql:
    open http://localhost:8081/graphiql 2>/dev/null || xdg-open http://localhost:8081/graphiql
