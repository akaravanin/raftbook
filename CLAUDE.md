# RaftBook — Architecture & Developer Guide

Event-sourced exchange core: deterministic matching engine, append-only log,
idempotent command handling, gRPC + GraphQL API, React frontend.

## Quick Start

### Option A — Docker (backend only)
```bash
docker compose up --build
# gRPC command plane  → localhost:50051
# GraphQL / GraphiQL  → localhost:8081/graphiql
```

### Option B — Local dev with UI hot-reload (recommended)
```bash
# Terminal 1 — infra
docker compose up -d postgres redis

# Terminal 2 — backend (seeds DB on first boot)
DATABASE_URL=postgres://raftbook:raftbook@localhost:5433/raftbook HTTP_ADDR=0.0.0.0:8081 cargo run -p engined

# Terminal 3 — frontend dev server (proxies /graphql → :8080)
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

---

## Core Principles

- Deterministic matching engine with price-time priority.
- Append-only event log as the source of truth.
- Idempotent command handling via persistent command dedupe.
- Replay-first design for crash recovery and auditability.
- Pluggable distributed claim/lease strategy for settlement workers.

## Components

### 1) Matching Engine (`crates/matching_engine`)

- In-memory order book.
- Limit order matching with strict price-time ordering.
- Cancel support.
- Deterministic behavior covered by tests.

### 2) Event Store (`crates/event_log`)

- `Event` model:
  - `OrderAccepted`
  - `TradeExecuted`
  - `OrderCanceled`
- Postgres-backed append-only table `event_log`.
- In-memory implementation retained for fast unit tests.

### 3) Idempotent Command Handling

- `IdempotentEventLog` async trait implemented by both `InMemoryEventLog` and `PostgresEventLog`.
- Table `command_log(command_id, event_seq)` persists command dedupe state in Postgres.
- `append_idempotent(command_id, event)` flow:
  1. Look up `command_id` in `command_log`.
  2. If found: return original event record (`inserted=false`).
  3. If not found: in a transaction append to `event_log` and bind command to `event_seq`.
  4. On concurrent duplicate conflict: rollback and return existing record.

### 4) Command Handler (`crates/command_handler`)

- `Command` enum: `PlaceOrder { command_id, order }`, `CancelOrder { command_id, order_id }`.
- `CommandHandler<L: IdempotentEventLog>` — generic over log backend for testability.
- `handle(cmd) → CommandResult`: runs matching, then persists events idempotently.
- Trade events use derived keys `{command_id}-trade-{n}` for per-fill deduplication.
- `restore_from_log()`: replay placeholder — full book replay pending `OrderPlaced` event (Phase 4).
- 8 integration tests covering: resting, full fill, partial fill, cancel, cancel-unknown, idempotent place, idempotent cancel, event log sequencing.

### 5) Engine Service + Seeder (`crates/engined`)

- Initializes Postgres schema via `PostgresEventLog::init_schema()`.
- Runs `seed::seed_market()` on every startup (fully idempotent via fixed `command_id`s).
- Starts gRPC server (`:50051`) and HTTP/GraphQL server (`:8080`) via `api::server::run()`.

#### Seed Scenario (`src/seed.rs`)

Produces 10 domain events representing a realistic opening session:

| # | Event | Detail |
|---|-------|--------|
| 0 | OrderAccepted | MM ask@102 qty10 (order #1, user #10) |
| 1 | OrderAccepted | MM ask@104 qty10 (order #2, user #10) |
| 2 | OrderAccepted | MM bid@99 qty10 (order #3, user #20) |
| 3 | OrderAccepted | MM bid@97 qty10 (order #4, user #20) |
| 4 | OrderAccepted | Taker bid@105 qty15 (order #5, user #30) |
| 5 | TradeExecuted | maker #1 ← taker #5 @ 102, qty 10 |
| 6 | TradeExecuted | maker #2 ← taker #5 @ 104, qty 5 |
| 7 | OrderAccepted | Taker ask@98 qty5 (order #6, user #40) |
| 8 | TradeExecuted | maker #3 ← taker #6 @ 99, qty 5 |
| 9 | OrderCanceled | order #4 (MM bid@97) |

Final resting book: **Ask@104 qty 5 | Bid@99 qty 5 | spread = 5 ticks**

### 6) API Layer (`crates/api`)

- **gRPC command plane** (`tonic 0.12` + `prost 0.13`): `PlaceOrder`, `CancelOrder`, server-streaming `StreamEvents`.
  - Subscribe-before-replay pattern: broadcast receiver opened before DB replay to prevent event gaps.
  - Proto definitions: `proto/exchange.proto` (`exchange.v1` package).
- **GraphQL query plane** (`async-graphql 7` + `axum 0.7`):
  - Query: `events(fromSeq: Int!): [GqlEventRecord!]!`, `health: String`
  - Mutation: `placeOrder(...)`, `cancelOrder(...)`
  - Subscription: `eventStream(fromSeq: Int)` via WebSocket (`graphql-ws` protocol)
  - GraphiQL explorer at `/graphiql`
- Both served from one binary; gRPC on `:50051`, HTTP/GraphQL on `:8080`.
- `AppState` wraps `Arc<Mutex<CommandHandler>>` + `broadcast::Sender<EventRecord>` + `PgPool`.
- Read queries use `PgPool` directly — no lock contention with writes.

### 7) React Frontend (`frontend/`)

- TypeScript + React 18 + Vite 5.
- **urql 4** GraphQL client with `graphql-ws` subscription exchange.
- Pages:
  - `/` — Dashboard: health status + event history table.
  - `/trade` — TradeForm: PlaceOrder / CancelOrder forms with auto-generated `commandId`.
  - `/events` — EventStream: live subscription feed with pause/resume/clear.
  - `/explorer` — embedded GraphiQL playground.
- Vite dev server proxies `/graphql` (HTTP + WebSocket) to `:8080`.
- Dark terminal theme (`--primary:#00d4aa`, `--bid:#22c55e`, `--ask:#ef4444`).

## Data Model (Postgres)

- `event_log`
  - `seq BIGSERIAL PRIMARY KEY`
  - `event JSONB NOT NULL`
  - `created_at TIMESTAMPTZ`
- `command_log`
  - `command_id TEXT PRIMARY KEY`
  - `event_seq BIGINT UNIQUE REFERENCES event_log(seq)`
  - `created_at TIMESTAMPTZ`

Schema source: `sql/001_event_log.sql`.

## Planned: Distributed Settlement Layer

Settlement workers will consume events and claim work via pluggable backends:

1. Postgres row claims (`FOR UPDATE SKIP LOCKED`).
2. Redis consumer-group or claim semantics.
3. Raft/etcd lease-based claim coordination.

Each strategy must preserve:

- At-least-once processing semantics.
- Idempotent effect application (dedupe keys in side-effect tables).
- Crash-safe handoff and reclaim.

## End-to-End Flow

1. Command API receives `command_id` + payload.
2. Validate + risk checks.
3. Matching decision.
4. Append domain events idempotently.
5. Project balances/positions/read models from event stream.
6. Settlement workers claim and process pending tasks.
7. Replay + invariant checks validate correctness.

## Invariants to Enforce

- No negative free balances.
- Asset conservation (debits == credits per asset domain).
- Deterministic replay produces identical state.
- Command retries do not create duplicate business effects.
