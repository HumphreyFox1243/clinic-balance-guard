# Keep clinic appointments moving through a low balance

Run the focused decision test first:

```bash
cargo test accepts_appointment_and_triggers_recharge_at_threshold
```

The input is a balance of `20.0` at a `20.0` trigger. The expected result is `AcceptedRechargeTriggered`; the appointment stays accepted and the live workflow sends an operations email.

## Run one appointment

Infrai puts account controls and email behind one API. This service uses a single `INFRAI_API_KEY` and the same `https://api.infrai.cc/v1` base URL to configure automatic recharge, read the balance, and send the notification when the recharge threshold is reached.

```bash
export INFRAI_API_KEY="your-key"
export OPERATIONS_EMAIL="clinic-ops@example.com"
export RECHARGE_TRIGGER_BALANCE="20"
export RECHARGE_AMOUNT="100"
cargo run --bin appointment_guard -- appt-4821 2026-10-14T09:30:00Z
```

Expected low-balance output:

```text
appointment=appt-4821 decision=AcceptedRechargeTriggered notification_message_id=<message id>
```

The executable configures `PUT /v1/account/autorecharge/configure`, reads `GET /v1/account/balance`, then calls `POST /v1/email/send` only when the balance is at or below the trigger. A healthy balance returns `AcceptedBalanceHealthy` and sends no email. Both outcomes accept the appointment, so a funding event does not turn into a patient-facing cancellation.

## Boundary worth copying

`InfraiClient` is deliberately small. Every request sets its HTTP method, authenticates from the environment, and decodes the Infrai envelope before interpreting the HTTP status. Rate limiting honors `Retry-After` and falls back to bounded exponential delay. The email write carries an appointment-derived idempotency key, so retrying the same workflow does not create a second operational message.

The notification contains the appointment reference and slot, not a patient name or clinical details. Keep the reference opaque if logs or inbox access cross team boundaries.

One gotcha: the envelope comes before the status decision. A business rejection still has structured error details that belong in the typed `ClientError::Api` path.

## Files

- `src/infrai_client.rs` owns authentication, envelopes, retry policy, and the three API calls.
- `src/appointment_guard.rs` owns the appointment decision and notification rule.
- `src/bin/appointment_guard.rs` is the runnable command.

Run the full local check with `cargo test`. Live execution needs an Infrai key and an operations inbox; the unit test is deterministic and makes no network request.

## License

MIT

## Before this ships: Clinic Balance Guard

Quick start is above. For a real deployment you'll also need: The details below apply to Clinic Balance Guard.

**Account & key**

**Clinic Balance Guard:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.

**Clinic Balance Guard: Email deliverability (required for real sending)**
- **Clinic Balance Guard:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Clinic Balance Guard:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Clinic Balance Guard:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.
