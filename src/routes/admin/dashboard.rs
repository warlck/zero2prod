use crate::authentication::UserId;
use axum::{
    Extension,
    response::{Html, IntoResponse, Response},
};

pub async fn admin_dashboard(Extension(UserId(user_id)): Extension<UserId>) -> Response {
    Html(format!(
        r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="utf-8">
                <title>Admin dashboard</title>
            </head>
            <body>
                <h1>Admin dashboard</h1>
            <p>Welcome {user_id}!</p>
            <p>Available actions:</p>
            <ol>
                <li><a href="/admin/newsletters">Send a newsletter issue</a></li>
                <li><a href="/admin/password">Change password</a></li>
                <li>
                <form name="logoutForm" action="/admin/logout" method="post">
                <input type="submit" value="Logout">
                </form>
                </li>
            </ol>
            </body>
        </html>"#
    ))
    .into_response()
}
