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

pub async fn subscriptions(
    State(pool): State<PgPool>,
    form: Result<Form<FormData>, FormRejection>,
) -> StatusCode {
    let Form(form) = match form {
        Ok(f) => f,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(&pool)
    .await
    {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            eprintln!("Failed to execute query: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
