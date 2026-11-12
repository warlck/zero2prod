use crate::idempotency::{NextAction, save_response, try_processing};
use crate::{authentication::UserId, idempotency::IdempotencyKey};
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
    idempotency_key: String,
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
    skip(pool, body),
    fields(user_id = tracing::field::Empty)
)]
pub async fn newsletters(
    State(pool): State<PgPool>,
    Extension(UserId(user_id)): Extension<UserId>,
    body: Result<Form<BodyData>, FormRejection>,
) -> Result<Response, PublishError> {
    tracing::Span::current().record("user_id", tracing::field::display(user_id));

    let Form(body) = body.map_err(|e| {
        tracing::error!("Failed to parse newsletter form: {:?}", e);
        PublishError::ValidationError(e.to_string()) // Preserve the existing 400 response.
    })?;

    let idempotency_key: IdempotencyKey = body
        .idempotency_key
        .try_into()
        .map_err(|e: anyhow::Error| PublishError::ValidationError(e.to_string()))?;

    let mut transaction = match try_processing(&pool, &idempotency_key, user_id).await? {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            return Ok(saved_response);
        }
    };

    let issue_id = insert_newsletter_issue(
        &mut transaction,
        &body.title,
        &body.text_content,
        &body.html_content,
    )
    .await
    .context("Failed to store newsletter issue details")?;

    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")?;

    let response = StatusCode::OK.into_response();
    let response = save_response(transaction, &idempotency_key, user_id, response).await?;
    Ok(response)
}

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
    .execute(&mut **transaction)
    .await?;

    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
             INSERT INTO issue_delivery_queue (
                 newsletter_issue_id,
                 subscriber_email
             )
             SELECT $1, email
             FROM subscriptions
             WHERE status = 'confirmed'
             "#,
        newsletter_issue_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
}
