mod mutation;
mod query;
mod subscription;
pub mod types;

use async_graphql::{futures_util::StreamExt, Schema};
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::{
        ws::WebSocketUpgrade,
        State,
    },
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};

use crate::AppState;
use mutation::Mutation;
use query::Query;
use subscription::Subscription;

pub type ExchangeSchema = Schema<Query, Mutation, Subscription>;

/// Build the async-graphql schema, wiring in all data sources via context data.
pub fn build_schema(state: AppState) -> ExchangeSchema {
    Schema::build(Query, Mutation, Subscription)
        .data(state.pool.clone())
        .data(state)
        .finish()
}

/// Axum sub-router that mounts GraphQL + GraphiQL endpoints.
pub fn router(schema: ExchangeSchema) -> Router {
    Router::new()
        .route("/graphql", post(graphql_handler).get(graphql_ws_handler))
        .route("/graphiql", get(graphiql_handler))
        .with_state(schema)
}

async fn graphql_handler(
    State(schema): State<ExchangeSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphql_ws_handler(
    State(schema): State<ExchangeSchema>,
    protocol: GraphQLProtocol,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        let (write, read) = socket.split();
        GraphQLWebSocket::new_with_pair(write, read, schema, protocol).serve()
    })
}

async fn graphiql_handler() -> impl IntoResponse {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql")
            .finish(),
    )
}
