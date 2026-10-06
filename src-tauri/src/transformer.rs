use serde::{Deserialize, Serialize};

use crate::http::{self, ApiError};

pub struct TransformRequest {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub user_message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transformed {
    pub text: String,
    pub cost_in_usd: Option<f64>,
}

pub trait Transformer: Send + Sync {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, ApiError>;
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
    fn transform(&self, request: TransformRequest) -> Result<Transformed, ApiError> {
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
            .post(http::endpoint(&request.base_url, "chat/completions"))
            .bearer_auth(&request.api_key)
            .json(&Body {
                model: &request.model,
                messages,
            })
            .send()?;
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
