use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "mocked-email-id"
        })))
        .mount(&app.email_server)
        .await;

    // Act
    app.post_subscriptions(body.into()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Ensure we don't call random APIs on the web
    assert_eq!(confirmation_links.html.host_str().unwrap(), "127.0.0.1");
    // Act
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    let app = spawn_app().await;
    let email = "ursula_le_guin@gmail.com";

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "mocked-email-id"
        })))
        .mount(&app.email_server)
        .await;

    let response = app
        .post_subscriptions("name=le%20guin&email=ursula_le_guin%40gmail.com".into())
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let status: String = sqlx::query_scalar("SELECT status FROM subscriptions WHERE email = $1")
        .bind(email)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(status, "pending confirmation");

    let requests = app.email_server.received_requests().await.unwrap();
    let links = app.get_confirmation_links(&requests[0]);
    let response = reqwest::get(links.html).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);

    let status: String = sqlx::query_scalar("SELECT status FROM subscriptions WHERE email = $1")
        .bind(email)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(status, "confirmed");
}

#[tokio::test]
async fn rejects_a_confirmation_with_a_token_that_does_not_exist() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = reqwest::get(&format!(
        "{}/subscriptions/confirm?subscription_token=mytoken",
        app.address
    ))
    .await
    .unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 401);
}
