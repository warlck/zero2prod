use crate::{
    configuration::Settings,
    email_client::EmailClient,
    routes::{
        admin_dashboard, change_password, change_password_form, confirm, health_check, home, login,
        login_form, logout, newsletter_form, newsletters, subscribe,
    },
};
use axum::{
    Router,
    body::Body,
    http::Request,
    routing::{get, post},
};
use axum_extra::extract::cookie::Key;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use uuid::Uuid;

use crate::authentication::reject_anonymous_users;
use axum::extract::FromRef;
use axum::middleware;
use secrecy::{ExposeSecret, SecretString};
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_redis_store::{RedisStore, fred::prelude::*};

#[derive(Clone)]
pub struct HmacSecret(pub SecretString);

#[derive(Clone, Debug)]
pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub email_client: EmailClient,
    pub base_url: ApplicationBaseUrl,
    pub hmac_secret: HmacSecret,
    cookie_key: Key,
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

impl FromRef<AppState> for HmacSecret {
    fn from_ref(state: &AppState) -> Self {
        state.hmac_secret.clone()
    }
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
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
        let redis_config = Config::from_url(configuration.redis_uri.expose_secret())
            .map_err(std::io::Error::other)?;

        let redis_client = Builder::from_config(redis_config)
            .build()
            .map_err(std::io::Error::other)?;

        redis_client.init().await.map_err(std::io::Error::other)?;

        let session_store = RedisStore::new(redis_client);
        let app = run(
            connection_pool,
            email_client,
            configuration.application.base_url,
            HmacSecret(configuration.application.hmac_secret),
            session_store,
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

pub fn run(
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: HmacSecret,
    session_store: RedisStore<Client>,
) -> Router {
    let cookie_key = Key::derive_from(&Sha256::digest(hmac_secret.0.expose_secret().as_bytes()));
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(base_url.starts_with("https://"))
        .with_expiry(Expiry::OnInactivity(time::Duration::minutes(10)));

    let app_state = AppState {
        db_pool,
        email_client,
        base_url: ApplicationBaseUrl(base_url),
        hmac_secret,
        cookie_key,
    };

    let admin_routes = Router::new()
        .route("/admin/dashboard", get(admin_dashboard))
        .route(
            "/admin/password",
            get(change_password_form).post(change_password),
        )
        .route("/admin/logout", post(logout))
        .route("/admin/newsletters", get(newsletter_form).post(newsletters))
        .route_layer(middleware::from_fn(reject_anonymous_users));
    Router::new()
        .route("/", get(home))
        .route("/login", get(login_form))
        .route("/login", post(login))
        .route("/health_check", get(health_check))
        .route("/subscriptions", post(subscribe))
        .route("/subscriptions/confirm", get(confirm))
        .merge(admin_routes)
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
        .layer(session_layer)
        .with_state(app_state)
}
