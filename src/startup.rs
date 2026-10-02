use crate::{
    configuration::Settings,
    email_client::EmailClient,
    routes::{confirm, health_check, home, login, login_form, newsletters, subscribe},
};
use axum::{
    Router,
    body::Body,
    http::Request,
    routing::{get, post},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use uuid::Uuid;

use axum::extract::FromRef;

#[derive(Clone, Debug)]
pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub email_client: EmailClient,
    pub base_url: ApplicationBaseUrl,
}
impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.db_pool.clone()
    }
}
impl FromRef<AppState> for EmailClient {
    fn from_ref(state: &AppState) -> Self {
        state.email_client.clone()
    }
}

impl FromRef<AppState> for ApplicationBaseUrl {
    fn from_ref(state: &AppState) -> Self {
        state.base_url.clone()
    }
}

pub struct Application {
    port: u16,
    listener: TcpListener,
    app: Router,
}

impl Application {
    pub async fn build(
        configuration: Settings,
        email_client: EmailClient,
    ) -> Result<Self, std::io::Error> {
        let connection_pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(configuration.database.with_db());

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address).await?;
        let port = listener.local_addr().unwrap().port();
        let app = run(
            connection_pool,
            email_client,
            configuration.application.base_url,
        );
        Ok(Self {
            port,
            listener,
            app,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> std::io::Result<()> {
        axum::serve(self.listener, self.app).await
    }
}

pub fn run(db_pool: PgPool, email_client: EmailClient, base_url: String) -> Router {
    let app_state = AppState {
        db_pool,
        email_client,
        base_url: ApplicationBaseUrl(base_url),
    };

    Router::new()
        .route("/", get(home))
        .route("/login", get(login_form))
        .route("/login", post(login))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .route("/subscriptions/confirm", get(confirm))
        .route("/newsletters", post(newsletters))
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
        .with_state(app_state)
}
