# RaftBook

Event-sourced exchange core in Rust — deterministic matching engine, append-only event log, idempotent command handling, gRPC + GraphQL API, React frontend.

## What's included

| Component | Description |
|---|---|
| `crates/matching_engine` | In-memory order book — price-time priority, limit orders, cancel |
| `crates/event_log` | Append-only event log — in-memory (tests) + Postgres (prod), idempotent append via `command_id` dedupe |
| `crates/command_handler` | Generic command handler — runs matching, persists events idempotently, 8 integration tests |
| `crates/api` | gRPC command plane (tonic) + GraphQL query/subscription plane (async-graphql + axum) |
| `crates/engined` | Binary entry point — seeds DB, starts both servers |
| `frontend/` | React 18 + TypeScript + Vite — live order stream, trade form, embedded GraphiQL |
| `proto/` | Protobuf definitions — `PlaceOrder`, `CancelOrder`, `StreamEvents` |
| `sql/` | Postgres schema — `event_log`, `command_log` tables |
| `scripts/` | `start.sh`, `stop.sh`, `restart.sh` — one-command stack management |

## Quick start

### Option A — One command (Docker, no local tooling required)

```bash
./scripts/start.sh
```

Detects whether `cargo` / `npm` are installed and falls back to Docker automatically. On first run the Rust build takes a few minutes; subsequent starts are fast.

| Endpoint | URL |
|---|---|
| UI | http://localhost:3000 |
| GraphQL / GraphiQL | http://localhost:8081/graphiql |
| gRPC | localhost:50051 |

```bash
./scripts/stop.sh      # stop everything
./scripts/restart.sh   # stop then start
```

Logs: `logs/backend.log`, `logs/frontend.log`
Containers: `docker logs -f raftbook-engine-1`, `docker logs -f raftbook-frontend-dev`

### Option B — Local dev with hot-reload (requires Rust + Node)

```bash
# Terminal 1 — infra
docker compose up -d postgres redis

# Terminal 2 — backend (seeds DB on first boot)
DATABASE_URL=postgres://raftbook:raftbook@localhost:5433/raftbook \
  HTTP_ADDR=0.0.0.0:8081 \
  cargo run -p engined

# Terminal 3 — frontend dev server
cd frontend && npm install && npm run dev
# Open http://localhost:3000
```

### Tests (no DB required)

```bash
cargo test --workspace
```

### `just` shortcuts

```bash
just infra-up     # start postgres + redis
just test         # cargo test --workspace
just ui-install   # npm install in frontend/
just ui-dev       # npm run dev
just grpc-ask     # grpcurl PlaceOrder (requires grpcurl)
just grpc-stream  # stream all events
just events       # psql dump of event_log
just graphiql     # open GraphiQL in browser
```

## Seed data

On every startup the engine runs an idempotent seed (fixed `command_id`s) producing a realistic opening session:

| # | Event | Detail |
|---|---|---|
| 0 | OrderAccepted | MM ask@102 qty10 |
| 1 | OrderAccepted | MM ask@104 qty10 |
| 2 | OrderAccepted | MM bid@99 qty10 |
| 3 | OrderAccepted | MM bid@97 qty10 |
| 4 | OrderAccepted | Taker bid@105 qty15 |
| 5 | TradeExecuted | maker#1 ← taker#5 @ 102, qty 10 |
| 6 | TradeExecuted | maker#2 ← taker#5 @ 104, qty 5 |
| 7 | OrderAccepted | Taker ask@98 qty5 |
| 8 | TradeExecuted | maker#3 ← taker#6 @ 99, qty 5 |
| 9 | OrderCanceled | order#4 (MM bid@97) |

**Final book: Ask@104 qty5 | Bid@99 qty5 | spread = 5 ticks**

## Architecture

```
┌─────────────┐   gRPC :50051    ┌──────────────────────────────────┐
│  gRPC client│ ────────────────▶│  CommandHandler                  │
└─────────────┘                  │  (PlaceOrder / CancelOrder)      │
                                 │       │                          │
┌─────────────┐  GraphQL :8081   │  MatchingEngine  EventLog        │
│  React UI   │ ────────────────▶│       │              │           │
│  /graphql   │◀── subscription ─│  broadcast::Sender<EventRecord>  │
└─────────────┘                  └──────────────────────────────────┘
                                                 │
                                          ┌──────┴──────┐
                                          │  Postgres   │
                                          │ event_log   │
                                          │ command_log │
                                          └─────────────┘
```

- **Matching engine**: BTreeMap price levels, VecDeque time ordering — deterministic price-time priority
- **Event sourcing**: every order placement, fill, and cancel is an immutable record in `event_log`
- **Idempotent commands**: `command_log(command_id → event_seq)` — retries return the original result, no duplicate effects
- **Subscribe-before-replay**: gRPC/GraphQL stream handlers subscribe to the broadcast channel before replaying history — no missed events
- **Generic over log backend**: `CommandHandler<L: IdempotentEventLog>` — tests use `InMemoryEventLog`, prod uses `PostgresEventLog`

## Environment variables

Copy `.env.example` to `.env` and adjust:

```bash
DATABASE_URL=postgres://raftbook:raftbook@localhost:5433/raftbook
GRPC_ADDR=0.0.0.0:50051
HTTP_ADDR=0.0.0.0:8081
RUST_LOG=engined=info,api=debug,command_handler=debug
```

> **Note:** The default credentials (`raftbook:raftbook`) are for local development only. Change them before any production deployment.

## Firewall (if accessing remotely)

```bash
sudo ufw allow 3000/tcp   # UI
sudo ufw allow 8081/tcp   # GraphQL
sudo ufw allow 50051/tcp  # gRPC
```

## Planned next

- Settlement workers with pluggable claim strategies: Postgres `FOR UPDATE SKIP LOCKED`, Redis consumer groups, Raft/etcd leases
- Balance/position projector — replay events into read-model tables
- Full book replay on startup (`OrderPlaced` event) for crash recovery
- End-to-end invariants: balance conservation, no negative free balance
