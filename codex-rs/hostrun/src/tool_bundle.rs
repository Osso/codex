use std::sync::Arc;
use std::sync::Mutex;

use codex_tool_api::FunctionToolSpec;
use codex_tool_api::ToolBundle;
use codex_tool_api::ToolCall;
use codex_tool_api::ToolError;
use codex_tool_api::ToolExecutor;
use codex_tool_api::ToolFuture;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

use crate::HostrunSessionStore;

#[derive(Clone, Debug, Default)]
pub struct HostrunToolConfig;

impl HostrunToolConfig {
    pub fn new(_runner: impl AsRef<std::path::Path>) -> Self {
        Self
    }
}

pub fn hostrun_tool_bundle(_config: HostrunToolConfig) -> ToolBundle {
    ToolBundle::new(
        hostrun_eval_spec(),
        Arc::new(HostrunToolExecutor {
            sessions: Mutex::new(HostrunSessionStore::new_auto_approve()),
        }),
    )
}

pub fn embedded_hostrun_tool_bundle() -> ToolBundle {
    hostrun_tool_bundle(HostrunToolConfig)
}

fn hostrun_eval_spec() -> FunctionToolSpec {
    FunctionToolSpec {
        name: "hostrun_eval".to_string(),
        description: concat!(
            "Evaluate JavaScript in a persistent Hostrun QuickJS session. ",
            "Use the contributed Hostrun instructions for available globals and host APIs."
        )
        .to_string(),
        strict: true,
        parameters: json!({
            "type": "object",
            "properties": {
                "code": { "type": "string" }
            },
            "required": ["code"],
            "additionalProperties": false
        }),
    }
}

struct HostrunToolExecutor {
    sessions: Mutex<HostrunSessionStore>,
}

impl ToolExecutor for HostrunToolExecutor {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a> {
        Box::pin(async move {
            let input = parse_eval_arguments(&call.arguments)?;
            self.run_eval(&input)
        })
    }
}

impl HostrunToolExecutor {
    fn run_eval(&self, input: &HostrunEvalArguments) -> Result<Value, ToolError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| ToolError::fatal("Hostrun session lock was poisoned"))?;
        let result = sessions
            .eval(
                input.session_id.as_deref().unwrap_or("default"),
                &input.code,
            )
            .map_err(|error| ToolError::respond_to_model(error.to_string()))?;

        serde_json::to_value(result)
            .map_err(|error| ToolError::fatal(format!("failed to encode Hostrun eval: {error}")))
    }
}

#[derive(Deserialize, serde::Serialize)]
struct HostrunEvalArguments {
    session_id: Option<String>,
    code: String,
}

fn parse_eval_arguments(arguments: &str) -> Result<HostrunEvalArguments, ToolError> {
    serde_json::from_str(arguments)
        .map_err(|error| ToolError::respond_to_model(format!("invalid Hostrun arguments: {error}")))
}

#[cfg(test)]
mod tests {
    use codex_tool_api::ToolCall;
    use codex_tool_api::ToolError;
    use serde_json::json;

    use super::HostrunToolConfig;
    use super::embedded_hostrun_tool_bundle;
    use super::hostrun_tool_bundle;

    #[test]
    fn hostrun_eval_tool_spec_accepts_session_id_and_code() {
        let bundle = embedded_hostrun_tool_bundle();

        assert_eq!(bundle.tool_name(), "hostrun_eval");
        assert!(bundle.spec().strict);
        assert_eq!(
            bundle.spec().parameters,
            json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string" }
                },
                "required": ["code"],
                "additionalProperties": false
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_code_returns_model_visible_error() {
        let bundle = embedded_hostrun_tool_bundle();
        let call = ToolCall {
            call_id: "call-1".to_string(),
            arguments: json!({ "session_id": "session-1" }).to_string(),
        };

        let error = bundle
            .executor()
            .execute(call)
            .await
            .expect_err("missing code should fail");

        match error {
            ToolError::RespondToModel(message) => {
                assert!(message.contains("missing field `code`"));
            }
            ToolError::Fatal(message) => {
                panic!("expected model-visible error, got fatal error: {message}");
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_returns_quickjs_eval_json() {
        let bundle = embedded_hostrun_tool_bundle();

        let output = bundle
            .executor()
            .execute(call("session-1", "ctx.count = 41; ctx.count + 1;"))
            .await
            .expect("tool output");

        assert_eq!(output["type"], json!("completed"));
        assert_eq!(output["executed"], json!("ctx.count = 41; ctx.count + 1;"));
        assert_eq!(output["value"], json!(42));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_defaults_missing_session_id_to_thread_session() {
        let bundle = embedded_hostrun_tool_bundle();
        let first = ToolCall {
            call_id: "call-1".to_string(),
            arguments: json!({ "code": "ctx.count = 7; ctx.count;" }).to_string(),
        };
        let second = ToolCall {
            call_id: "call-2".to_string(),
            arguments: json!({ "code": "ctx.count += 1; ctx.count;" }).to_string(),
        };

        let first_output = bundle.executor().execute(first).await.expect("first eval");
        let second_output = bundle
            .executor()
            .execute(second)
            .await
            .expect("second eval");

        assert_eq!(first_output["value"], json!(7));
        assert_eq!(second_output["value"], json!(8));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_returns_structured_approval_json() {
        let bundle = embedded_hostrun_tool_bundle();

        let output = bundle
            .executor()
            .execute(call(
                "session-1",
                "rclone.deletefile('spaces:bucket/probe.txt')",
            ))
            .await
            .expect("tool output");

        assert_eq!(
            output,
            json!({
                "type": "needs_approval",
                "executed": "",
                "value": null,
                "approval": {
                    "id": "rclone.deletefile:spaces:bucket/probe.txt",
                    "tool": "rclone.deletefile",
                    "summary": "Delete spaces:bucket/probe.txt",
                    "args": {
                        "target": "spaces:bucket/probe.txt"
                    }
                }
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_runs_approved_cli_command() {
        let bundle = embedded_hostrun_tool_bundle();

        let output = bundle
            .executor()
            .execute(call(
                "session-1",
                "cli.printf('hello').stdout.text().run();",
            ))
            .await
            .expect("tool output");

        assert_eq!(
            output["value"],
            json!({
                "program": "printf",
                "args": ["hello"],
                "exitCode": 0,
                "success": true,
                "stdout": "hello",
                "stdoutMeta": {
                    "bytes": 5,
                    "capturedBytes": 5,
                    "truncated": false
                }
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_runs_approved_fs_helpers() {
        let bundle = embedded_hostrun_tool_bundle();
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tool-fs.txt");
        let path_text = path.to_string_lossy().to_string();

        let output = bundle
            .executor()
            .execute(call(
                "session-1",
                &format!(
                    "fs.write({}, 'tool fs'); fs.read({});",
                    json!(path_text),
                    json!(path_text)
                ),
            ))
            .await
            .expect("tool output");

        assert_eq!(output["value"], json!("tool fs"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn executor_reuses_one_quickjs_session_across_calls() {
        let bundle = embedded_hostrun_tool_bundle();

        let first = bundle
            .executor()
            .execute(call("session-1", "ctx.count = 1; ctx.count;"))
            .await
            .expect("first eval");
        let second = bundle
            .executor()
            .execute(call("session-1", "ctx.count += 1; ctx.count;"))
            .await
            .expect("second eval");

        assert_eq!(first["value"], json!(1));
        assert_eq!(second["value"], json!(2));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn legacy_runner_config_is_ignored_by_embedded_runtime() {
        let bundle = hostrun_tool_bundle(HostrunToolConfig::new("/missing/runner.js"));

        let output = bundle
            .executor()
            .execute(call("session-1", "1 + 1"))
            .await
            .expect("embedded runtime does not spawn runner");

        assert_eq!(output["value"], json!(2));
    }

    fn call(session_id: &str, code: &str) -> ToolCall {
        ToolCall {
            call_id: "call-1".to_string(),
            arguments: json!({
                "session_id": session_id,
                "code": code
            })
            .to_string(),
        }
    }
}
