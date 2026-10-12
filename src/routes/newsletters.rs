use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use anyhow::Context;

use axum::{
    Extension, Form,
    extract::{State, rejection::FormRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,        // Matches name="title".
    text_content: String, // Matches name="text_content".
    html_content: String, // Matches name="html_content".
}

#[derive(thiserror::Error, Debug)]
pub enum PublishError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl IntoResponse for PublishError {
    fn into_response(self) -> Response {
        match self {
            PublishError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            PublishError::UnexpectedError(e) => {
                tracing::error!("{:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(pool, email_client, body),
    fields(user_id = tracing::field::Empty)
)]
pub async fn newsletters(
    State(pool): State<PgPool>,
    State(email_client): State<EmailClient>,
    Extension(UserId(user_id)): Extension<UserId>,
    body: Result<Form<BodyData>, FormRejection>,
) -> Result<StatusCode, PublishError> {
    tracing::Span::current().record("user_id", tracing::field::display(user_id));

    let Form(body) = body.map_err(|e| {
        tracing::error!("Failed to parse newsletter form: {:?}", e);
        PublishError::ValidationError(e.to_string()) // Preserve the existing 400 response.
    })?;

    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .context("Failed to get confirmed subscribers")?;

    for subscriber in subscribers {
        match SubscriberEmail::parse(subscriber.email) {
            Ok(email) => {
                email_client
                    .send_email(&email, &body.title, &body.html_content, &body.text_content)
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
