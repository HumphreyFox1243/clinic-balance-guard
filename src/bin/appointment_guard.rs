use std::{env, process::ExitCode};

use clinic_balance_guard::{
    appointment_guard::{protect_appointment, AppointmentRequest},
    infrai_client::InfraiClient,
};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("appointment guard failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let appointment_id = args
        .next()
        .ok_or("usage: appointment_guard <appointment_id> <slot_utc>")?;
    let slot_utc = args
        .next()
        .ok_or("usage: appointment_guard <appointment_id> <slot_utc>")?;
    let operations_email = env::var("OPERATIONS_EMAIL")?;
    let trigger_balance = env_f64("RECHARGE_TRIGGER_BALANCE", 20.0)?;
    let recharge_amount = env_f64("RECHARGE_AMOUNT", 100.0)?;

    let client = InfraiClient::from_env()?;
    let result = protect_appointment(
        &client,
        AppointmentRequest {
            appointment_id,
            slot_utc,
        },
        &operations_email,
        trigger_balance,
        recharge_amount,
    )
    .await?;

    println!(
        "appointment={} decision={:?} notification_message_id={}",
        result.appointment_id,
        result.decision,
        result.notification_message_id.as_deref().unwrap_or("none")
    );
    Ok(())
}

fn env_f64(name: &str, default: f64) -> Result<f64, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => Ok(value.parse()?),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error.into()),
    }
}
