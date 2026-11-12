use argon2::password_hash::{SaltString, rand_core::OsRng};
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::LazyLock;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::Application;
use zero2prod::configuration::{DatabaseSettings, get_configuration};
use zero2prod::email_client::EmailClient;
use zero2prod::issue_delivery_worker::{ExecutionOutcome, try_execute_task};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}
impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }
    async fn store(&self, pool: &PgPool) {
        let salt = SaltString::generate(&mut OsRng);
        // Match parameters of the default password
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash)
    VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(pool)
        .await
        .expect("Failed to store test user.");
    }
}
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub port: u16,
    pub test_user: TestUser,
    pub api_client: reqwest::Client, // Share cookies between login and publishing.
    pub email_client: EmailClient,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/subscriptions", self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();
        // Extract the link from one of the request fields.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };
        let html = get_link(body["html"].as_str().unwrap());
        let plain_text = get_link(body["text"].as_str().unwrap());
        ConfirmationLinks { html, plain_text }
    }

    pub async fn post_newsletters(&self, body: &[(&str, &str)]) -> reqwest::Response {
        self.api_client // Automatically sends the saved session cookie.
            .post(format!("{}/admin/newsletters", self.address))
            .form(body) // URL-encode the fields and set the correct Content-Type.
            .send()
            .await
            .expect("Failed to execute newsletter request")
    }

    pub async fn follow_redirect(
        &self,
        client: &reqwest::Client,
        response: reqwest::Response,
    ) -> reqwest::Response {
        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .expect("Location header missing")
            .to_str()
            .unwrap();
        client
            .get(format!("{}{location}", self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn login(&self) {
        let response = self
            .api_client
            .post(format!("{}/login", self.address))
            .form(&[
                ("username", self.test_user.username.as_str()),
                ("password", self.test_user.password.as_str()),
            ])
            .send()
            .await
            .expect("Failed to execute login request");

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/admin/dashboard"
        ); // Verify login succeeded before continuing the test.
    }

    pub async fn dispatch_all_pending_emails(&self) {
        loop {
            if let ExecutionOutcome::EmptyQueue =
                try_execute_task(&self.db_pool, &self.email_client)
                    .await
                    .unwrap()
            {
                break;
            }
        }
    }
}

// Ensure that the `tracing` stack is only initialized once using `LazyLock`
static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    }
});

pub async fn spawn_app() -> TestApp {
    LazyLock::force(&TRACING);

    let email_server = MockServer::start().await;
    let mut configuration = get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let db_pool = configure_database(&configuration.database).await;

    // Use port 0 so the OS assigns a random free port for tests
    configuration.application.port = 0;
    configuration.email_client.base_url = Some(email_server.uri());
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application");

    let application_port = application.port();

    tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address: format!("http://127.0.0.1:{}", application_port),
        db_pool,
        email_server,
        port: application_port,
        test_user: TestUser::generate(),
        api_client: reqwest::Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Failed to build test client"),
        email_client: configuration.email_client.client(),
    };

    test_app.test_user.store(&test_app.db_pool).await;
    test_app
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // 1. Connect using connect_with and without_db()
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // 2. Migrate using connect_with and with_db()
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}
