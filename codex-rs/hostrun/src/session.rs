use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use rquickjs::Context;
use rquickjs::Ctx;
use rquickjs::Function;
use rquickjs::Runtime;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::cli_approval;
use crate::cli_execution;
use crate::cli_payload;
use crate::fs_capability::execute_fs_operation;
use crate::fs_capability::fs_approval;
use crate::http_capability::execute_http_request;
use crate::http_capability::http_request_approval;
use crate::process_registry::ManagedProcess;
use crate::process_registry::ProcessRegistry;
use crate::tmp_capability::remove_tmp_resource;
use crate::tmp_capability::tmp_resources;

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
    cwd: Arc<Mutex<PathBuf>>,
    processes: Arc<Mutex<ProcessRegistry>>,
}

impl HostrunSession {
    pub fn new() -> Result<Self, HostrunSessionError> {
        Self::new_with_capability_mode(HostCapabilityMode::PendingApproval)
    }

    pub fn new_auto_approve() -> Result<Self, HostrunSessionError> {
        Self::new_with_capability_mode(HostCapabilityMode::AutoApprove)
    }

    pub fn new_auto_approve_with_cwd(cwd: impl AsRef<Path>) -> Result<Self, HostrunSessionError> {
        Self::new_with_capability_mode_and_cwd(HostCapabilityMode::AutoApprove, cwd)
    }

    fn new_with_capability_mode(
        capability_mode: HostCapabilityMode,
    ) -> Result<Self, HostrunSessionError> {
        let cwd = std::env::current_dir()
            .map_err(|error| HostrunSessionError::Eval(format!("failed to read cwd: {error}")))?;
        Self::new_with_capability_mode_and_cwd(capability_mode, cwd)
    }

    fn new_with_capability_mode_and_cwd(
        capability_mode: HostCapabilityMode,
        cwd: impl AsRef<Path>,
    ) -> Result<Self, HostrunSessionError> {
        let runtime = Runtime::new().map_err(HostrunSessionError::from_quickjs)?;
        let context = Context::full(&runtime).map_err(HostrunSessionError::from_quickjs)?;
        let cwd = canonicalize_cwd(cwd.as_ref())?;
        let cwd = Arc::new(Mutex::new(cwd));
        let processes = Arc::new(Mutex::new(ProcessRegistry::default()));
        let session = Self {
            _runtime: runtime,
            context,
            capability_mode,
            cwd,
            processes,
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
        let invoker = Arc::new(HostCapabilityInvoker {
            capability_mode,
            cwd: Arc::clone(&self.cwd),
            processes: Arc::clone(&self.processes),
        });
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
        if let Ok(mut processes) = self.processes.lock() {
            processes.kill_all();
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
    cwd: Arc<Mutex<PathBuf>>,
    processes: Arc<Mutex<ProcessRegistry>>,
}

impl HostCapabilityInvoker {
    fn invoke_tool(&self, tool_path: &str, args_json: &str) -> String {
        let args = serde_json::from_str(args_json).unwrap_or(Value::Null);
        match tool_path {
            "host.cwd" => self.invoke_host_cwd(),
            "host.cd" => self.invoke_host_cd(args),
            "fs.write" | "fs.read" | "fs.exists" | "fs.remove" | "fs.glob" => {
                self.invoke_fs_operation(tool_path, args)
            }
            "rclone.deletefile" => pending_approval(rclone_deletefile_approval(args)),
            "http.request" => self.invoke_http_request(args),
            "process.wait" => self.invoke_process_wait(args),
            "process.kill" => self.invoke_process_kill(args),
            tool if tool.starts_with("cli.") => self.invoke_cli_command(tool, args),
            _ => json!({
                "type": "denied",
                "reason": format!("Hostrun capability is unavailable: {tool_path}")
            })
            .to_string(),
        }
    }

    fn invoke_fs_operation(&self, tool_path: &str, args: Value) -> String {
        let cwd = match self.current_cwd() {
            Ok(cwd) => cwd,
            Err(error) => return denied(error.to_string()),
        };
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => {
                pending_approval(fs_approval(tool_path, args, &cwd))
            }
            HostCapabilityMode::AutoApprove => match execute_fs_operation(tool_path, args, &cwd) {
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
            HostCapabilityMode::AutoApprove => match self.execute_cli_command(tool_path, args) {
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

    fn invoke_process_wait(&self, args: Value) -> String {
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => {
                pending_approval(process_approval("process.wait", "Wait for process", args))
            }
            HostCapabilityMode::AutoApprove => match self.wait_process(args) {
                Ok(value) => completed(value),
                Err(error) => denied(error.to_string()),
            },
        }
    }

    fn invoke_process_kill(&self, args: Value) -> String {
        match self.capability_mode {
            HostCapabilityMode::PendingApproval => {
                pending_approval(process_approval("process.kill", "Kill process", args))
            }
            HostCapabilityMode::AutoApprove => match self.kill_process(args) {
                Ok(value) => completed(value),
                Err(error) => denied(error.to_string()),
            },
        }
    }

    fn execute_cli_command(
        &self,
        tool_path: &str,
        args: Value,
    ) -> Result<Value, HostrunSessionError> {
        let program = tool_path.trim_start_matches("cli.");
        let (cli_args, io) = cli_payload::split_command_payload(args);
        let payload = cli_payload::command_args(program, cli_args, io);
        let Value::Object(payload) = merge_cli_payload(program, payload, tool_path)? else {
            unreachable!("cli command payload is always an object");
        };
        let argv = cli_payload::payload_args(&payload)?;
        let session_cwd = self.current_cwd()?;
        let cwd = cli_payload::payload_cwd(&payload, &session_cwd)?;
        if payload.get("action").and_then(Value::as_str) == Some("spawn") {
            return self.spawn_process(program, &argv, payload, &cwd);
        }
        let output = cli_execution::run_cli_process(program, &argv, payload.get("stdin"), &cwd)?;
        cli_execution::cli_execution_result(program, &argv, &payload, output, &cwd)
    }

    fn spawn_process(
        &self,
        program: &str,
        argv: &[String],
        payload: serde_json::Map<String, Value>,
        cwd: &Path,
    ) -> Result<Value, HostrunSessionError> {
        if payload.get("stdin").and_then(cli_execution::stdin_type) == Some("stream") {
            return Err(HostrunSessionError::Eval(
                "spawn does not support stream stdin; use run() for command graphs".to_string(),
            ));
        }
        let stdin = cli_execution::stdin_input(payload.get("stdin"), cwd)?;
        let mut child =
            cli_execution::spawn_cli_process(program, argv, stdin.bytes.is_some(), cwd)?;
        let pid = child.id();
        cli_execution::write_cli_stdin(program, &mut child, stdin.bytes)?;
        let mut processes = self.processes.lock().map_err(|error| {
            HostrunSessionError::Eval(format!("failed to lock process registry: {error}"))
        })?;
        let id = processes.insert(program, argv, child, payload, cwd.to_path_buf());
        Ok(crate::process_registry::started_value(
            &id, pid, program, argv,
        ))
    }

    fn wait_process(&self, args: Value) -> Result<Value, HostrunSessionError> {
        let id = field_as_string(&args, "id");
        let process = self.take_process(&id)?;
        let output = process.child.wait_with_output().map_err(|error| {
            HostrunSessionError::Eval(format!("failed to wait for {}: {error}", process.program))
        })?;
        let output =
            cli_execution::cli_process_output(&process.program, &process.argv, Vec::new(), output);
        cli_execution::cli_execution_result(
            &process.program,
            &process.argv,
            &process.payload,
            output,
            &process.cwd,
        )
    }

    fn kill_process(&self, args: Value) -> Result<Value, HostrunSessionError> {
        let id = field_as_string(&args, "id");
        let mut process = self.take_process(&id)?;
        process.child.kill().map_err(|error| {
            HostrunSessionError::Eval(format!("failed to kill {}: {error}", process.program))
        })?;
        let status = process.child.wait().map_err(|error| {
            HostrunSessionError::Eval(format!(
                "failed to wait for killed {}: {error}",
                process.program
            ))
        })?;
        Ok(json!({
            "id": id,
            "program": process.program,
            "args": process.argv,
            "exitCode": status.code(),
            "success": false,
            "killed": true
        }))
    }

    fn take_process(&self, id: &str) -> Result<ManagedProcess, HostrunSessionError> {
        self.processes
            .lock()
            .map_err(|error| {
                HostrunSessionError::Eval(format!("failed to lock process registry: {error}"))
            })?
            .take(id)
            .ok_or_else(|| HostrunSessionError::Eval(format!("unknown Hostrun process: {id}")))
    }

    fn invoke_host_cwd(&self) -> String {
        match self.current_cwd() {
            Ok(cwd) => completed(json!(cwd)),
            Err(error) => denied(error.to_string()),
        }
    }

    fn invoke_host_cd(&self, args: Value) -> String {
        let path = field_as_string(&args, "path");
        match self.change_cwd(&path) {
            Ok(cwd) => completed(json!(cwd)),
            Err(error) => denied(error.to_string()),
        }
    }

    fn current_cwd(&self) -> Result<PathBuf, HostrunSessionError> {
        self.cwd.lock().map(|cwd| cwd.clone()).map_err(|error| {
            HostrunSessionError::Eval(format!("failed to lock Hostrun cwd: {error}"))
        })
    }

    fn change_cwd(&self, path: &str) -> Result<PathBuf, HostrunSessionError> {
        let current_cwd = self.current_cwd()?;
        let requested = crate::fs_capability::resolve_path(&current_cwd, path);
        let cwd = canonicalize_cwd(&requested)?;
        *self.cwd.lock().map_err(|error| {
            HostrunSessionError::Eval(format!("failed to lock Hostrun cwd: {error}"))
        })? = cwd.clone();
        Ok(cwd)
    }
}

fn canonicalize_cwd(path: &Path) -> Result<PathBuf, HostrunSessionError> {
    let cwd = path.canonicalize().map_err(|error| {
        HostrunSessionError::Eval(format!("failed to resolve cwd {}: {error}", path.display()))
    })?;
    if !cwd.is_dir() {
        return Err(HostrunSessionError::Eval(format!(
            "Hostrun cwd is not a directory: {}",
            cwd.display()
        )));
    }
    Ok(cwd)
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
    let (cli_args, io) = cli_payload::split_command_payload(args);
    let command = cli_approval::command_summary(program, &cli_args);
    let io_summary = cli_approval::io_summary(io.as_ref());
    let verb = if io
        .as_ref()
        .and_then(|io| io.get("action"))
        .and_then(Value::as_str)
        == Some("spawn")
    {
        "Spawn"
    } else {
        "Run"
    };
    HostrunApprovalRequest {
        id: format!("cli.{program}:{command}"),
        tool: format!("cli.{program}"),
        summary: format!("{verb} {command}{io_summary}"),
        args: cli_payload::command_args(program, cli_args, io),
    }
}

fn process_approval(tool: &str, verb: &str, args: Value) -> HostrunApprovalRequest {
    let id = field_as_string(&args, "id");
    HostrunApprovalRequest {
        id: format!("{tool}:{id}"),
        tool: tool.to_string(),
        summary: format!("{verb} {id}"),
        args,
    }
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
#[path = "table_error_tests.rs"]
mod table_error_tests;

#[cfg(test)]
#[path = "path_tests.rs"]
mod path_tests;

#[cfg(test)]
#[path = "byte_tests.rs"]
mod byte_tests;

#[cfg(test)]
#[path = "date_tests.rs"]
mod date_tests;

#[cfg(test)]
#[path = "cwd_tests.rs"]
mod cwd_tests;

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
#[path = "cli_wrapper_tests.rs"]
mod cli_wrapper_tests;

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod spawn_tests;

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

#[cfg(test)]
#[path = "github_pr_tests.rs"]
mod github_pr_tests;

#[cfg(test)]
#[path = "git_commit_tests.rs"]
mod git_commit_tests;
