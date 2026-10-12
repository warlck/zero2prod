use super::{FLASH_COOKIE_NAME, FLASH_COOKIE_PATH};
use axum::response::Html;
use axum_extra::extract::cookie::{Cookie, SignedCookieJar};

pub async fn login_form(jar: SignedCookieJar) -> (SignedCookieJar, Html<String>) {
    let messages_html = match jar.get(FLASH_COOKIE_NAME) {
        None => String::new(),
        Some(cookie) => {
            format!(
                "<p><i>{}</i></p>",
                htmlescape::encode_minimal(cookie.value())
            )
        }
    };
    let jar = jar.remove(Cookie::build(FLASH_COOKIE_NAME).path(FLASH_COOKIE_PATH));
    (
        jar,
        Html(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {messages_html}
    <form action="/login" method="post">
        <label>Username
            <input type="text" placeholder="Enter Username" name="username">
        </label>
        <label>Password
            <input type="password" placeholder="Enter Password" name="password">
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        )),
    )
}
