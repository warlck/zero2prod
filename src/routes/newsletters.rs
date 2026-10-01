use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use anyhow::Context;
use argon2::{Algorithm, Argon2, Params, Version};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::Engine;
use secrecy::{ExposeSecret, SecretString};
use sha3::Digest;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

#[derive(thiserror::Error, Debug)]
pub enum PublishError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        match self {
            PublishError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            PublishError::UnexpectedError(e) => {
                tracing::error!("{:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            PublishError::AuthError(_) => (
                StatusCode::UNAUTHORIZED,
                [(header::WWW_AUTHENTICATE, r#"Basic realm="publish""#)],
            )
                .into_response(),
        }
    }
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(pool, email_client, headers, body),
    fields(user_id = tracing::field::Empty)
)]
pub async fn newsletters(
    State(pool): State<PgPool>,
    State(email_client): State<EmailClient>,
    headers: HeaderMap,
    body: Result<Json<BodyData>, JsonRejection>,
) -> Result<StatusCode, PublishError> {
    let credentials = basic_authentication(&headers).map_err(PublishError::AuthError)?;
    let user_id = validate_credentials(credentials, &pool).await?;
    tracing::Span::current().record("user_id", tracing::field::display(user_id));

    let Json(body) = body.map_err(|e| {
        tracing::error!("Failed to parse JSON body: {:?}", e);
        PublishError::ValidationError(e.to_string())
    })?;

    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .context("Failed to get confirmed subscribers")?;

    for subscriber in subscribers {
        match SubscriberEmail::parse(subscriber.email) {
            Ok(email) => {
                email_client
                    .send_email(&email, &body.title, &body.content.html, &body.content.text)
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", email.as_ref())
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    "A confirmed subscriber has an invalid email stored."
                );
            }
        }
    }

    Ok(StatusCode::OK)
}

struct ConfirmedSubscriber {
    email: String,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<ConfirmedSubscriber>, anyhow::Error> {
    let rows = sqlx::query_as!(
        ConfirmedSubscriber,
        r#"
            SELECT email
            FROM subscriptions
            WHERE status = 'confirmed'
            "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug)]
pub struct Credentials {
    pub username: String,
    pub password: SecretString,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // 1. Ensure the Authorization header is present and valid UTF-8
    let header_value = headers
        .get(header::AUTHORIZATION)
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    // 2. Ensure scheme is 'Basic '
    let base64encoded_credentials = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;

    // 3. Decode base64 bytes
    let decoded_credentials = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .context("Failed to base64-decode 'Basic' credentials.")?;

    // 4. Ensure decoded bytes are valid UTF-8
    let decoded_credentials = String::from_utf8(decoded_credentials)
        .context("The decoded credential string is not valid UTF8.")?;

    // 5. Split into username and password on the first ':'
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials {
        username,
        password: SecretString::from(password),
    })
}

#[tracing::instrument(name = "Validate credentials", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let hasher = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None)
            .context("Failed to build Argon2 parameters")
            .map_err(PublishError::UnexpectedError)?,
    );
    let hasher = argon2::Argon2::new();
    let row: Option<_> = sqlx::query!(
        r#"
        SELECT user_id, password_hash, salt
        FROM users
        WHERE username = $1
        "#,
        credentials.username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")
    .map_err(PublishError::UnexpectedError)?;

    let (expected_password_hash, user_id, salt) = match row {
        Some(row) => (row.password_hash, row.user_id, row.salt),
        None => {
            return Err(PublishError::AuthError(anyhow::anyhow!(
                "Unknown username."
            )));
        }
    };

    let password_hash = hasher
        .hash_password(credentials.password.as_bytes(), &salt)
        .context("Failed to hash password")
        .map_err(PublishError::UnexpectedError)?;
    let password_hash = format!("{:x}", password_hash.hash.unwrap());
    if password_hash != expected_password_hash {
        Err(PublishError::AuthError(anyhow::anyhow!(
            "Invalid password."
        )))
    } else {
        Ok(user_id)
    }
}
