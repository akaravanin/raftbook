use async_graphql::{Context, Result, Subscription};
use async_stream::stream;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tracing::warn;

use crate::{graphql::types::GqlEventRecord, AppState};

pub struct Subscription;

#[Subscription]
impl Subscription {
    /// Live stream of every event appended to the log.
    ///
    /// Connects via WebSocket at `/graphql`.  Use `fromSeq` to receive events
    /// starting from a specific sequence number — events already in the log
    /// at subscription time are not replayed here; use the `events` query for
    /// historical data first, then subscribe from the last seen sequence.
    ///
    /// Example:
    /// ```graphql
    /// subscription {
    ///   eventStream {
    ///     seq
    ///     event { ... on TradeExecuted { price quantity } }
    ///   }
    /// }
    /// ```
    async fn event_stream(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 0)] from_seq: i64,
    ) -> Result<impl Stream<Item = GqlEventRecord>> {
        let state = ctx.data::<AppState>()?;
        let mut rx = state.event_tx.subscribe();
        let from_seq = from_seq as u64;

        Ok(stream! {
            loop {
                match rx.recv().await {
                    Ok(record) if record.seq >= from_seq => {
                        yield GqlEventRecord::from(record);
                    }
                    Ok(_) => {} // skip events before the requested cursor
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "GraphQL subscription receiver lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}
