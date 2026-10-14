use serde::{Deserialize, Serialize};

use crate::http::{self, ApiError};

pub enum TransformConnection {
    Api { base_url: String, api_key: String },
    Claude { executable: Option<String> },
}

pub struct TransformRequest {
    pub connection: TransformConnection,
    pub model: String,
    pub system_prompt: String,
    pub user_message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transformed {
    pub text: String,
    pub cost_in_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransformError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("{0}")]
    Cli(String),
}

#[derive(Default)]
pub struct ProviderTransformer {
    http: HttpTransformer,
    claude: crate::claude_cli::ClaudeCliTransformer,
}

impl Transformer for ProviderTransformer {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, TransformError> {
        match &request.connection {
            TransformConnection::Api { .. } => self.http.transform(request),
            TransformConnection::Claude { .. } => self.claude.transform(request),
        }
    }
}

pub trait Transformer: Send + Sync {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, TransformError>;
}

pub struct HttpTransformer {
    client: reqwest::blocking::Client,
}

impl HttpTransformer {
    pub fn new() -> Self {
        Self {
            client: http::client(),
        }
    }
}

impl Default for HttpTransformer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct Body<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
}

#[derive(Deserialize)]
struct Reply {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ReplyMessage,
}

#[derive(Deserialize)]
struct ReplyMessage {
    content: Option<String>,
}

impl Transformer for HttpTransformer {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, TransformError> {
        let TransformConnection::Api { base_url, api_key } = &request.connection else {
            return Err(TransformError::Cli(
                "HTTP transformer requires an API connection".into(),
            ));
        };
        let mut messages = Vec::new();
        if !request.system_prompt.trim().is_empty() {
            messages.push(Message {
                role: "system",
                content: &request.system_prompt,
            });
        }
        messages.push(Message {
            role: "user",
            content: &request.user_message,
        });
        let response = self
            .client
            .post(http::endpoint(base_url, "chat/completions"))
            .bearer_auth(api_key)
            .json(&Body {
                model: &request.model,
                messages,
            })
            .send()
            .map_err(ApiError::from)?;
        let (body, cost) = http::body_and_cost_in_usd(response)?;
        let reply = serde_json::from_str::<Reply>(&body)
            .map_err(|error| ApiError::InvalidResponse(error.to_string()))?;
        let choice = reply
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::InvalidResponse("no choices".into()))?;
        Ok(Transformed {
            text: choice.message.content.unwrap_or_default(),
            cost_in_usd: cost,
        })
    }
}
