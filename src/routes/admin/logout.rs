use crate::session_state::TypedSession;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};

pub async fn logout(session: TypedSession) -> Response {
    if let Err(e) = session.log_out().await {
        tracing::error!("Failed to clear session during logout: {:?}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Redirect::to("/login").into_response()
}
