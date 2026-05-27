use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;

use rquickjs::Context;
use rquickjs::Ctx;
use rquickjs::Function;
use rquickjs::Runtime;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::fs_capability::{execute_fs_operation, fs_approval};
use crate::http_capability::{execute_http_request, http_request_approval};
use crate::output_intent::apply_output_intent;
use crate::tmp_capability::{remove_tmp_resource, tmp_resources};

const APPROVAL_REQUIRED_PREFIX: &str = "__HOSTRUN_APPROVAL_REQUIRED__:";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostrunEvalResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub executed: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub console: Vec<HostrunConsoleMessage>,
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<HostrunApprovalRequest>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostrunConsoleMessage {
    pub level: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HostrunApprovalRequest {
    pub id: String,
    pub tool: String,
    pub summary: String,
    pub args: Value,
}

pub struct HostrunSession {
    _runtime: Runtime,
    context: Context,
    capability_mode: HostCapabilityMode,
}

impl HostrunSession {
    pub fn new() -> Result<Self, HostrunSessionError> {
        Self::new_with_capability_mode(HostCapabilityMode::PendingApproval)
    }

    pub fn new_auto_approve() -> Result<Self, HostrunSessionError> {
        Self::new_with_capability_mode(HostCapabilityMode::AutoApprove)
    }

    fn new_with_capability_mode(
        capability_mode: HostCapabilityMode,
    ) -> Result<Self, HostrunSessionError> {
        let runtime = Runtime::new().map_err(HostrunSessionError::from_quickjs)?;
        let context = Context::full(&runtime).map_err(HostrunSessionError::from_quickjs)?;
        let session = Self {
            _runtime: runtime,
            context,
            capability_mode,
        };
        session.initialize_context(capability_mode)?;
        Ok(session)
    }

    pub fn eval(&self, code: &str) -> Result<HostrunEvalResult, HostrunSessionError> {
        self.context
            .with(|ctx| self.eval_in_context(ctx, code))
            .map_err(HostrunSessionError::from_quickjs)?
    }

    fn initialize_context(
        &self,
        capability_mode: HostCapabilityMode,
    ) -> Result<(), HostrunSessionError> {
        let invoker = Arc::new(HostCapabilityInvoker { capability_mode });
        self.context
            .with(|ctx| {
                let globals = ctx.globals();
                let invoke_tool =
                    Function::new(ctx.clone(), move |tool_path: String, args_json: String| {
                        invoker.invoke_tool(&tool_path, &args_json)
                    })?;
                globals.set("__hostrun_invokeTool", invoke_tool)?;
                ctx.eval::<(), _>(HOSTRUN_BOOTSTRAP)
            })
            .map_err(HostrunSessionError::from_quickjs)
    }

    fn eval_in_context(
        &self,
        ctx: Ctx<'_>,
        code: &str,
    ) -> Result<Result<HostrunEvalResult, HostrunSessionError>, rquickjs::Error> {
        let globals = ctx.globals();
        globals.set("__hostrun_evalCode", code)?;
        let output =
            ctx.eval::<String, _>("globalThis.__hostrun_run(globalThis.__hostrun_evalCode)")?;
        globals.set("__hostrun_evalCode", rquickjs::Null)?;
        Ok(parse_eval_output(&output))
    }
}

impl Drop for HostrunSession {
    fn drop(&mut self) {
        if !matches!(self.capability_mode, HostCapabilityMode::AutoApprove) {
            return;
        }
        for resource in tmp_resources(&self.context) {
            remove_tmp_resource(&resource.path);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HostrunSessionError {
    #[error("{0}")]
    Eval(String),
}

impl HostrunSessionError {
    fn from_quickjs(error: rquickjs::Error) -> Self {
        Self::Eval(error.to_string())
    }
}

#[derive(Clone, Copy)]
enum HostCapabilityMode {
    PendingApproval,
    AutoApprove,
}

struct HostCapabilityInvoker {
    capability_mode: HostCapabilityMode,
}

impl HostCapabilityInvoker {
    fn invoke_tool(&self, tool_path: &str, args_json: &str) -> String {
        let args = serde_json::from_str(args_json).unwrap_or(Value::Null);
        match tool_path {
            "fs.write" | "fs.read" | "fs.exists" | "fs.remove" => {
                self.invoke_fs_operation(tool_path, args)
            }
            "rclone.deletefile" => pending_approval(rclone_deletefile_approval(args)),
            "http.request" => self.invoke_http_request(args),
            tool if tool.starts_with("cli.") => self.invoke_cli_command(tool, args),
            _ => json!({
                "type": "denied",
                "reason": format!("Hostrun capability is unavailable: {tool_path}")
            })
            .to_string(),
        }
    }

    fn invoke_fs_operation(&self, tool_path: &str, args: Value) -> String {
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => pending_approval(fs_approval(tool_path, args)),
            HostCapabilityMode::AutoApprove => match execute_fs_operation(tool_path, args) {
                Ok(value) => completed(value),
                Err(error) => denied(error.to_string()),
            },
        }
    }

    fn invoke_cli_command(&self, tool_path: &str, args: Value) -> String {
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => {
                pending_approval(cli_command_approval(tool_path, args))
            }
            HostCapabilityMode::AutoApprove => match execute_cli_command(tool_path, args) {
                Ok(value) => completed(value),
                Err(error) => denied(error.to_string()),
            },
        }
    }

    fn invoke_http_request(&self, args: Value) -> String {
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => pending_approval(http_request_approval(args)),
            HostCapabilityMode::AutoApprove => match execute_http_request(args) {
                Ok(value) => completed(value),
                Err(error) => denied(error.to_string()),
            },
        }
    }
}

fn parse_eval_output(output: &str) -> Result<HostrunEvalResult, HostrunSessionError> {
    if let Some(approval_json) = output.strip_prefix(APPROVAL_REQUIRED_PREFIX) {
        let approval = serde_json::from_str(approval_json).map_err(|error| {
            HostrunSessionError::Eval(format!("invalid approval JSON: {error}"))
        })?;
        return Ok(HostrunEvalResult {
            result_type: "needs_approval".to_string(),
            executed: String::new(),
            console: Vec::new(),
            value: None,
            approval: Some(approval),
        });
    }

    serde_json::from_str(output)
        .map_err(|error| HostrunSessionError::Eval(format!("invalid Hostrun result JSON: {error}")))
}

fn pending_approval(approval: HostrunApprovalRequest) -> String {
    json!({
        "type": "needs_approval",
        "approval": approval
    })
    .to_string()
}

fn completed(value: Value) -> String {
    json!({
        "type": "completed",
        "value": value
    })
    .to_string()
}

fn denied(reason: String) -> String {
    json!({
        "type": "denied",
        "reason": reason
    })
    .to_string()
}

fn rclone_deletefile_approval(args: Value) -> HostrunApprovalRequest {
    let target = field_as_string(&args, "target");
    HostrunApprovalRequest {
        id: format!("rclone.deletefile:{target}"),
        tool: "rclone.deletefile".to_string(),
        summary: format!("Delete {target}"),
        args,
    }
}

fn cli_command_approval(tool_path: &str, args: Value) -> HostrunApprovalRequest {
    let program = tool_path.trim_start_matches("cli.");
    let (cli_args, io) = split_cli_command_payload(args);
    let command = cli_command_summary(program, &cli_args);
    HostrunApprovalRequest {
        id: format!("cli.{program}:{command}"),
        tool: format!("cli.{program}"),
        summary: format!("Run {command}"),
        args: cli_command_args(program, cli_args, io),
    }
}

fn split_cli_command_payload(args: Value) -> (Vec<Value>, Option<Value>) {
    match args {
        Value::Array(args) => (args, None),
        Value::Object(mut payload) if payload.contains_key("args") => {
            let cli_args = match payload.remove("args").unwrap_or(Value::Null) {
                Value::Array(args) => args,
                Value::Null => Vec::new(),
                other => vec![other],
            };
            if payload.is_empty() {
                (cli_args, None)
            } else {
                (cli_args, Some(Value::Object(payload)))
            }
        }
        Value::Null => (Vec::new(), None),
        other => (vec![other], None),
    }
}

fn cli_command_args(program: &str, args: Vec<Value>, io: Option<Value>) -> Value {
    let mut payload = json!({
        "program": program,
        "args": args,
    });
    if let (Value::Object(payload), Some(Value::Object(io))) = (&mut payload, io) {
        payload.extend(io);
    }
    payload
}

fn cli_command_summary(program: &str, args: &[Value]) -> String {
    let mut parts = vec![program.to_string()];
    parts.extend(args.iter().map(cli_arg_summary));
    parts.join(" ")
}

fn cli_arg_summary(arg: &Value) -> String {
    match arg {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn execute_cli_command(tool_path: &str, args: Value) -> Result<Value, HostrunSessionError> {
    let program = tool_path.trim_start_matches("cli.");
    let (cli_args, io) = split_cli_command_payload(args);
    let payload = cli_command_args(program, cli_args, io);
    let Value::Object(payload) = merge_cli_payload(program, payload, tool_path)? else {
        unreachable!("cli command payload is always an object");
    };
    let argv = cli_payload_args(&payload)?;
    let output = run_cli_process(program, &argv, payload.get("stdin"))?;
    cli_execution_result(program, &argv, &payload, output)
}

fn merge_cli_payload(
    program: &str,
    fallback: Value,
    tool_path: &str,
) -> Result<Value, HostrunSessionError> {
    let Value::Object(mut payload) = fallback else {
        return Err(HostrunSessionError::Eval(format!(
            "invalid payload for {tool_path}"
        )));
    };
    payload
        .entry("program".to_string())
        .or_insert_with(|| Value::String(program.to_string()));
    payload
        .entry("args".to_string())
        .or_insert_with(|| json!([]));
    Ok(Value::Object(payload))
}

fn cli_payload_args(
    payload: &serde_json::Map<String, Value>,
) -> Result<Vec<String>, HostrunSessionError> {
    let values = payload
        .get("args")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    values.iter().map(cli_arg_to_string).collect()
}

fn cli_arg_to_string(value: &Value) -> Result<String, HostrunSessionError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        Value::Array(_) | Value::Object(_) => Err(HostrunSessionError::Eval(format!(
            "cli arguments must be scalar argv values, got {value}"
        ))),
    }
}

struct CliProcessOutput {
    exit_code: Option<i32>,
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_cli_process(
    program: &str,
    argv: &[String],
    stdin: Option<&Value>,
) -> Result<CliProcessOutput, HostrunSessionError> {
    let mut child = Command::new(program)
        .args(argv)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to start {program}: {error}"))
        })?;

    if let Some(stdin) = stdin {
        let input = stdin_bytes(stdin)?;
        let child_stdin = child.stdin.as_mut().ok_or_else(|| {
            HostrunSessionError::Eval(format!("failed to open stdin for {program}"))
        })?;
        child_stdin.write_all(&input).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to write stdin for {program}: {error}"))
        })?;
    }

    let output = child.wait_with_output().map_err(|error| {
        HostrunSessionError::Eval(format!("failed to wait for {program}: {error}"))
    })?;
    Ok(CliProcessOutput {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn stdin_bytes(stdin: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    let stdin_type = stdin
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("stream");
    match stdin_type {
        "text" => Ok(field_as_string(stdin, "text").into_bytes()),
        "file" => fs::read(field_as_string(stdin, "path")).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to read stdin file: {error}"))
        }),
        "json" => serialize_json_stdin(stdin.get("value").unwrap_or(&Value::Null)),
        "yaml" => serialize_yaml_stdin(stdin.get("value").unwrap_or(&Value::Null)),
        "csv" => serialize_delimited_rows(stdin.get("rows"), ","),
        "tsv" => serialize_delimited_rows(stdin.get("rows"), "\t"),
        "jsonLines" => serialize_json_lines(stdin.get("values")),
        "lines" => serialize_lines(stdin.get("lines")),
        "stream" => stream_stdin_bytes(stdin.get("source")),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin source type: {other}"
        ))),
    }
}

fn stream_stdin_bytes(source: Option<&Value>) -> Result<Vec<u8>, HostrunSessionError> {
    let source = source
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream source is required".to_string()))?;
    let stream = source
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or("stdout");
    let command = source
        .get("command")
        .ok_or_else(|| HostrunSessionError::Eval("stdin stream command is required".to_string()))?;
    let Value::Object(command) = command else {
        return Err(HostrunSessionError::Eval(
            "stdin stream command must be an object".to_string(),
        ));
    };
    let program = command
        .get("program")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            HostrunSessionError::Eval("stdin stream command program is required".to_string())
        })?;
    let argv = cli_payload_args(command)?;
    let output = run_cli_process(program, &argv, None)?;
    match stream {
        "stdout" => Ok(output.stdout),
        "stderr" => Ok(output.stderr),
        other => Err(HostrunSessionError::Eval(format!(
            "unsupported stdin stream source: {other}"
        ))),
    }
}

fn serialize_json_stdin(value: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    serde_json::to_vec(value)
        .map(|mut bytes| {
            bytes.push(b'\n');
            bytes
        })
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize JSON stdin: {error}"))
        })
}

fn serialize_yaml_stdin(value: &Value) -> Result<Vec<u8>, HostrunSessionError> {
    serde_yaml::to_string(value)
        .map(String::into_bytes)
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize YAML stdin: {error}"))
        })
}

fn serialize_delimited_rows(
    rows: Option<&Value>,
    delimiter: &str,
) -> Result<Vec<u8>, HostrunSessionError> {
    let rows = rows
        .and_then(Value::as_array)
        .ok_or_else(|| HostrunSessionError::Eval("stdin rows must be an array".to_string()))?;
    let lines = rows
        .iter()
        .map(|row| {
            row.as_array()
                .ok_or_else(|| HostrunSessionError::Eval("stdin row must be an array".to_string()))
                .map(|cells| {
                    cells
                        .iter()
                        .map(stdin_cell_text)
                        .collect::<Vec<_>>()
                        .join(delimiter)
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((lines.join("\n") + "\n").into_bytes())
}

fn stdin_cell_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn serialize_json_lines(values: Option<&Value>) -> Result<Vec<u8>, HostrunSessionError> {
    let values = values.and_then(Value::as_array).ok_or_else(|| {
        HostrunSessionError::Eval("stdin JSON lines must be an array".to_string())
    })?;
    let lines = values
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            HostrunSessionError::Eval(format!("failed to serialize JSONL stdin: {error}"))
        })?;
    Ok((lines.join("\n") + "\n").into_bytes())
}

fn serialize_lines(lines: Option<&Value>) -> Result<Vec<u8>, HostrunSessionError> {
    let lines = lines
        .and_then(Value::as_array)
        .ok_or_else(|| HostrunSessionError::Eval("stdin lines must be an array".to_string()))?;
    let text = lines
        .iter()
        .map(stdin_cell_text)
        .collect::<Vec<_>>()
        .join("\n");
    Ok((text + "\n").into_bytes())
}

fn cli_execution_result(
    program: &str,
    argv: &[String],
    payload: &serde_json::Map<String, Value>,
    output: CliProcessOutput,
) -> Result<Value, HostrunSessionError> {
    let mut result = serde_json::Map::new();
    result.insert("program".to_string(), Value::String(program.to_string()));
    result.insert("args".to_string(), json!(argv));
    result.insert("exitCode".to_string(), json!(output.exit_code));
    result.insert("success".to_string(), Value::Bool(output.success));
    let stderr_to_stdout = output_intent_type(payload.get("stderr")) == Some("stdout");
    let stdout = if stderr_to_stdout {
        let mut bytes = output.stdout.clone();
        bytes.extend_from_slice(&output.stderr);
        bytes
    } else {
        output.stdout.clone()
    };
    apply_output_intent(&mut result, "stdout", payload.get("stdout"), &stdout)?;
    if !stderr_to_stdout {
        apply_output_intent(&mut result, "stderr", payload.get("stderr"), &output.stderr)?;
    }
    if let Some(combined) = payload.get("combined") {
        let mut bytes = output.stdout.clone();
        bytes.extend_from_slice(&output.stderr);
        apply_output_intent(&mut result, "combined", Some(combined), &bytes)?;
    }
    Ok(Value::Object(result))
}

fn output_intent_type(intent: Option<&Value>) -> Option<&str> {
    intent
        .and_then(|intent| intent.get("type"))
        .and_then(Value::as_str)
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub struct HostrunSessionStore {
    sessions: HashMap<String, HostrunSession>,
    capability_mode: HostCapabilityMode,
}

impl HostrunSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            capability_mode: HostCapabilityMode::PendingApproval,
        }
    }

    pub fn new_auto_approve() -> Self {
        Self {
            sessions: HashMap::new(),
            capability_mode: HostCapabilityMode::AutoApprove,
        }
    }

    pub fn eval(
        &mut self,
        session_id: &str,
        code: &str,
    ) -> Result<HostrunEvalResult, HostrunSessionError> {
        let session = match self.sessions.entry(session_id.to_string()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(HostrunSession::new_with_capability_mode(
                self.capability_mode,
            )?),
        };
        session.eval(code)
    }
}

impl Default for HostrunSessionStore {
    fn default() -> Self {
        Self::new()
    }
}

const HOSTRUN_BOOTSTRAP: &str = include_str!("bootstrap.js");

#[cfg(test)]
#[path = "session_tests.rs"]
mod session_tests;

#[cfg(test)]
#[path = "projection_tests.rs"]
mod projection_tests;

#[cfg(test)]
#[path = "collection_tests.rs"]
mod collection_tests;

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;

#[cfg(test)]
#[path = "path_tests.rs"]
mod path_tests;

#[cfg(test)]
#[path = "byte_tests.rs"]
mod byte_tests;

#[cfg(test)]
#[path = "structured_write_tests.rs"]
mod structured_write_tests;

#[cfg(test)]
#[path = "structured_data_tests.rs"]
mod structured_data_tests;

#[cfg(test)]
#[path = "command_execution_tests.rs"]
mod command_execution_tests;

#[cfg(test)]
#[path = "fs_execution_tests.rs"]
mod fs_execution_tests;

#[cfg(test)]
#[path = "http_execution_tests.rs"]
mod http_execution_tests;

#[cfg(test)]
#[path = "rg_execution_tests.rs"]
mod rg_execution_tests;

#[cfg(test)]
#[path = "tmp_tests.rs"]
mod tmp_tests;
