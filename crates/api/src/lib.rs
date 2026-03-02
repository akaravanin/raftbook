//! `api` crate — gRPC command plane + GraphQL query/mutation/subscription plane.
//!
//! Both servers run in the same Tokio runtime:
//! - **gRPC** on port 50051 — `PlaceOrder`, `CancelOrder`, `StreamEvents`
//! - **HTTP** on port 8080 — GraphQL (`/graphql`), GraphiQL (`/graphiql`)
//!
//! They share `AppState` which carries:
//! - A `Mutex<CommandHandler>` for serialised book mutations.
//! - A `broadcast::Sender<EventRecord>` so newly persisted events are pushed
//!   to all active gRPC streams and GraphQL subscriptions without polling.
//! - A raw `PgPool` for cheap read queries in GraphQL resolvers without
//!   needing to acquire the command-handler lock.

pub mod graphql;
pub mod grpc;
pub mod server;

use std::sync::Arc;

use command_handler::CommandHandler;
use event_log::{EventRecord, PostgresEventLog};
use sqlx::PgPool;
use tokio::sync::{broadcast, Mutex};

/// Shared, cheaply-cloneable application state.
#[derive(Clone)]
pub struct AppState {
    /// Serialised access to the matching engine + idempotent event writes.
    pub handler: Arc<Mutex<CommandHandler<PostgresEventLog>>>,

    /// Newly persisted `EventRecord`s are broadcast here so streaming
    /// consumers (gRPC `StreamEvents`, GraphQL `eventStream` subscription)
    /// receive them without polling the database.
    ///
    /// Subscribers should subscribe *before* replaying history to avoid the
    /// race between replay completion and live stream start.
    pub event_tx: broadcast::Sender<EventRecord>,

    /// Shared pool for read-only GraphQL resolvers.  Safe to use concurrently;
    /// `PgPool` is internally `Arc`-wrapped.
    pub pool: PgPool,
}

impl AppState {
    pub fn new(
        handler: CommandHandler<PostgresEventLog>,
        pool: PgPool,
        broadcast_capacity: usize,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(broadcast_capacity);
        Self {
            handler: Arc::new(Mutex::new(handler)),
            event_tx,
            pool,
        }
    }
}
