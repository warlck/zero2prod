use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use axum::{
    Form,
    extract::{State, rejection::FormRejection},
    http::StatusCode,
};
use chrono::Utc;
use sqlx::PgPool;
use std::convert::{TryFrom, TryInto};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

pub fn parse_subscriber(form: FormData) -> Result<NewSubscriber, String> {
    let name = SubscriberName::parse(form.name)?;
    let email = SubscriberEmail::parse(form.email)?;
    Ok(NewSubscriber { email, name })
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name })
    }
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

    let new_subscriber = match form.try_into() {
        Ok(form) => form,
        Err(e) => {
            tracing::error!("Failed to parse subscriber: {:?}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    match insert_subscriber(&pool, &new_subscriber).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(pool, new_subscriber)
)]
pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
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
