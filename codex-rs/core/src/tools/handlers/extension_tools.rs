use std::time::Instant;

use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::ExecCommandSource;
use codex_tool_api::ToolBundle as ExtensionToolBundle;
use codex_tool_api::ToolError as ExtensionToolError;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::flat_tool_name;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;

pub(crate) struct BundledToolOutput {
    value: Value,
}

impl ToolOutput for BundledToolOutput {
    fn log_preview(&self) -> String {
        self.value.to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.value.to_string()),
                success: Some(true),
            },
        }
    }

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<Value> {
        Some(self.value.clone())
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        self.value.clone()
    }
}

pub(crate) struct BundledToolHandler {
    bundle: ExtensionToolBundle,
    spec: ToolSpec,
}

impl BundledToolHandler {
    pub(crate) fn new(bundle: ExtensionToolBundle, spec: ToolSpec) -> Self {
        Self { bundle, spec }
    }

    fn arguments_from_payload<'a>(&self, payload: &'a ToolPayload) -> Option<&'a str> {
        let ToolPayload::Function { arguments } = payload else {
            return None;
        };
        Some(arguments)
    }

    async fn execute_with_hostrun_events(
        &self,
        invocation: ToolInvocation,
        arguments: String,
    ) -> Result<Value, FunctionCallError> {
        let hostrun_display = hostrun_display_from_arguments(self.bundle.tool_name(), &arguments);
        let hostrun_emitter = hostrun_display
            .as_ref()
            .map(|display| hostrun_emitter(display, invocation.turn.cwd.clone()));
        let event_ctx = ToolEventCtx::new(
            invocation.session.as_ref(),
            invocation.turn.as_ref(),
            &invocation.call_id,
            None,
        );
        emit_hostrun_begin(hostrun_emitter.as_ref(), event_ctx).await;

        let started_at = Instant::now();
        let result = self
            .execute_bundle(invocation.call_id.clone(), arguments)
            .await;

        let value = match result {
            Ok(value) => value,
            Err(error) => {
                emit_hostrun_failure(hostrun_emitter.as_ref(), event_ctx, &error).await;
                return Err(map_extension_tool_error(error));
            }
        };

        emit_hostrun_success(
            hostrun_display.as_ref(),
            hostrun_emitter.as_ref(),
            event_ctx,
            &value,
            started_at,
        )
        .await;
        Ok(value)
    }

    async fn execute_bundle(
        &self,
        call_id: String,
        arguments: String,
    ) -> Result<Value, ExtensionToolError> {
        self.bundle
            .executor()
            .execute(codex_tool_api::ToolCall { call_id, arguments })
            .await
    }
}

impl ToolHandler for BundledToolHandler {
    type Output = BundledToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(self.bundle.tool_name())
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(self.spec.clone())
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        self.arguments_from_payload(payload).is_some()
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let arguments = self.arguments_from_payload(&invocation.payload)?;
        Some(PreToolUsePayload {
            tool_name: HookToolName::new(flat_tool_name(&self.tool_name()).into_owned()),
            tool_input: extension_tool_hook_input(arguments),
        })
    }

    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &Self::Output,
    ) -> Option<PostToolUsePayload> {
        let arguments = self.arguments_from_payload(&invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: HookToolName::new(flat_tool_name(&self.tool_name()).into_owned()),
            tool_use_id: invocation.call_id.clone(),
            tool_input: extension_tool_hook_input(arguments),
            tool_response: result
                .post_tool_use_response(&invocation.call_id, &invocation.payload)?,
        })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let arguments = self
            .arguments_from_payload(&invocation.payload)
            .ok_or_else(|| {
                FunctionCallError::Fatal(format!(
                    "tool {} invoked with incompatible payload",
                    self.bundle.tool_name()
                ))
            })?
            .to_string();

        let value = self
            .execute_with_hostrun_events(invocation, arguments)
            .await?;
        Ok(BundledToolOutput { value })
    }
}

#[derive(Clone)]
struct HostrunDisplay {
    code: String,
}

fn hostrun_display_from_arguments(tool_name: &str, arguments: &str) -> Option<HostrunDisplay> {
    if tool_name != "hostrun_eval" {
        return None;
    }

    let value: Value = serde_json::from_str(arguments).ok()?;
    let code = value.get("code")?.as_str()?.to_string();
    Some(HostrunDisplay { code })
}

fn hostrun_emitter(
    display: &HostrunDisplay,
    cwd: codex_utils_absolute_path::AbsolutePathBuf,
) -> ToolEmitter {
    ToolEmitter::unified_exec(
        &[
            "hostrun".to_string(),
            "eval".to_string(),
            display.code.clone(),
        ],
        cwd,
        ExecCommandSource::Agent,
        None,
    )
}

async fn emit_hostrun_begin(emitter: Option<&ToolEmitter>, event_ctx: ToolEventCtx<'_>) {
    if let Some(emitter) = emitter {
        emitter.begin(event_ctx).await;
    }
}

async fn emit_hostrun_failure(
    emitter: Option<&ToolEmitter>,
    event_ctx: ToolEventCtx<'_>,
    error: &ExtensionToolError,
) {
    if let Some(emitter) = emitter {
        emitter
            .emit(
                event_ctx,
                ToolEventStage::Failure(ToolEventFailure::Message(error.to_string())),
            )
            .await;
    }
}

async fn emit_hostrun_success(
    display: Option<&HostrunDisplay>,
    emitter: Option<&ToolEmitter>,
    event_ctx: ToolEventCtx<'_>,
    value: &Value,
    started_at: Instant,
) {
    let (Some(display), Some(emitter)) = (display, emitter) else {
        return;
    };
    let output = hostrun_exec_output(display, value, started_at);
    emitter
        .emit(
            event_ctx,
            ToolEventStage::Success {
                output,
                applied_patch_delta: None,
            },
        )
        .await;
}

fn hostrun_exec_output(
    _display: &HostrunDisplay,
    value: &Value,
    started_at: Instant,
) -> ExecToolCallOutput {
    let output = hostrun_output_text(value);
    ExecToolCallOutput {
        exit_code: 0,
        stdout: StreamOutput::new(output.clone()),
        stderr: StreamOutput::new(String::new()),
        aggregated_output: StreamOutput::new(output),
        duration: started_at.elapsed(),
        timed_out: false,
    }
}

fn hostrun_output_text(value: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(console) = value.get("console").and_then(Value::as_array) {
        for entry in console {
            if let Some(message) = entry.get("message").and_then(Value::as_str) {
                lines.push(message.to_string());
            }
        }
    }
    if lines.is_empty() {
        if let Some(result) = value.get("value") {
            if !result.is_null() {
                lines.push(result.to_string());
            }
        }
    }
    lines.join("\n")
}

pub(crate) fn extension_tool_spec(
    spec: &codex_tool_api::FunctionToolSpec,
) -> Result<ToolSpec, serde_json::Error> {
    Ok(ToolSpec::Function(ResponsesApiTool {
        name: spec.name.clone(),
        description: spec.description.clone(),
        strict: spec.strict,
        defer_loading: None,
        parameters: codex_tools::parse_tool_input_schema(&spec.parameters)?,
        output_schema: None,
    }))
}

fn map_extension_tool_error(error: ExtensionToolError) -> FunctionCallError {
    match error {
        ExtensionToolError::RespondToModel(message) => FunctionCallError::RespondToModel(message),
        ExtensionToolError::Fatal(message) => FunctionCallError::Fatal(message),
    }
}

fn extension_tool_hook_input(arguments: &str) -> Value {
    if arguments.trim().is_empty() {
        return Value::Object(serde_json::Map::new());
    }

    serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::BundledToolHandler;
    use super::BundledToolOutput;
    use super::extension_tool_spec;
    use crate::tools::context::ToolCallSource;
    use crate::tools::context::ToolInvocation;
    use crate::tools::context::ToolPayload;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::tools::registry::ToolHandler;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::protocol::EventMsg;

    struct StubExtensionExecutor;

    impl codex_tool_api::ToolExecutor for StubExtensionExecutor {
        fn execute(&self, _call: codex_tool_api::ToolCall) -> codex_tool_api::ToolFuture<'_> {
            Box::pin(async { Ok(json!({ "ok": true })) })
        }
    }

    struct StubHostrunExecutor;

    impl codex_tool_api::ToolExecutor for StubHostrunExecutor {
        fn execute(&self, _call: codex_tool_api::ToolCall) -> codex_tool_api::ToolFuture<'_> {
            Box::pin(async {
                Ok(json!({
                    "type": "completed",
                    "executed": "console.log('hello')",
                    "console": [
                        { "level": "log", "message": "hello" }
                    ],
                    "value": null
                }))
            })
        }
    }

    #[tokio::test]
    async fn exposes_generic_hook_payloads_and_is_conservatively_mutating() {
        let bundle = codex_tool_api::ToolBundle::new(
            codex_tool_api::FunctionToolSpec {
                name: "extension_echo".to_string(),
                description: "Echoes arguments.".to_string(),
                strict: true,
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                    },
                    "required": ["message"],
                    "additionalProperties": false,
                }),
            },
            Arc::new(StubExtensionExecutor),
        );
        let spec = extension_tool_spec(bundle.spec()).expect("extension spec should convert");
        let handler = BundledToolHandler::new(bundle, spec);
        let (session, turn) = crate::session::tests::make_session_and_context().await;
        let invocation = ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-extension".to_string(),
            tool_name: codex_tools::ToolName::plain("extension_echo"),
            source: ToolCallSource::Direct,
            pre_tool_use_approved: false,
            pre_tool_use_approval_required: false,
            payload: ToolPayload::Function {
                arguments: json!({ "message": "hello" }).to_string(),
            },
        };
        let output = BundledToolOutput {
            value: json!({ "ok": true }),
        };

        assert!(ToolHandler::is_mutating(&handler, &invocation).await);
        assert_eq!(
            ToolHandler::pre_tool_use_payload(&handler, &invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_input: json!({ "message": "hello" }),
            })
        );
        assert_eq!(
            ToolHandler::post_tool_use_payload(&handler, &invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("extension_echo"),
                tool_use_id: "call-extension".to_string(),
                tool_input: json!({ "message": "hello" }),
                tool_response: json!({ "ok": true }),
            })
        );
    }

    #[tokio::test]
    async fn hostrun_extension_tool_emits_exec_events_with_code() {
        let bundle = codex_tool_api::ToolBundle::new(
            codex_tool_api::FunctionToolSpec {
                name: "hostrun_eval".to_string(),
                description: "Evaluate Hostrun code.".to_string(),
                strict: true,
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string" },
                    },
                    "required": ["code"],
                    "additionalProperties": false,
                }),
            },
            Arc::new(StubHostrunExecutor),
        );
        let spec = extension_tool_spec(bundle.spec()).expect("extension spec should convert");
        let handler = BundledToolHandler::new(bundle, spec);
        let (session, turn, rx) = crate::session::tests::make_session_and_context_with_rx().await;
        let invocation = ToolInvocation {
            session,
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call_id: "call-hostrun".to_string(),
            tool_name: codex_tools::ToolName::plain("hostrun_eval"),
            source: ToolCallSource::Direct,
            pre_tool_use_approved: false,
            pre_tool_use_approval_required: false,
            payload: ToolPayload::Function {
                arguments: json!({ "code": "console.log('hello')" }).to_string(),
            },
        };

        let output = handler.handle(invocation).await.expect("hostrun output");

        assert_eq!(output.value["console"][0]["message"], json!("hello"));
        let begin = rx.recv().await.expect("begin event");
        match begin.msg {
            EventMsg::ExecCommandBegin(event) => {
                assert_eq!(
                    event.command,
                    vec![
                        "hostrun".to_string(),
                        "eval".to_string(),
                        "console.log('hello')".to_string()
                    ]
                );
            }
            other => panic!("expected hostrun exec begin, got {other:?}"),
        }
        let end = rx.recv().await.expect("end event");
        match end.msg {
            EventMsg::ExecCommandEnd(event) => {
                assert_eq!(event.command[2], "console.log('hello')");
                assert_eq!(event.aggregated_output, "hello");
                assert_eq!(event.exit_code, 0);
            }
            other => panic!("expected hostrun exec end, got {other:?}"),
        }
    }
}
