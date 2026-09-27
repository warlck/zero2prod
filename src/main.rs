use zero2prod::{
    Application,
    configuration::get_configuration,
    telemetry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");

    let email_client = configuration.email_client.client();

    let application = Application::build(configuration, email_client).await?;
    application.run_until_stopped().await?;

    Ok(())
}
