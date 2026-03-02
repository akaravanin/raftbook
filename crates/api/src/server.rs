use std::net::SocketAddr;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tonic::transport::Server as TonicServer;
use tracing::info;

use crate::{graphql, grpc, AppState};

/// Bind and run both servers concurrently.  Neither returns under normal
/// operation; either task completing is treated as a fatal error.
pub async fn run(
    state: AppState,
    grpc_addr: SocketAddr,
    http_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let grpc_fut = run_grpc(state.clone(), grpc_addr);
    let http_fut = run_http(state, http_addr);

    tokio::try_join!(grpc_fut, http_fut)?;
    Ok(())
}

// ── gRPC server (tonic, HTTP/2) ───────────────────────────────────────────────

async fn run_grpc(
    state: AppState,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let exchange_svc = grpc::make_exchange_server(state);

    info!(%addr, "gRPC server listening");

    TonicServer::builder()
        .add_service(exchange_svc)
        .serve(addr)
        .await?;

    Ok(())
}

// ── HTTP server (axum, HTTP/1.1 + WebSocket) ──────────────────────────────────

async fn run_http(
    state: AppState,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = graphql::build_schema(state);
    let app = graphql::router(schema)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive()); // tighten for production

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server listening (GraphQL at /graphql, GraphiQL at /graphiql)");

    axum::serve(listener, app).await?;
    Ok(())
}
