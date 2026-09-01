use crate::routes::{health_check, subscriptions};
use axum::{
    Router,
    routing::{get, post},
};
use sqlx::PgPool;
use tokio::net::TcpListener;

pub async fn run(listener: TcpListener, db_pool: PgPool) -> std::io::Result<()> {
    let app = Router::new()
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscriptions))
        .with_state(db_pool);

    axum::serve(listener, app).await
}
