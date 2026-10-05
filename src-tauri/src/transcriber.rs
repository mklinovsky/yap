use reqwest::blocking::multipart::{Form, Part};
use serde::Deserialize;

pub struct TranscribeRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub languages: Vec<String>,
    pub keywords: Vec<String>,
    pub audio: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TranscribeError {
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("network error: {0}")]
    Network(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transcription {
    pub text: String,
    /// USD, when the endpoint reports it.
    pub cost: Option<f64>,
}

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, TranscribeError>;
}

pub struct HttpTranscriber {
    client: reqwest::blocking::Client,
}

impl HttpTranscriber {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Default for HttpTranscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct Body {
    text: String,
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

impl Transcriber for HttpTranscriber {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, TranscribeError> {
        let file = Part::bytes(request.audio)
            .file_name("audio.flac")
            .mime_str("audio/flac")
            .map_err(network)?;
        let mut form = Form::new()
            .text("model", request.model.clone())
            // LiteLLM treats every OpenAI model without "gpt-4o" in its name as Whisper and fills in
            // verbose_json when this is missing; gpt-transcribe rejects verbose_json.
            .text("response_format", "json")
            .part("file", file);
        let gpt_transcribe = request.model.starts_with("gpt-transcribe");
        if gpt_transcribe {
            // gpt-transcribe replaced `language` with `languages[]`; OpenAI says never to send both.
            for language in request.languages {
                form = form.text("languages[]", language);
            }
            for keyword in request.keywords {
                form = form.text("keywords[]", keyword);
            }
        } else if let Some(language) = request.languages.into_iter().next() {
            form = form.text("language", language);
        }
        let url = format!(
            "{}/audio/transcriptions",
            request.base_url.trim_end_matches('/')
        );
        let response = self
            .client
            .post(url)
            .bearer_auth(request.api_key)
            .multipart(form)
            .send()
            .map_err(network)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            let message = serde_json::from_str::<ErrorBody>(&body)
                .map(|body| body.error.message)
                .unwrap_or_else(|_| shorten(&body));
            return Err(TranscribeError::Http {
                status: status.as_u16(),
                message,
            });
        }
        let cost = response
            .headers()
            .get("x-litellm-response-cost")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok());
        let body = response.text().map_err(network)?;
        serde_json::from_str::<Body>(&body)
            .map(|body| Transcription {
                text: body.text,
                cost,
            })
            .map_err(|error| TranscribeError::InvalidResponse(error.to_string()))
    }
}

// Proxies answer errors with whole HTML pages, which would end up in the tray menu.
fn shorten(body: &str) -> String {
    const LIMIT: usize = 200;
    match body.char_indices().nth(LIMIT) {
        Some((end, _)) => format!("{}…", &body[..end]),
        None => body.to_string(),
    }
}

fn network(error: reqwest::Error) -> TranscribeError {
    TranscribeError::Network(error.to_string())
}
