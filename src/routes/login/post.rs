use super::{FLASH_COOKIE_NAME, FLASH_COOKIE_PATH};
use crate::TypedSession;
use crate::authentication::{AuthError, Credentials, validate_credentials};
use crate::startup::ApplicationBaseUrl;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use secrecy::SecretString;
use sqlx::PgPool;
use time::Duration;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: SecretString,
}

#[tracing::instrument(
    skip(form, pool, base_url, jar, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    State(pool): State<PgPool>,
    State(base_url): State<ApplicationBaseUrl>,
    jar: SignedCookieJar,
    session: TypedSession,
    Form(form): Form<FormData>,
) -> Response {
    let credentials = Credentials {
        username: form.username,
        password: form.password,
    };
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));
    match validate_credentials(credentials, &pool).await {
        // Case 1: Valid credentials -> Redirect to "/admin/dashboard"
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(user_id));

            if let Err(e) = session.cycle_id().await {
                tracing::error!("Failed to cycle session ID: {:?}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            if let Err(e) = session.insert_user_id(user_id).await {
                tracing::error!("Failed to insert user_id into session: {:?}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }

            let jar = jar.remove(Cookie::build(FLASH_COOKIE_NAME).path(FLASH_COOKIE_PATH));
            (jar, Redirect::to("/admin/dashboard")).into_response()
        }

        // Case 2: Wrong credentials -> Redirect to "/login"
        Err(AuthError::InvalidCredentials(_)) => {
            let cookie = Cookie::build((FLASH_COOKIE_NAME, "Authentication failed"))
                .path(FLASH_COOKIE_PATH)
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(base_url.0.starts_with("https://"))
                .max_age(Duration::minutes(5));
            (jar.add(cookie), Redirect::to("/login")).into_response()
        }
        // Case 3: DB / server error -> 500 Internal Server Error
        Err(AuthError::UnexpectedError(e)) => {
            tracing::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
