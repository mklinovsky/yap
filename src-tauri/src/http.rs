use std::time::Duration;

use reqwest::blocking::{Client, Response};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("network error: {0}")]
    Network(String),
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::Network(error.to_string())
    }
}

pub(crate) fn client() -> Client {
    // reqwest's 30 s default is far too short to upload and transcribe a max-length recording;
    // the connect timeout keeps an unreachable endpoint from hanging for that long.
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(15 * 60))
        .build()
        .expect("HTTP client")
}

pub(crate) fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{path}", base_url.trim_end_matches('/'))
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

pub(crate) fn body_and_cost_in_usd(response: Response) -> Result<(String, Option<f64>), ApiError> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        let message = serde_json::from_str::<ErrorBody>(&body)
            .map(|body| body.error.message)
            .unwrap_or(body);
        return Err(ApiError::Http {
            status: status.as_u16(),
            message,
        });
    }
    let cost = response
        .headers()
        .get("x-litellm-response-cost")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse().ok());
    Ok((response.text()?, cost))
}
