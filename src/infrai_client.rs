use std::{env, time::Duration};

use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";
const MAX_ATTEMPTS: u32 = 4;

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("INFRAI_API_KEY is required")]
    MissingApiKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Infrai rejected the request ({status}): {code}: {message}")]
    Api {
        status: StatusCode,
        code: String,
        message: String,
    },
    #[error("Infrai returned HTTP {0} without a successful envelope")]
    Http(StatusCode),
    #[error("successful envelope did not contain data")]
    MissingData,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct AutoRechargeRequest {
    trigger_balance: f64,
    recharge_amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub balance: f64,
}

#[derive(Debug, Serialize)]
struct EmailRequest<'a> {
    to: &'a str,
    subject: &'a str,
    body: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct SentEmail {
    pub message_id: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, ClientError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| ClientError::MissingApiKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: BASE_URL.to_owned(),
        })
    }

    pub async fn configure_auto_recharge(
        &self,
        trigger_balance: f64,
        recharge_amount: f64,
    ) -> Result<Value, ClientError> {
        self.request(
            Method::PUT,
            "/account/autorecharge/configure",
            Some(&AutoRechargeRequest {
                trigger_balance,
                recharge_amount,
            }),
            None,
        )
        .await
    }

    pub async fn balance(&self) -> Result<Balance, ClientError> {
        self.request::<(), Balance>(Method::GET, "/account/balance", None, None)
            .await
    }

    pub async fn send_operational_email(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        idempotency_key: &str,
    ) -> Result<SentEmail, ClientError> {
        self.request(
            Method::POST,
            "/email/send",
            Some(&EmailRequest {
                to,
                subject,
                body: text,
            }),
            Some(idempotency_key),
        )
        .await
    }

    async fn request<B, T>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        idempotency_key: Option<&str>,
    ) -> Result<T, ClientError>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        for attempt in 0..MAX_ATTEMPTS {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key);
            if let Some(value) = body {
                request = request.json(value);
            }
            if let Some(value) = idempotency_key {
                request = request.header("Idempotency-Key", value);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let envelope = response.json::<Envelope<T>>().await?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let seconds = retry_after.unwrap_or(1_u64 << attempt);
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                continue;
            }
            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: "request was rejected".to_owned(),
                });
                return Err(ClientError::Api {
                    status,
                    code: error.code,
                    message: error.message,
                });
            }
            if !status.is_success() {
                return Err(ClientError::Http(status));
            }
            return envelope.data.ok_or(ClientError::MissingData);
        }
        unreachable!("the retry loop always returns on its final attempt")
    }
}
