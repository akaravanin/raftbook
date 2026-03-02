use std::collections::HashMap;

use async_trait::async_trait;
use matching_engine::{OrderId, Trade};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::{PgPool, Postgres, Row, Transaction};
use thiserror::Error;

// ── Domain events ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    OrderAccepted {
        order_id: OrderId,
    },
    TradeExecuted {
        trade: Trade,
    },
    OrderCanceled {
        order_id: OrderId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventRecord {
    pub seq: u64,
    pub event: Event,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotentAppendResult {
    pub record: EventRecord,
    pub inserted: bool,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum EventLogError {
    #[error("sequence overflow")]
    SequenceOverflow,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid sequence value from database: {0}")]
    InvalidSequence(i64),
    #[error("command exists but no event record was found for command_id: {0}")]
    MissingCommandEvent(String),
}

// ── Sync append-only trait (used by in-memory impl in unit tests) ─────────────

pub trait AppendOnlyLog {
    fn append(&mut self, event: Event) -> Result<EventRecord, EventLogError>;
    fn read_from(&self, from_seq: u64) -> Vec<EventRecord>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Async idempotent log trait (used by CommandHandler) ───────────────────────
//
// Both InMemoryEventLog and PostgresEventLog implement this so CommandHandler
// can be generic over the log backend: fast in-memory tests, Postgres in prod.

#[async_trait]
pub trait IdempotentEventLog: Send {
    async fn append_idempotent(
        &mut self,
        command_id: &str,
        event: Event,
    ) -> Result<IdempotentAppendResult, EventLogError>;

    async fn read_from_async(&self, from_seq: u64) -> Result<Vec<EventRecord>, EventLogError>;
}

// ── In-memory implementation ──────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct InMemoryEventLog {
    records: Vec<EventRecord>,
    /// Maps command_id → event seq for deduplication.
    command_index: HashMap<String, u64>,
}

impl InMemoryEventLog {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AppendOnlyLog for InMemoryEventLog {
    fn append(&mut self, event: Event) -> Result<EventRecord, EventLogError> {
        let seq: u64 = self
            .records
            .len()
            .try_into()
            .map_err(|_| EventLogError::SequenceOverflow)?;
        let record = EventRecord { seq, event };
        self.records.push(record.clone());
        Ok(record)
    }

    fn read_from(&self, from_seq: u64) -> Vec<EventRecord> {
        self.records
            .iter()
            .filter(|r| r.seq >= from_seq)
            .cloned()
            .collect()
    }

    fn len(&self) -> usize {
        self.records.len()
    }
}

#[async_trait]
impl IdempotentEventLog for InMemoryEventLog {
    async fn append_idempotent(
        &mut self,
        command_id: &str,
        event: Event,
    ) -> Result<IdempotentAppendResult, EventLogError> {
        if let Some(&seq) = self.command_index.get(command_id) {
            let record = self.records[seq as usize].clone();
            return Ok(IdempotentAppendResult {
                record,
                inserted: false,
            });
        }
        let record = self.append(event)?;
        self.command_index
            .insert(command_id.to_string(), record.seq);
        Ok(IdempotentAppendResult {
            record,
            inserted: true,
        })
    }

    async fn read_from_async(&self, from_seq: u64) -> Result<Vec<EventRecord>, EventLogError> {
        Ok(self.read_from(from_seq))
    }
}

// ── Postgres implementation ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PostgresEventLog {
    pool: PgPool,
}

impl PostgresEventLog {
    pub async fn connect(database_url: &str) -> Result<Self, EventLogError> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    /// Build from an existing pool — use this when the pool is shared
    /// between the command handler and read-only query paths (e.g. API layer).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Expose the underlying pool so callers can share it with other components.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn init_schema(&self) -> Result<(), EventLogError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_log (
                seq BIGSERIAL PRIMARY KEY,
                event JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS command_log (
                command_id TEXT PRIMARY KEY,
                event_seq BIGINT NOT NULL UNIQUE REFERENCES event_log(seq),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn append(&self, event: Event) -> Result<EventRecord, EventLogError> {
        let row = sqlx::query(
            r#"
            INSERT INTO event_log (event)
            VALUES ($1)
            RETURNING seq, event
            "#,
        )
        .bind(Json(event))
        .fetch_one(&self.pool)
        .await?;
        self.row_to_record(&row)
    }

    pub async fn read_from(&self, from_seq: u64) -> Result<Vec<EventRecord>, EventLogError> {
        let from_seq: i64 = from_seq
            .try_into()
            .map_err(|_| EventLogError::SequenceOverflow)?;
        let rows = sqlx::query(
            r#"
            SELECT seq, event
            FROM event_log
            WHERE seq >= $1
            ORDER BY seq ASC
            "#,
        )
        .bind(from_seq)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(self.row_to_record(&row)?);
        }
        Ok(out)
    }

    pub async fn len(&self) -> Result<usize, EventLogError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event_log")
            .fetch_one(&self.pool)
            .await?;
        count
            .try_into()
            .map_err(|_| EventLogError::InvalidSequence(count))
    }

    async fn pg_append_idempotent(
        &self,
        command_id: &str,
        event: Event,
    ) -> Result<IdempotentAppendResult, EventLogError> {
        if let Some(record) = self.lookup_command_event(command_id).await? {
            return Ok(IdempotentAppendResult {
                record,
                inserted: false,
            });
        }

        let mut tx = self.pool.begin().await?;
        let inserted = self.append_in_tx(&mut tx, event).await?;

        let cmd_res = sqlx::query(
            r#"INSERT INTO command_log (command_id, event_seq) VALUES ($1, $2)"#,
        )
        .bind(command_id)
        .bind(i64::try_from(inserted.seq).map_err(|_| EventLogError::SequenceOverflow)?)
        .execute(&mut *tx)
        .await;

        match cmd_res {
            Ok(_) => {
                tx.commit().await?;
                Ok(IdempotentAppendResult {
                    record: inserted,
                    inserted: true,
                })
            }
            Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
                tx.rollback().await?;
                let existing = self
                    .lookup_command_event(command_id)
                    .await?
                    .ok_or_else(|| EventLogError::MissingCommandEvent(command_id.to_string()))?;
                Ok(IdempotentAppendResult {
                    record: existing,
                    inserted: false,
                })
            }
            Err(e) => {
                tx.rollback().await?;
                Err(EventLogError::Database(e))
            }
        }
    }

    async fn append_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event: Event,
    ) -> Result<EventRecord, EventLogError> {
        let row = sqlx::query(
            r#"
            INSERT INTO event_log (event)
            VALUES ($1)
            RETURNING seq, event
            "#,
        )
        .bind(Json(event))
        .fetch_one(&mut **tx)
        .await?;
        self.row_to_record(&row)
    }

    async fn lookup_command_event(
        &self,
        command_id: &str,
    ) -> Result<Option<EventRecord>, EventLogError> {
        let row = sqlx::query(
            r#"
            SELECT e.seq, e.event
            FROM command_log c
            JOIN event_log e ON e.seq = c.event_seq
            WHERE c.command_id = $1
            "#,
        )
        .bind(command_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| self.row_to_record(&r)).transpose()
    }

    fn row_to_record(&self, row: &sqlx::postgres::PgRow) -> Result<EventRecord, EventLogError> {
        let seq_i64: i64 = row.get("seq");
        let seq = seq_i64
            .try_into()
            .map_err(|_| EventLogError::InvalidSequence(seq_i64))?;
        let event_json: Json<Event> = row.get("event");
        Ok(EventRecord {
            seq,
            event: event_json.0,
        })
    }
}

#[async_trait]
impl IdempotentEventLog for PostgresEventLog {
    async fn append_idempotent(
        &mut self,
        command_id: &str,
        event: Event,
    ) -> Result<IdempotentAppendResult, EventLogError> {
        PostgresEventLog::pg_append_idempotent(self, command_id, event).await
    }

    async fn read_from_async(&self, from_seq: u64) -> Result<Vec<EventRecord>, EventLogError> {
        PostgresEventLog::read_from(self, from_seq).await
    }
}
