use axum::{
    Form,
    extract::{State, rejection::FormRejection},
    http::StatusCode,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(pool, form),
    fields(
        subscriber_email = %form.as_ref().map(|f| &f.0.email[..]).unwrap_or(""),
        subscriber_name = %form.as_ref().map(|f| &f.0.name[..]).unwrap_or("")
    )
)]
pub async fn subscriptions(
    State(pool): State<PgPool>,
    form: Result<Form<FormData>, FormRejection>,
) -> StatusCode {
    let Form(form) = match form {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to parse form: {:?}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    match insert_subscriber(&pool, &form).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(pool, form)
)]
pub async fn insert_subscriber(pool: &PgPool, form: &FormData) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}
