use crate::routes::{health_check, subscriptions};
use axum::{
    Router,
    body::Body,
    http::Request,
    routing::{get, post},
};
use sqlx::PgPool;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use uuid::Uuid;

pub async fn run(listener: TcpListener, db_pool: PgPool) -> std::io::Result<()> {
    let app = Router::new()
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscriptions))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<Body>| {
                info_span!(
                    "request",
                    method=%request.method(),
                    uri=%request.uri(),
                    request_id= %Uuid::new_v4(),
                )
            }),
        )
        .with_state(db_pool);

    axum::serve(listener, app).await
}
