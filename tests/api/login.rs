use crate::helpers::spawn_app;

#[tokio::test]
async fn login_flash_message_is_stored_in_a_cookie_and_shown_once() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let response = client
        .post(format!("{}/login", app.address))
        .form(&[
            ("username", app.test_user.username.as_str()),
            ("password", "wrong-password"),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap(),
        "/login"
    );

    // Inspect the Set-Cookie header, not the response body.
    let flash_cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "flash")
        .expect("Expected a flash cookie");

    assert!(!flash_cookie.value().is_empty());
    assert!(flash_cookie.http_only());

    // The same client automatically sends the stored cookie.
    let response = client
        .get(format!("{}/login", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // The server should instruct the browser to delete the cookie.
    let deletion_cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "flash")
        .expect("Expected the flash cookie to be cleared");

    let expired = deletion_cookie
        .expires()
        .is_some_and(|expiry| expiry <= std::time::SystemTime::now());

    assert!(
        deletion_cookie.max_age() == Some(std::time::Duration::ZERO) || expired,
        "Flash cookie must expire after being consumed"
    );

    let body = response.text().await.unwrap();
    assert!(body.contains("<p><i>Authentication failed</i></p>"));

    // Refreshing should not display the message again.
    let response = client
        .get(format!("{}/login", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let body = response.text().await.unwrap();
    assert!(!body.contains("Authentication failed"));
    assert!(body.contains("<form"));
}

#[tokio::test]
async fn redirect_to_admin_dashboard_after_login_success() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true) // <--- ensure cookie_store is enabled
        .build()
        .unwrap();

    let response = client
        .post(format!("{}/login", app.address))
        .form(&[
            ("username", app.test_user.username.as_str()),
            ("password", app.test_user.password.as_str()),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap(),
        "/admin/dashboard"
    );

    let session_cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "id")
        .expect("Successful login should issue a session cookie");

    assert!(!session_cookie.value().is_empty());
    assert!(session_cookie.http_only());

    let response = client
        .get(format!("{}/admin/dashboard", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(
        response.headers()[reqwest::header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );

    let body = response.text().await.unwrap();
    assert!(body.contains("<title>Admin dashboard</title>"));
    assert!(body.contains("<h1>Admin dashboard</h1>"));
    assert!(body.contains(&format!("<p>Welcome {}!</p>", app.test_user.user_id)));
}

#[tokio::test]
async fn you_must_be_logged_in_to_access_the_admin_dashboard() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = client
        .get(format!("{}/admin/dashboard", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap(),
        "/login"
    );
}
