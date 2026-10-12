use crate::helpers::spawn_app;
use uuid::Uuid;

#[tokio::test]
async fn you_must_be_logged_in_to_see_the_change_password_form() {
    // Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    // Act
    let response = client
        .get(format!("{}/admin/password", app.address))
        .send()
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap(),
        "/login"
    );
}

#[tokio::test]
async fn you_must_be_logged_in_to_change_your_password() {
    // Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let new_password = Uuid::new_v4().to_string();

    // Act
    let response = client
        .post(format!("{}/admin/password", app.address))
        .form(&[
            ("current_password", &Uuid::new_v4().to_string()),
            ("new_password", &new_password),
            ("new_password_check", &new_password),
        ])
        .send()
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION]
            .to_str()
            .unwrap(),
        "/login"
    );
}

#[tokio::test]
async fn new_password_fields_must_match() {
    // Arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let another_new_password = Uuid::new_v4().to_string();

    // A client that retains session cookies across requests
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // Act - Part 1: Log in
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

    // Act - Part 2: Try to change password
    let response = client
        .post(format!("{}/admin/password", app.address))
        .form(&[
            ("current_password", app.test_user.password.as_str()),
            ("new_password", &new_password),
            ("new_password_check", &another_new_password),
        ])
        .send()
        .await
        .unwrap();

    // Act - Part 3: Follow redirect
    let response = app.follow_redirect(&client, response).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let html_page = response.text().await.unwrap();
    assert!(html_page.contains(
        "<p><i>You entered two different new passwords - the field values must match.</i></p>"
    ));
}

#[tokio::test]
async fn current_password_must_be_valid() {
    // Arrange
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // Log in with valid credentials.
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

    // Act: submit matching new passwords, but an invalid current password.
    let response = client
        .post(format!("{}/admin/password", app.address))
        .form(&[
            ("current_password", "wrong-password"),
            ("new_password", new_password.as_str()),
            ("new_password_check", new_password.as_str()),
        ])
        .send()
        .await
        .unwrap();

    // Assert: redirect back to the form and explain the failure.
    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION],
        "/admin/password"
    );

    let response = app.follow_redirect(&client, response).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let html_page = response.text().await.unwrap();
    assert!(html_page.contains("<p><i>The current password is incorrect.</i></p>"));

    // A fresh client must still be able to log in with the original password.
    let fresh_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let response = fresh_client
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
}

#[tokio::test]
async fn new_password_length_must_be_between_12_and_128_characters() {
    let cases = [
        ("empty", String::new(), false),
        ("too short", "a".repeat(11), false),
        ("minimum length", "a".repeat(12), true),
        ("maximum length", "a".repeat(128), true),
        ("too long", "a".repeat(129), false),
        ("short Unicode", "é".repeat(11), false),
        ("minimum Unicode", "é".repeat(12), true),
        ("maximum Unicode", "é".repeat(128), true),
        ("long Unicode", "é".repeat(129), false),
    ];

    for (case, new_password, valid) in cases {
        // Each case gets its own user and database.
        let app = spawn_app().await;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(true)
            .build()
            .unwrap();

        // Log in with the original password.
        let response = client
            .post(format!("{}/login", app.address))
            .form(&[
                ("username", app.test_user.username.as_str()),
                ("password", app.test_user.password.as_str()),
            ])
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER, "{case}");
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/admin/dashboard",
            "{case}"
        );

        // Submit matching passwords to isolate length validation.
        let response = client
            .post(format!("{}/admin/password", app.address))
            .form(&[
                ("current_password", app.test_user.password.as_str()),
                ("new_password", new_password.as_str()),
                ("new_password_check", new_password.as_str()),
            ])
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER, "{case}");
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/admin/password",
            "{case}"
        );

        let response = app.follow_redirect(&client, response).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK, "{case}");

        let html = response.text().await.unwrap();
        assert_eq!(
            html.contains(
                "<p><i>The new password must be between 12 and 128 characters long.</i></p>"
            ),
            !valid,
            "{case}"
        );

        // Verify accepted passwords work and rejected changes preserve
        // the original password. Use a client without an existing session.
        let expected_password = if valid {
            new_password.as_str()
        } else {
            app.test_user.password.as_str()
        };

        let fresh_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let response = fresh_client
            .post(format!("{}/login", app.address))
            .form(&[
                ("username", app.test_user.username.as_str()),
                ("password", expected_password),
            ])
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER, "{case}");
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/admin/dashboard",
            "{case}"
        );
    }
}

#[tokio::test]
async fn changing_password_works() {
    let app = spawn_app().await;
    let new_password = Uuid::new_v4().to_string();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    // Log in with the original password.
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

    // Change the password.
    let response = client
        .post(format!("{}/admin/password", app.address))
        .form(&[
            ("current_password", app.test_user.password.as_str()),
            ("new_password", new_password.as_str()),
            ("new_password_check", new_password.as_str()),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[reqwest::header::LOCATION],
        "/admin/password"
    );

    let response = app.follow_redirect(&client, response).await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let html = response.text().await.unwrap();
    assert!(html.contains("<p><i>Your password has been successfully changed.</i></p>"));

    // Check both passwords without an existing authenticated session.
    for (case, password, expected_location) in [
        ("old password", app.test_user.password.as_str(), "/login"),
        ("new password", new_password.as_str(), "/admin/dashboard"),
    ] {
        let fresh_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        let response = fresh_client
            .post(format!("{}/login", app.address))
            .form(&[
                ("username", app.test_user.username.as_str()),
                ("password", password),
            ])
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER, "{case}");
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            expected_location,
            "{case}"
        );
    }
}
