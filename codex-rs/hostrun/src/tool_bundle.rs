use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;

use codex_tool_api::FunctionToolSpec;
use codex_tool_api::ToolBundle;
use codex_tool_api::ToolCall;
use codex_tool_api::ToolError;
use codex_tool_api::ToolExecutor;
use codex_tool_api::ToolFuture;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

#[derive(Clone, Debug)]
pub struct HostrunToolConfig {
    runner: PathBuf,
}

impl HostrunToolConfig {
    pub fn new(runner: impl AsRef<Path>) -> Self {
        Self {
            runner: runner.as_ref().to_path_buf(),
        }
    }
}

pub fn hostrun_tool_bundle(config: HostrunToolConfig) -> ToolBundle {
    ToolBundle::new(
        hostrun_eval_spec(),
        Arc::new(HostrunToolExecutor { config }),
    )
}

fn hostrun_eval_spec() -> FunctionToolSpec {
    FunctionToolSpec {
        name: "hostrun_eval".to_string(),
        description: "Evaluate JavaScript in a Hostrun session.".to_string(),
        strict: true,
        parameters: json!({
            "type": "object",
            "properties": {
                "session_id": { "type": "string" },
                "code": { "type": "string" }
            },
            "required": ["session_id", "code"],
            "additionalProperties": false
        }),
    }
}

struct HostrunToolExecutor {
    config: HostrunToolConfig,
}

impl ToolExecutor for HostrunToolExecutor {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a> {
        Box::pin(async move {
            let input = parse_eval_arguments(&call.arguments)?;
            run_hostrun_eval(&self.config.runner, &input)
        })
    }
}

#[derive(Deserialize, serde::Serialize)]
struct HostrunEvalArguments {
    session_id: String,
    code: String,
}

fn parse_eval_arguments(arguments: &str) -> Result<HostrunEvalArguments, ToolError> {
    serde_json::from_str(arguments).map_err(|error| {
        ToolError::respond_to_model(format!("Invalid Hostrun eval arguments: {error}"))
    })
}

fn run_hostrun_eval(runner: &Path, input: &HostrunEvalArguments) -> Result<Value, ToolError> {
    let mut child = spawn_runner(runner)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ToolError::fatal("Hostrun runner stdin was unavailable"))?;
    let input_json = serde_json::to_vec(input)
        .map_err(|error| ToolError::fatal(format!("failed to encode Hostrun eval: {error}")))?;

    stdin
        .write_all(&input_json)
        .map_err(|error| ToolError::fatal(format!("failed to write Hostrun eval: {error}")))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .map_err(|error| ToolError::fatal(format!("failed to wait for Hostrun runner: {error}")))?;

    if !output.status.success() {
        return Err(runner_failure(output.stderr));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|error| ToolError::fatal(format!("Hostrun runner returned invalid JSON: {error}")))
}

fn spawn_runner(runner: &Path) -> Result<std::process::Child, ToolError> {
    Command::new(runner)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ToolError::fatal(format!("failed to start Hostrun runner: {error}")))
}

fn runner_failure(stderr: Vec<u8>) -> ToolError {
    let message = String::from_utf8_lossy(&stderr).trim().to_string();
    if message.is_empty() {
        return ToolError::respond_to_model("Hostrun runner failed");
    }

    ToolError::respond_to_model(message)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use codex_tool_api::ToolCall;
    use codex_tool_api::ToolError;
    use serde_json::json;
    use tempfile::TempDir;

    use super::HostrunToolConfig;
    use super::hostrun_tool_bundle;

    #[test]
    fn hostrun_eval_tool_spec_accepts_session_id_and_code() {
        let bundle = hostrun_tool_bundle(HostrunToolConfig::new("/bin/hostrun-runner"));

        assert_eq!(bundle.tool_name(), "hostrun_eval");
        assert!(bundle.spec().strict);
        assert_eq!(
            bundle.spec().parameters,
            json!({
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "code": { "type": "string" }
                },
                "required": ["session_id", "code"],
                "additionalProperties": false
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_arguments_return_model_visible_error() {
        let bundle = hostrun_tool_bundle(HostrunToolConfig::new("/bin/hostrun-runner"));
        let call = ToolCall {
            call_id: "call-1".to_string(),
            arguments: json!({ "code": "1 + 1" }).to_string(),
        };

        let error = bundle
            .executor()
            .execute(call)
            .await
            .expect_err("missing session id should fail");

        match error {
            ToolError::RespondToModel(message) => {
                assert!(message.contains("missing field `session_id`"));
            }
            ToolError::Fatal(message) => {
                panic!("expected model-visible error, got fatal error: {message}");
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_returns_structured_runner_json() {
        let temp_dir = TempDir::new().expect("temp dir");
        let runner = temp_dir.path().join("hostrun-runner");
        write_runner(
            &runner,
            r#"#!/bin/sh
read input
printf '%s\n' '{"type":"needs_approval","approval":{"id":"approval-1","tool":"rclone.deletefile","summary":"Delete probe","args":{"target":"spaces:bucket/probe.txt"}}}'
"#,
        );
        let bundle = hostrun_tool_bundle(HostrunToolConfig::new(&runner));
        let call = ToolCall {
            call_id: "call-1".to_string(),
            arguments: json!({
                "session_id": "session-1",
                "code": "tools.rclone.deletefile({ target: 'spaces:bucket/probe.txt' })"
            })
            .to_string(),
        };

        let output = bundle.executor().execute(call).await.expect("tool output");

        assert_eq!(
            output,
            json!({
                "type": "needs_approval",
                "approval": {
                    "id": "approval-1",
                    "tool": "rclone.deletefile",
                    "summary": "Delete probe",
                    "args": {
                        "target": "spaces:bucket/probe.txt"
                    }
                }
            })
        );
    }

    fn write_runner(path: &Path, content: &str) {
        fs::write(path, content).expect("write runner");
        let mut permissions = fs::metadata(path).expect("runner metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("runner executable");
    }
}
