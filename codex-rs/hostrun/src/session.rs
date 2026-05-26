use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

use rquickjs::Context;
use rquickjs::Ctx;
use rquickjs::Function;
use rquickjs::Runtime;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

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
}

impl HostrunSession {
    pub fn new() -> Result<Self, HostrunSessionError> {
        let runtime = Runtime::new().map_err(HostrunSessionError::from_quickjs)?;
        let context = Context::full(&runtime).map_err(HostrunSessionError::from_quickjs)?;
        let session = Self {
            _runtime: runtime,
            context,
        };
        session.initialize_context()?;
        Ok(session)
    }

    pub fn eval(&self, code: &str) -> Result<HostrunEvalResult, HostrunSessionError> {
        self.context
            .with(|ctx| self.eval_in_context(ctx, code))
            .map_err(HostrunSessionError::from_quickjs)?
    }

    fn initialize_context(&self) -> Result<(), HostrunSessionError> {
        let invoker = Arc::new(HostCapabilityInvoker);
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

#[derive(Default)]
struct HostCapabilityInvoker;

impl HostCapabilityInvoker {
    fn invoke_tool(&self, tool_path: &str, args_json: &str) -> String {
        let args = serde_json::from_str(args_json).unwrap_or(Value::Null);
        match tool_path {
            "fs.write" => pending_approval(fs_write_approval(args)),
            "fs.read" => pending_approval(fs_path_approval("fs.read", "Read", args)),
            "fs.exists" => {
                pending_approval(fs_path_approval("fs.exists", "Check existence of", args))
            }
            "fs.remove" => pending_approval(fs_path_approval("fs.remove", "Remove", args)),
            "rclone.deletefile" => pending_approval(rclone_deletefile_approval(args)),
            "http.request" => pending_approval(http_request_approval(args)),
            tool if tool.starts_with("cli.") => pending_approval(cli_command_approval(tool, args)),
            _ => json!({
                "type": "denied",
                "reason": format!("Hostrun capability is unavailable: {tool_path}")
            })
            .to_string(),
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

fn fs_write_approval(args: Value) -> HostrunApprovalRequest {
    let path = field_as_string(&args, "path");
    let content = field_as_string(&args, "content");
    HostrunApprovalRequest {
        id: format!("fs.write:{path}"),
        tool: "fs.write".to_string(),
        summary: format!("Write {} bytes to {path}", content.len()),
        args,
    }
}

fn fs_path_approval(tool: &str, verb: &str, args: Value) -> HostrunApprovalRequest {
    let path = field_as_string(&args, "path");
    HostrunApprovalRequest {
        id: format!("{tool}:{path}"),
        tool: tool.to_string(),
        summary: format!("{verb} {path}"),
        args,
    }
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

fn http_request_approval(args: Value) -> HostrunApprovalRequest {
    let method = field_as_string(&args, "method");
    let url = field_as_string(&args, "url");
    HostrunApprovalRequest {
        id: format!("http.request:{}:{url}", method.to_uppercase()),
        tool: "http.request".to_string(),
        summary: format!("HTTP {} {url}", method.to_uppercase()),
        args: redact_http_auth(args),
    }
}

fn redact_http_auth(mut args: Value) -> Value {
    redact_http_auth_field(&mut args);
    redact_http_headers(&mut args);
    args
}

fn redact_http_auth_field(args: &mut Value) {
    let Some(auth) = args.get_mut("auth") else {
        return;
    };
    match auth {
        Value::Object(auth) => {
            for key in ["bearer", "token"] {
                redact_object_key(auth, key);
            }
            if let Some(basic) = auth.get_mut("basic") {
                redact_http_basic_auth(basic);
            }
        }
        other => {
            *other = Value::String("<redacted>".to_string());
        }
    }
}

fn redact_http_basic_auth(basic: &mut Value) {
    match basic {
        Value::Object(basic) => redact_object_key(basic, "password"),
        other => *other = Value::String("<redacted>".to_string()),
    }
}

fn redact_http_headers(args: &mut Value) {
    let Some(Value::Object(headers)) = args.get_mut("headers") else {
        return;
    };
    for (key, value) in headers {
        if is_sensitive_http_header(key) {
            *value = Value::String("<redacted>".to_string());
        }
    }
}

fn is_sensitive_http_header(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "x-api-key" | "x-auth-token"
    )
}

fn redact_object_key(object: &mut serde_json::Map<String, Value>, key: &str) {
    if object.contains_key(key) {
        object.insert(key.to_string(), Value::String("<redacted>".to_string()));
    }
}

fn field_as_string(args: &Value, field: &str) -> String {
    args.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub struct HostrunSessionStore {
    sessions: HashMap<String, HostrunSession>,
}

impl HostrunSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn eval(
        &mut self,
        session_id: &str,
        code: &str,
    ) -> Result<HostrunEvalResult, HostrunSessionError> {
        let session = match self.sessions.entry(session_id.to_string()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(HostrunSession::new()?),
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
