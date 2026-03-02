mod seed;

use api::AppState;
use command_handler::CommandHandler;
use event_log::PostgresEventLog;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "engined=info,api=debug,command_handler=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://raftbook:raftbook@localhost:5432/raftbook".to_string());

    let grpc_addr = std::env::var("GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    let http_addr = std::env::var("HTTP_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;

    // ── Bootstrap DB + schema ─────────────────────────────────────────────────
    let log = PostgresEventLog::connect(&database_url).await?;
    log.init_schema().await?;

    let pool = log.pool().clone();
    let mut handler = CommandHandler::new(log);

    // ── Seed realistic market data (idempotent) ───────────────────────────────
    seed::seed_market(&mut handler).await?;

    info!("seed complete, starting servers");

    // ── Build shared state and run both servers ───────────────────────────────
    // broadcast capacity of 4096: enough buffer for burst activity without
    // causing slow subscribers to OOM the process.
    let state = AppState::new(handler, pool, 4096);

    api::server::run(state, grpc_addr, http_addr).await
}
