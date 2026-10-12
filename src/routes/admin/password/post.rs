use crate::authentication::{
    AuthError, Credentials, UserId, change_password as change_password_auth, get_username,
    validate_credentials,
};
use axum::Extension;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, SameSite, SignedCookieJar};
use time::Duration;

use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: SecretString,
    new_password: SecretString,
    new_password_check: SecretString,
}

pub async fn change_password(
    State(pool): State<PgPool>,
    jar: SignedCookieJar,
    Extension(UserId(user_id)): Extension<UserId>,
    Form(form): Form<FormData>,
) -> Response {
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        let cookie = Cookie::build((
            "flash",
            "You entered two different new passwords - the field values must match.",
        ))
        .path("/admin/password")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::minutes(5));

        return (jar.add(cookie), Redirect::to("/admin/password")).into_response();
    }

    let password_length = form.new_password.expose_secret().chars().count();

    if !(12..=128).contains(&password_length) {
        let cookie = Cookie::build((
            "flash",
            "The new password must be between 12 and 128 characters long.",
        ))
        .path("/admin/password")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::minutes(5));

        return (jar.add(cookie), Redirect::to("/admin/password")).into_response();
    }

    let username = match get_username(user_id, &pool).await {
        Ok(username) => username,
        Err(e) => {
            tracing::error!("Failed to get username: {:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let credentials = Credentials {
        username,
        password: form.current_password,
    };

    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                let cookie = Cookie::build(("flash", "The current password is incorrect."))
                    .path("/admin/password")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .max_age(Duration::minutes(5));

                (jar.add(cookie), Redirect::to("/admin/password")).into_response()
            }
            AuthError::UnexpectedError(e) => {
                tracing::error!("{:?}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        };
    }

    if let Err(e) = change_password_auth(user_id, form.new_password, &pool).await {
        tracing::error!("Failed to change password: {:?}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let cookie = Cookie::build(("flash", "Your password has been successfully changed."))
        .path("/admin/password")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::minutes(5));

    (jar.add(cookie), Redirect::to("/admin/password")).into_response()
}
