//! One isolated, subscription-authenticated Claude Code invocation per transformation.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::transformer::{
    TransformConnection, TransformError, TransformRequest, Transformed, Transformer,
};

const OUTPUT_LIMIT: u64 = 1024 * 1024;

pub struct ClaudeCliTransformer {
    timeout: Duration,
}

impl Default for ClaudeCliTransformer {
    fn default() -> Self {
        Self::with_timeout(Duration::from_secs(60))
    }
}

impl ClaudeCliTransformer {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

fn cli_error(message: impl Into<String>) -> TransformError {
    TransformError::Cli(message.into())
}

fn resolve(executable: Option<&str>) -> Result<PathBuf, TransformError> {
    if let Some(path) = executable.filter(|path| !path.trim().is_empty()) {
        let path = PathBuf::from(path.trim());
        if !path.is_absolute() || !path.is_file() {
            return Err(cli_error("Claude executable must be an absolute path to an installed Claude Code executable."));
        }
        return Ok(path);
    }
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        for relative in [
            ".local/bin/claude",
            ".volta/bin/claude",
            ".volta/bin/bin/claude",
            ".npm-global/bin/claude",
        ] {
            candidates.push(PathBuf::from(&home).join(relative));
        }
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
    ]);
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join("claude")));
    }
    candidates.into_iter().find(|path| path.is_absolute() && path.is_file()).ok_or_else(|| {
        cli_error("Claude Code was not found. Install it, or set its absolute executable path in Transformations.")
    })
}

fn command(executable: &Path, directory: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Keep HOME/keychain access for the CLI's own login, but never inherit API/provider overrides.
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("ANTHROPIC_")
            || name.starts_with("CLAUDE_CODE_")
            || name == "CLAUDE_CONFIG_DIR"
        {
            command.env_remove(key);
        }
    }
    let mut paths = vec![executable
        .parent()
        .unwrap_or(Path::new("/usr/local/bin"))
        .to_path_buf()];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".volta/bin"));
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    command.env(
        "PATH",
        std::env::join_paths(paths).unwrap_or_else(|_| OsString::from("/usr/bin:/bin")),
    );
    command.args(["--safe-mode", "--setting-sources", ""]);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

struct Running {
    child: Child,
    stopped: bool,
}

impl Running {
    fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        // Kill the entire group, including a wrapper's descendants that still own the pipes.
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Output {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

enum PipeResult {
    Stdout(std::io::Result<Vec<u8>>),
    Stderr(std::io::Result<Vec<u8>>),
    Input(std::io::Result<()>),
}

fn read_pipe(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn run(mut command: Command, input: String, deadline: Instant) -> Result<Output, TransformError> {
    let mut running = Running {
        child: command.spawn().map_err(|error| {
            cli_error(format!(
                "Could not start Claude Code: {error}. Check the executable path."
            ))
        })?,
        stopped: false,
    };
    let stdout = running.child.stdout.take().expect("piped stdout");
    let stderr = running.child.stderr.take().expect("piped stderr");
    let mut stdin = running.child.stdin.take().expect("piped stdin");
    let (sender, receiver) = mpsc::channel();
    let out_sender = sender.clone();
    std::thread::spawn(move || {
        let _ = out_sender.send(PipeResult::Stdout(read_pipe(stdout)));
    });
    let err_sender = sender.clone();
    std::thread::spawn(move || {
        let _ = err_sender.send(PipeResult::Stderr(read_pipe(stderr)));
    });
    std::thread::spawn(move || {
        let _ = sender.send(PipeResult::Input(stdin.write_all(input.as_bytes())));
    });
    let mut out = None;
    let mut err = None;
    let mut status = None;
    let mut input_done = false;
    let mut input_error = None;
    while Instant::now() < deadline {
        if status.is_none() {
            status = running
                .child
                .try_wait()
                .map_err(|error| cli_error(format!("Could not wait for Claude Code: {error}")))?;
            if status.is_some() {
                running.stop();
            }
        }
        match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok(PipeResult::Stdout(result)) => out = Some(result),
            Ok(PipeResult::Stderr(result)) => err = Some(result),
            // On failure Claude can exit before reading stdin; report its output instead.
            Ok(PipeResult::Input(result)) => {
                input_done = true;
                input_error = result.err();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if [&out, &err]
            .iter()
            .any(|result| matches!(result, Some(Ok(bytes)) if bytes.len() as u64 > OUTPUT_LIMIT))
        {
            return Err(cli_error("Claude Code output exceeded the 1 MiB limit."));
        }
        if status.is_some() && out.is_some() && err.is_some() && input_done {
            break;
        }
    }
    let (Some(status), Some(out), Some(err), true) = (status, out, err, input_done) else {
        return Err(cli_error(
            "Claude Code timed out. Try again or choose a faster model.",
        ));
    };
    if status.success() {
        if let Some(error) = input_error {
            return Err(cli_error(format!(
                "Could not send transcript to Claude Code: {error}"
            )));
        }
    }
    let decode = |result: std::io::Result<Vec<u8>>| {
        let bytes = result
            .map_err(|error| cli_error(format!("Could not read Claude Code output: {error}")))?;
        String::from_utf8(bytes).map_err(|_| cli_error("Claude Code returned invalid UTF-8."))
    };
    Ok(Output {
        status,
        stdout: decode(out)?,
        stderr: decode(err)?,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Authentication {
    logged_in: bool,
    auth_method: String,
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct Reply {
    is_error: bool,
    subtype: String,
    result: Option<String>,
    #[serde(default)]
    errors: Vec<String>,
}

impl Transformer for ClaudeCliTransformer {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, TransformError> {
        let TransformConnection::Claude { executable } = &request.connection else {
            return Err(cli_error("Claude transformer requires a Claude connection"));
        };
        let executable = resolve(executable.as_deref())?;
        let directory = tempfile::Builder::new()
            .prefix("yap-claude-")
            .tempdir()
            .map_err(|error| {
                cli_error(format!(
                    "Could not create Claude working directory: {error}"
                ))
            })?;
        let deadline = Instant::now() + self.timeout;
        let mut auth = command(&executable, directory.path());
        auth.args(["auth", "status", "--json"]);
        let output = run(auth, String::new(), deadline)?;
        let login_message = "Claude Code subscription login is required. Update Claude Code if needed, then run claude auth login in Terminal and select your Claude subscription.";
        let authentication: Authentication =
            serde_json::from_str(&output.stdout).map_err(|_| cli_error(login_message))?;
        if !output.status.success()
            || !authentication.logged_in
            || authentication.auth_method != "claude.ai"
            || authentication
                .subscription_type
                .as_deref()
                .is_none_or(|plan| plan.is_empty() || plan == "free")
        {
            return Err(cli_error(login_message));
        }
        let mut process = command(&executable, directory.path());
        process.args([
            "-p",
            "--output-format",
            "json",
            "--tools",
            "",
            "--disallowedTools",
            "mcp__*",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--permission-mode",
            "dontAsk",
            "--no-session-persistence",
            "--disable-slash-commands",
        ]);
        if !request.model.trim().is_empty() {
            process.args(["--model", request.model.trim()]);
        }
        let system = if request.system_prompt.trim().is_empty() {
            "Transform the supplied text as requested. Return only the resulting text, without commentary."
        } else {
            &request.system_prompt
        };
        process.args(["--system-prompt", system]);
        let output = run(process, request.user_message, deadline)?;
        let reply = serde_json::from_str::<Reply>(&output.stdout);
        match reply {
            Ok(reply)
                if output.status.success() && !reply.is_error && reply.subtype == "success" =>
            {
                let text = reply.result.unwrap_or_default();
                if text.trim().is_empty() {
                    return Err(cli_error("Transformation returned no text"));
                }
                Ok(Transformed {
                    text,
                    cost_in_usd: None,
                })
            }
            Ok(reply) => {
                let message = reply.errors.into_iter().next().or(reply.result).filter(|message| !message.trim().is_empty()).unwrap_or_else(|| "Claude Code could not complete the transformation. Check your login, model and subscription usage limits.".into());
                Err(cli_error(message.chars().take(800).collect::<String>()))
            }
            Err(_) if !output.status.success() => Err(cli_error(format!(
                "Claude Code failed ({}): {}",
                output.status,
                output.stderr.trim().chars().take(800).collect::<String>()
            ))),
            Err(error) => Err(cli_error(format!(
                "Claude Code returned invalid JSON: {error}"
            ))),
        }
    }
}
