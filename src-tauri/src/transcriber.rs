use reqwest::blocking::multipart::{Form, Part};
use serde::Deserialize;

use crate::http::{self, ApiError};

pub struct TranscribeRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub languages: Vec<String>,
    pub keywords: Vec<String>,
    pub audio: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transcription {
    pub text: String,
    pub cost_in_usd: Option<f64>,
}

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, ApiError>;
}

pub struct HttpTranscriber {
    client: reqwest::blocking::Client,
}

impl HttpTranscriber {
    pub fn new() -> Self {
        Self {
            client: http::client(),
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

impl Transcriber for HttpTranscriber {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, ApiError> {
        let file = Part::bytes(request.audio)
            .file_name("audio.flac")
            .mime_str("audio/flac")?;
        let mut form = Form::new()
            .text("model", request.model.clone())
            // LiteLLM treats every OpenAI model without "gpt-4o" in its name as Whisper and fills in
            // verbose_json when this is missing; gpt-transcribe rejects verbose_json.
            .text("response_format", "json")
            .part("file", file);
        // Proxies such as LiteLLM name models with a provider prefix, e.g. `openai/gpt-transcribe`.
        let model_name = request.model.rsplit('/').next().unwrap_or_default();
        let gpt_transcribe = model_name.starts_with("gpt-transcribe");
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
        let response = self
            .client
            .post(http::endpoint(&request.base_url, "audio/transcriptions"))
            .bearer_auth(request.api_key)
            .multipart(form)
            .send()?;
        let (body, cost) = http::body_and_cost_in_usd(response)?;
        serde_json::from_str::<Body>(&body)
            .map(|body| Transcription {
                text: body.text,
                cost_in_usd: cost,
            })
            .map_err(|error| ApiError::InvalidResponse(error.to_string()))
    }
}
