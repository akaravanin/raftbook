# RaftBook

Event-sourced exchange core scaffold in Rust.

## What is included

- `matching_engine`: deterministic price-time matching engine (limit orders + cancel)
- `event_log`: append-only event log abstraction with:
  - in-memory implementation
  - Postgres-backed implementation (`event_log` + `command_log` tables, append/read/len)
  - idempotent append API via `command_id` dedupe
- `engined`: runnable service entrypoint wiring matching + Postgres event logging
- Docker essentials:
  - `Dockerfile` for `engined`
  - `docker-compose.yml` with `engine`, `postgres`, and `redis`
- SQL schema file for the event store:
  - `sql/001_event_log.sql`

## Quick start (local)

```bash
cargo test
export DATABASE_URL=postgres://raftbook:raftbook@localhost:5432/raftbook
cargo run -p engined
```

## Quick start (docker)

```bash
docker compose up --build
```

## Next implementation milestones

1. Build settlement workers with pluggable claim strategies:
   - Postgres `FOR UPDATE SKIP LOCKED`
   - Redis stream/consumer group claims
   - Raft/etcd lease-based claims
2. Add replay and crash-recovery checks from event sequence.
3. Add end-to-end invariants (balance conservation, no negative free balance).
