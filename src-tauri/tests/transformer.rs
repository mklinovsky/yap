use mockito::Matcher;
use serde_json::json;
use yap_lib::http::ApiError;
use yap_lib::transformer::{HttpTransformer, TransformRequest, Transformer};

fn request(base_url: String) -> TransformRequest {
    TransformRequest {
        base_url,
        api_key: "sk-test".into(),
        model: "gpt-6-luna".into(),
        system_prompt: "Fix grammar.".into(),
        user_message: "hello world".into(),
    }
}

fn reply(content: serde_json::Value) -> String {
    json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string()
}

#[test]
fn sends_system_prompt_and_user_message_and_returns_the_answer() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-test")
        .match_body(Matcher::Json(json!({
            "model": "gpt-6-luna",
            "messages": [
                {"role": "system", "content": "Fix grammar."},
                {"role": "user", "content": "hello world"}
            ]
        })))
        .with_header("content-type", "application/json")
        .with_body(reply(json!("Hello, world.")))
        .create();

    let transformed = HttpTransformer::new()
        .transform(request(server.url()))
        .unwrap();

    mock.assert();
    assert_eq!(
        (transformed.text.as_str(), transformed.cost_in_usd),
        ("Hello, world.", None)
    );
}

#[test]
fn empty_system_prompt_sends_only_the_user_message() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/chat/completions")
        .match_body(Matcher::Json(json!({
            "model": "gpt-6-luna",
            "messages": [{"role": "user", "content": "hello world"}]
        })))
        .with_body(reply(json!("ok")))
        .create();

    HttpTransformer::new()
        .transform(TransformRequest {
            system_prompt: String::new(),
            ..request(server.url())
        })
        .unwrap();

    mock.assert();
}

#[test]
fn trailing_slash_on_base_url_is_ignored() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/chat/completions")
        .with_body(reply(json!("ok")))
        .create();

    HttpTransformer::new()
        .transform(request(format!("{}/", server.url())))
        .unwrap();

    mock.assert();
}

#[test]
fn cost_comes_from_the_litellm_header() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/chat/completions")
        .with_header("x-litellm-response-cost", "0.00071")
        .with_body(reply(json!("ok")))
        .create();

    let transformed = HttpTransformer::new()
        .transform(request(server.url()))
        .unwrap();

    assert_eq!(transformed.cost_in_usd, Some(0.00071));
}

#[test]
fn null_content_is_returned_as_empty_text() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/chat/completions")
        .with_body(reply(serde_json::Value::Null))
        .create();

    let transformed = HttpTransformer::new()
        .transform(request(server.url()))
        .unwrap();

    assert_eq!(transformed.text, "");
}

#[test]
fn http_error_carries_status_and_provider_message() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/chat/completions")
        .with_status(404)
        .with_body(r#"{"error":{"message":"The model `gpt-6` does not exist"}}"#)
        .create();

    let error = HttpTransformer::new()
        .transform(request(server.url()))
        .unwrap_err();

    assert_eq!(
        error,
        ApiError::Http {
            status: 404,
            message: "The model `gpt-6` does not exist".into()
        }
    );
}

#[test]
fn response_without_choices_is_invalid() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/chat/completions")
        .with_body(r#"{"choices":[]}"#)
        .create();

    let error = HttpTransformer::new()
        .transform(request(server.url()))
        .unwrap_err();

    assert!(
        matches!(error, ApiError::InvalidResponse(_)),
        "got {error:?}"
    );
}
