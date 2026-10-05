use std::sync::{Arc, Mutex};

use mockito::Matcher;
use yap_lib::transcriber::{HttpTranscriber, TranscribeError, TranscribeRequest, Transcriber};

fn request(base_url: String) -> TranscribeRequest {
    TranscribeRequest {
        base_url,
        api_key: "sk-test".into(),
        model: "whisper-1".into(),
        languages: vec![],
        keywords: vec![],
        audio: b"fLaC-fake".to_vec(),
    }
}

#[test]
fn returns_text_transcribed_by_the_endpoint() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/audio/transcriptions")
        .match_header("authorization", "Bearer sk-test")
        .match_body(Matcher::AllOf(vec![
            Matcher::Regex(r#"name="model"\r\n\r\nwhisper-1\r\n"#.into()),
            Matcher::Regex(
                r#"name="file"; filename="audio.flac"\r\nContent-Type: audio/flac"#.into(),
            ),
        ]))
        .with_header("content-type", "application/json")
        .with_body(r#"{"text":"hello world"}"#)
        .create();

    let transcription = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap();

    mock.assert();
    assert_eq!(transcription.text, "hello world");
}

#[test]
fn sends_configured_language() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/audio/transcriptions")
        .match_body(Matcher::Regex(r#"name="language"\r\n\r\nsk\r\n"#.into()))
        .with_body(r#"{"text":"ahoj"}"#)
        .create();

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            languages: vec!["sk".into()],
            ..request(server.url())
        })
        .unwrap();

    mock.assert();
}

#[test]
fn http_error_carries_status_and_provider_message() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/audio/transcriptions")
        .with_status(401)
        .with_body(r#"{"error":{"message":"Incorrect API key provided"}}"#)
        .create();

    let error = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap_err();

    assert_eq!(
        error,
        TranscribeError::Http {
            status: 401,
            message: "Incorrect API key provided".into(),
        }
    );
}

#[test]
fn base_url_with_trailing_slash_hits_the_same_endpoint() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/audio/transcriptions")
        .with_body(r#"{"text":"ok"}"#)
        .create();

    HttpTranscriber::new()
        .transcribe(request(format!("{}/", server.url())))
        .unwrap();

    mock.assert();
}

#[test]
fn success_without_transcription_text_is_an_invalid_response() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/audio/transcriptions")
        .with_body("<html>proxy login</html>")
        .create();

    let error = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap_err();

    assert!(
        matches!(error, TranscribeError::InvalidResponse(_)),
        "{error:?}"
    );
}

fn capture_body(server: &mut mockito::Server) -> Arc<Mutex<String>> {
    let body = Arc::new(Mutex::new(String::new()));
    let sink = body.clone();
    server
        .mock("POST", "/audio/transcriptions")
        .with_body_from_request(move |request| {
            *sink.lock().unwrap() = String::from_utf8_lossy(request.body().unwrap()).into_owned();
            br#"{"text":"ok"}"#.to_vec()
        })
        .create();
    body
}

#[test]
fn gpt_transcribe_receives_language_as_languages_array() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "gpt-transcribe".into(),
            languages: vec!["sk".into()],
            ..request(server.url())
        })
        .unwrap();

    let body = body.lock().unwrap();
    assert_eq!(
        (
            body.contains("name=\"languages[]\"\r\n\r\nsk\r\n"),
            body.contains("name=\"language\""),
        ),
        (true, false)
    );
}

#[test]
fn gpt_transcribe_receives_each_keyword() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "gpt-transcribe".into(),
            keywords: vec!["Tauri".into(), "rusqlite".into()],
            ..request(server.url())
        })
        .unwrap();

    let body = body.lock().unwrap();
    assert_eq!(
        (
            body.contains("name=\"keywords[]\"\r\n\r\nTauri\r\n"),
            body.contains("name=\"keywords[]\"\r\n\r\nrusqlite\r\n"),
        ),
        (true, true)
    );
}

#[test]
fn other_models_do_not_receive_keywords() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "whisper-1".into(),
            keywords: vec!["Tauri".into()],
            ..request(server.url())
        })
        .unwrap();

    assert!(!body.lock().unwrap().contains("keywords[]"));
}

#[test]
fn gpt_transcribe_receives_every_selected_language() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "gpt-transcribe".into(),
            languages: vec!["sk".into(), "en".into()],
            ..request(server.url())
        })
        .unwrap();

    let body = body.lock().unwrap();
    assert_eq!(
        (
            body.contains("name=\"languages[]\"\r\n\r\nsk\r\n"),
            body.contains("name=\"languages[]\"\r\n\r\nen\r\n"),
        ),
        (true, true)
    );
}

#[test]
fn single_language_models_receive_only_the_first_language() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "whisper-1".into(),
            languages: vec!["sk".into(), "en".into()],
            ..request(server.url())
        })
        .unwrap();

    let body = body.lock().unwrap();
    assert_eq!(
        (
            body.matches("name=\"language\"").count(),
            body.contains("name=\"language\"\r\n\r\nsk\r\n"),
        ),
        (1, true)
    );
}

#[test]
fn requests_plain_json_response_format() {
    let mut server = mockito::Server::new();
    let body = capture_body(&mut server);

    HttpTranscriber::new()
        .transcribe(TranscribeRequest {
            model: "gpt-transcribe".into(),
            ..request(server.url())
        })
        .unwrap();

    assert!(body
        .lock()
        .unwrap()
        .contains("name=\"response_format\"\r\n\r\njson\r\n"));
}

#[test]
fn reports_cost_from_litellm_header() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/audio/transcriptions")
        .with_header("x-litellm-response-cost", "0.00123")
        .with_body(r#"{"text":"hello"}"#)
        .create();

    let transcription = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap();

    assert_eq!(transcription.cost, Some(0.00123));
}

#[test]
fn cost_is_unknown_without_litellm_header() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/audio/transcriptions")
        .with_body(r#"{"text":"hello"}"#)
        .create();

    let transcription = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap();

    assert_eq!(transcription.cost, None);
}

#[test]
fn unparsable_cost_header_is_ignored() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/audio/transcriptions")
        .with_header("x-litellm-response-cost", "n/a")
        .with_body(r#"{"text":"hello"}"#)
        .create();

    let transcription = HttpTranscriber::new()
        .transcribe(request(server.url()))
        .unwrap();

    assert_eq!(transcription.text, "hello");
    assert_eq!(transcription.cost, None);
}
