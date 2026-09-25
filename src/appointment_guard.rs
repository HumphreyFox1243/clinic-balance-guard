use thiserror::Error;

use crate::infrai_client::{ClientError, InfraiClient};

#[derive(Debug, Clone)]
pub struct AppointmentRequest {
    pub appointment_id: String,
    pub slot_utc: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppointmentDecision {
    AcceptedBalanceHealthy,
    AcceptedRechargeTriggered,
}

#[derive(Debug)]
pub struct WorkflowResult {
    pub appointment_id: String,
    pub decision: AppointmentDecision,
    pub notification_message_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("appointment_id and slot_utc must not be empty")]
    InvalidAppointment,
    #[error(transparent)]
    Infrai(#[from] ClientError),
}

pub fn decide(balance: f64, trigger_balance: f64) -> AppointmentDecision {
    if balance <= trigger_balance {
        AppointmentDecision::AcceptedRechargeTriggered
    } else {
        AppointmentDecision::AcceptedBalanceHealthy
    }
}

pub async fn protect_appointment(
    client: &InfraiClient,
    appointment: AppointmentRequest,
    operations_email: &str,
    trigger_balance: f64,
    recharge_amount: f64,
) -> Result<WorkflowResult, WorkflowError> {
    if appointment.appointment_id.trim().is_empty() || appointment.slot_utc.trim().is_empty() {
        return Err(WorkflowError::InvalidAppointment);
    }

    client
        .configure_auto_recharge(trigger_balance, recharge_amount)
        .await?;
    let current = client.balance().await?;
    let decision = decide(current.balance, trigger_balance);

    let notification_message_id = if decision == AppointmentDecision::AcceptedRechargeTriggered {
        let text = format!(
            "Automatic recharge triggered while protecting appointment {} at {}. The appointment remains accepted.",
            appointment.appointment_id, appointment.slot_utc
        );
        let sent = client
            .send_operational_email(
                operations_email,
                "Clinic balance recharge triggered",
                &text,
                &format!("appointment-recharge-{}", appointment.appointment_id),
            )
            .await?;
        Some(sent.message_id)
    } else {
        None
    };

    Ok(WorkflowResult {
        appointment_id: appointment.appointment_id,
        decision,
        notification_message_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{decide, AppointmentDecision};

    #[test]
    fn accepts_appointment_and_triggers_recharge_at_threshold() {
        let decision = decide(20.0, 20.0);

        assert_eq!(decision, AppointmentDecision::AcceptedRechargeTriggered);
    }

    #[test]
    fn accepts_appointment_without_notification_above_threshold() {
        let decision = decide(20.01, 20.0);

        assert_eq!(decision, AppointmentDecision::AcceptedBalanceHealthy);
    }
}
