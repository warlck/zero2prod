use crate::helpers::spawn_app;

#[tokio::test]
async fn logout_clears_session_state() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // Log in and keep a copy of the original session cookie.
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
        response.headers()[reqwest::header::LOCATION],
        "/admin/dashboard"
    );

    let session_cookie = response
        .cookies()
        .find(|cookie| cookie.name() == "id")
        .expect("Successful login should issue a session cookie");
    let old_session_cookie = format!("{}={}", session_cookie.name(), session_cookie.value());

    // Verify the session grants access before logging out.
    let response = client
        .get(format!("{}/admin/dashboard", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Log out.
    let response = client
        .post(format!("{}/admin/logout", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[reqwest::header::LOCATION], "/login");

    // The same browser should no longer have access.
    let response = client
        .get(format!("{}/admin/dashboard", app.address))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[reqwest::header::LOCATION], "/login");

    // Reusing the old cookie must not restore access either.
    let replay_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = replay_client
        .get(format!("{}/admin/dashboard", app.address))
        .header(reqwest::header::COOKIE, old_session_cookie)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[reqwest::header::LOCATION], "/login");
}

#[tokio::test]
async fn anonymous_requests_to_admin_routes_are_redirected() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    for (method, path) in [
        (reqwest::Method::GET, "/admin/dashboard"),
        (reqwest::Method::GET, "/admin/password"),
        (reqwest::Method::POST, "/admin/password"),
        (reqwest::Method::POST, "/admin/logout"),
        (reqwest::Method::GET, "/admin/newsletters"),
        (reqwest::Method::POST, "/admin/newsletters"),
    ] {
        // POST requests deliberately have no form body.
        let response = client
            .request(method.clone(), format!("{}{path}", app.address))
            .send()
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            reqwest::StatusCode::SEE_OTHER,
            "{method} {path}"
        );
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/login",
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn public_routes_remain_accessible_without_a_session() {
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    for path in ["/", "/login", "/health_check"] {
        let response = client
            .get(format!("{}{path}", app.address))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::OK, "{path}");
    }
}
