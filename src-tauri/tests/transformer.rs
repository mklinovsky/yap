use mockito::Matcher;
use serde_json::json;
use yap_lib::http::ApiError;
use yap_lib::transformer::{
    HttpTransformer, TransformConnection, TransformError, TransformRequest, Transformer,
};

fn request(base_url: String) -> TransformRequest {
    TransformRequest {
        connection: TransformConnection::Api {
            base_url,
            api_key: "sk-test".into(),
        },
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
        TransformError::Api(ApiError::Http {
            status: 404,
            message: "The model `gpt-6` does not exist".into()
        })
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
        matches!(error, TransformError::Api(ApiError::InvalidResponse(_))),
        "got {error:?}"
    );
}

#[cfg(unix)]
mod claude {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};
    use yap_lib::claude_cli::ClaudeCliTransformer;
    use yap_lib::transformer::ProviderTransformer;

    struct Fixture {
        directory: tempfile::TempDir,
        executable: String,
    }

    impl Fixture {
        fn new(auth: &str, body: &str) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let executable = directory.path().join("claude");
            let script = format!(
                "#!/bin/sh\nfor arg do\n if [ \"$arg\" = auth ]; then\n  printf '%s\\n' '{}'\n  exit 0\n fi\ndone\n{}\n",
                auth, body,
            );
            std::fs::write(&executable, script).unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self {
                directory,
                executable: executable.to_str().unwrap().into(),
            }
        }
        fn subscribed(body: &str) -> Self {
            Self::new(
                r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"max"}"#,
                body,
            )
        }
        fn request(&self) -> TransformRequest {
            TransformRequest {
                connection: TransformConnection::Claude {
                    executable: Some(self.executable.clone()),
                },
                model: "sonnet".into(),
                system_prompt: "Fix grammar.".into(),
                user_message: "hello $(touch should-not-exist); 'world'\nsecond line".into(),
            }
        }
    }

    #[test]
    fn cli_receives_stdin_and_isolated_arguments_and_ignores_cost_estimate() {
        let fixture = Fixture::subscribed(
            r#"
            printf '%s\n' "$@" > "$(dirname "$0")/args"
            pwd > "$(dirname "$0")/cwd"
            cat > "$(dirname "$0")/input"
            printf '%s\n' '{"is_error":false,"subtype":"success","result":"Hello, world.","total_cost_usd":0.03}'
        "#,
        );
        let request = fixture.request();
        let expected_input = request.user_message.clone();
        let transformed = ProviderTransformer::default().transform(request).unwrap();
        assert_eq!(
            transformed,
            yap_lib::transformer::Transformed {
                text: "Hello, world.".into(),
                cost_in_usd: None
            }
        );
        assert_eq!(
            std::fs::read_to_string(fixture.directory.path().join("input")).unwrap(),
            expected_input
        );
        let args = std::fs::read_to_string(fixture.directory.path().join("args")).unwrap();
        assert!(args.contains("--tools\n\n"));
        assert!(args.contains("--setting-sources\n\n"));
        assert!(args.contains("--safe-mode\n"));
        assert!(args.contains("--model\nsonnet\n"));
        assert!(args.contains("--system-prompt\nFix grammar.\n"));
        assert!(args.contains("--strict-mcp-config\n"));
        assert!(args.contains("--no-session-persistence\n"));
        assert!(!args.contains("hello"));
        let cwd = std::fs::read_to_string(fixture.directory.path().join("cwd")).unwrap();
        assert!(cwd.contains("yap-claude-"));
        assert!(
            !std::path::Path::new(cwd.trim()).exists(),
            "temporary directory is cleaned up"
        );
        assert!(!fixture.directory.path().join("should-not-exist").exists());
    }

    #[test]
    fn missing_subscription_is_rejected_before_sending_a_transcript() {
        for authentication in [
            r#"{"loggedIn":false,"authMethod":"none","subscriptionType":null}"#,
            r#"{"loggedIn":true,"authMethod":"api_key","subscriptionType":"max"}"#,
            r#"{"loggedIn":true,"authMethod":"claude.ai","subscriptionType":"free"}"#,
        ] {
            let fixture = Fixture::new(authentication, "touch \"$(dirname \"$0\")/invoked\"");
            let error = ClaudeCliTransformer::default()
                .transform(fixture.request())
                .unwrap_err();
            assert!(error.to_string().contains("subscription login is required"));
            assert!(!fixture.directory.path().join("invoked").exists());
        }
    }

    #[test]
    fn cli_errors_invalid_json_and_blank_results_are_failures() {
        for (body, expected) in [
            (
                r#"echo '{"is_error":true,"subtype":"error_during_execution","result":"Usage limit reached"}'"#,
                "Usage limit reached",
            ),
            (
                r#"echo '{"is_error":false,"subtype":"success","result":"  "}'"#,
                "returned no text",
            ),
            ("echo invalid", "invalid JSON"),
            (
                "echo 'Unsupported option' >&2; exit 2",
                "Unsupported option",
            ),
            (
                r#"echo '{"is_error":false,"subtype":"success","result":"Do not paste"}'; exit 1"#,
                "Do not paste",
            ),
        ] {
            let fixture = Fixture::subscribed(body);
            let error = ClaudeCliTransformer::default()
                .transform(fixture.request())
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn hung_process_times_out_and_is_reaped() {
        let fixture = Fixture::subscribed("exec sleep 10");
        let started = Instant::now();
        let error = ClaudeCliTransformer::with_timeout(Duration::from_secs(2))
            .transform(fixture.request())
            .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn inherited_pipes_do_not_keep_a_finished_process_alive() {
        let fixture = Fixture::subscribed(
            r#"
            sleep 10 &
            cat >/dev/null
            echo '{"is_error":false,"subtype":"success","result":"OK"}'
        "#,
        );
        assert_eq!(
            ClaudeCliTransformer::with_timeout(Duration::from_secs(5))
                .transform(fixture.request())
                .unwrap()
                .text,
            "OK"
        );
    }

    #[test]
    fn excessive_output_is_bounded() {
        let fixture = Fixture::subscribed("head -c 1100000 /dev/zero");
        let error = ClaudeCliTransformer::default()
            .transform(fixture.request())
            .unwrap_err();
        assert!(error.to_string().contains("1 MiB"));
    }

    #[test]
    fn missing_executable_reports_setup_instructions() {
        let fixture = Fixture::subscribed("exit 0");
        let mut request = fixture.request();
        request.connection = TransformConnection::Claude {
            executable: Some("/missing/yap/claude".into()),
        };
        assert!(ClaudeCliTransformer::default()
            .transform(request)
            .unwrap_err()
            .to_string()
            .contains("absolute path"));
    }

    #[test]
    fn api_credentials_and_provider_overrides_are_not_inherited() {
        // Isolate environment changes in a separate test process, rather than mutating a
        // multithreaded test runner's environment.
        if std::env::var_os("YAP_CLI_ENV_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "claude::api_credentials_and_provider_overrides_are_not_inherited",
                ])
                .env("YAP_CLI_ENV_CHILD", "1")
                .env("ANTHROPIC_API_KEY", "must-not-be-used")
                .env("ANTHROPIC_AUTH_TOKEN", "must-not-be-used")
                .env("CLAUDE_CODE_USE_BEDROCK", "1")
                .env("CLAUDE_CONFIG_DIR", "/missing/config")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let fixture = Fixture::subscribed(
            r#"
            test -z "$ANTHROPIC_API_KEY$ANTHROPIC_AUTH_TOKEN$CLAUDE_CODE_USE_BEDROCK$CLAUDE_CONFIG_DIR" || exit 3
            cat >/dev/null
            echo '{"is_error":false,"subtype":"success","result":"OK"}'
        "#,
        );
        assert_eq!(
            ClaudeCliTransformer::default()
                .transform(fixture.request())
                .unwrap()
                .text,
            "OK"
        );
    }
    #[test]
    fn success_requires_complete_prompt_delivery() {
        let fixture = Fixture::subscribed(
            r#"echo '{"is_error":false,"subtype":"success","result":"Premature result"}'"#,
        );
        let mut request = fixture.request();
        request.user_message = "x".repeat(1024 * 1024);
        let error = ClaudeCliTransformer::default()
            .transform(request)
            .unwrap_err();
        assert!(error.to_string().contains("send transcript"));
    }
}
