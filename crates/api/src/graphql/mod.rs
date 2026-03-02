mod mutation;
mod query;
mod subscription;
pub mod types;

use async_graphql::Schema;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

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
    protocol: GraphQLSubscription,
) -> impl IntoResponse {
    protocol.on_upgrade(schema)
}

async fn graphiql_handler() -> impl IntoResponse {
    Html(
        async_graphql::http::GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/graphql")
            .finish(),
    )
}
