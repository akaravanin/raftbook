use async_graphql::{Context, Object, Result};
use event_log::PostgresEventLog;
use sqlx::PgPool;

use crate::graphql::types::GqlEventRecord;

pub struct Query;

#[Object]
impl Query {
    /// Replay the event log from `from_seq` (inclusive).  Use `fromSeq: 0`
    /// to read the full history.
    async fn events(&self, ctx: &Context<'_>, from_seq: i64) -> Result<Vec<GqlEventRecord>> {
        let pool = ctx.data::<PgPool>()?;
        let log = PostgresEventLog::from_pool(pool.clone());
        let records = log
            .read_from(from_seq as u64)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;
        Ok(records.into_iter().map(GqlEventRecord::from).collect())
    }

    /// Health check — returns the server version string.
    async fn health(&self) -> &str {
        "raftbook ok"
    }
}
