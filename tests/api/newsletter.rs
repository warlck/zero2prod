use wiremock::{
    Mock, ResponseTemplate,
    matchers::{any, method, path},
};

use crate::helpers::{ConfirmationLinks, TestApp, spawn_app};

fn newsletter_form_data() -> [(&'static str, &'static str); 3] {
    [
        ("title", "Newsletter title"),                      // Email subject.
        ("text_content", "Newsletter body as plain text"),  // Plain-text version.
        ("html_content", "<p>Newsletter body as HTML</p>"), // HTML version.
    ]
}

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    app.login().await;
    create_unconfirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        // We assert that no request is fired at the email server!
        .expect(0)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletters(&newsletter_form_data()).await;
    assert_eq!(response.status().as_u16(), 200);
    // Mock verifies on Drop that we haven't sent the newsletter email
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    // Arrange
    let app = spawn_app().await;
    app.login().await;
    create_confirmed_subscriber(&app).await;

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "id": "mocked-email-id" })),
        )
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app
        .post_newsletters(&newsletter_form_data()) // Borrow the form fields.
        .await;

    assert_eq!(response.status().as_u16(), 200);
    // When app drops, wiremock verifies that 1 email was sent!
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    app.login().await; // Ensure the request reaches form validation.

    let test_cases = [
        (
            vec![
                ("text_content", "Plain text"),
                ("html_content", "<p>HTML</p>"),
            ],
            "missing title",
        ),
        (
            vec![("title", "Newsletter"), ("html_content", "<p>HTML</p>")],
            "missing text_content",
        ),
        (
            vec![("title", "Newsletter"), ("text_content", "Plain text")],
            "missing html_content",
        ),
    ];

    for (body, description) in test_cases {
        let response = app.post_newsletters(&body).await;

        assert_eq!(
            response.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "{description}" // Identify which missing field caused a failure.
        );
    }
}

#[tokio::test]
async fn publishing_requires_a_session_even_with_valid_basic_credentials() {
    let app = spawn_app().await; // Deliberately do not log in.
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // Unauthorized requests must not send newsletter emails.
        .mount(&app.email_server)
        .await;

    for use_basic_auth in [false, true] {
        let mut request = app
            .api_client
            .post(format!("{}/admin/newsletters", app.address))
            .form(&newsletter_form_data());

        if use_basic_auth {
            request = request.basic_auth(&app.test_user.username, Some(&app.test_user.password));
        }

        let response = request.send().await.unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()[reqwest::header::LOCATION],
            "/login" // Anonymous users must log in first.
        );
        assert!(
            response
                .headers()
                .get(reqwest::header::WWW_AUTHENTICATE)
                .is_none() // No longer advertise Basic authentication.
        );
    }
}

/// Use the public API of the application under test to create
/// an unconfirmed subscriber.
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let _mock_guard = Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "id": "mocked-email-id" })),
        )
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    // We can then reuse the same helper and just add
    // an extra step to actually call the confirmation link!
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[tokio::test]
async fn newsletters_rejects_json_bodies() {
    let app = spawn_app().await;
    app.login().await; // Reach the handler rather than the login redirect.

    let response = app
        .api_client
        .post(format!("{}/admin/newsletters", app.address))
        .json(&serde_json::json!({
            "title": "Newsletter",
            "text_content": "Plain text",
            "html_content": "<p>HTML</p>"
        })) // Correct fields, deliberately wrong body format.
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}
