use tokio::task::JoinError;
use zero2prod::{
    Application,
    configuration::get_configuration,
    issue_delivery_worker::run_worker_until_stopped,
    telemetry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = Application::build(configuration.clone()).await?;
    let application_task = tokio::spawn(application.run_until_stopped());
    let worker_task = tokio::spawn(run_worker_until_stopped(configuration));
    tokio::select! {
    o = application_task => report_exit("API", o),
    o = worker_task => report_exit("Background worker", o),
    };
    Ok(())
}

use std::fmt::{Debug, Display};
fn report_exit<E>(task_name: &str, outcome: Result<Result<(), E>, JoinError>)
where
    E: Debug + Display,
{
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name);
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            );
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}'s task failed to complete",
                task_name
            );
        }
    }
}
