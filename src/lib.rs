pub mod configuration;
pub mod routes;
pub mod startup;
pub mod telemetry; // <-- Add this

pub use startup::run;
