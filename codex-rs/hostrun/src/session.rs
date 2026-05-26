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
mod tests {
    use serde_json::json;

    use super::HostrunSession;
    use super::HostrunSessionStore;

    #[test]
    fn keeps_live_ctx_objects_across_evaluations() {
        let session = HostrunSession::new().expect("session");
        session
            .eval("ctx.files = ['a.txt', 'probe.txt'];")
            .expect("set ctx");

        let result = session
            .eval("ctx.probes = ctx.files.containing('probe'); ctx.probes.length;")
            .expect("filter ctx");

        assert_eq!(result.value, Some(json!(1)));
    }

    #[test]
    fn keeps_ctx_alive_after_normal_exception() {
        let session = HostrunSession::new().expect("session");
        session
            .eval("ctx.counter = { value: 41 };")
            .expect("set ctx");
        session
            .eval("throw new Error('boom');")
            .expect_err("normal exception should be returned");

        let result = session
            .eval("ctx.counter.value += 1;")
            .expect("increment ctx");

        assert_eq!(result.value, Some(json!(42)));
    }

    #[test]
    fn returns_built_in_fs_write_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("tools.fs.write({ path: '/tmp/hostrun.txt', content: 'hello' });")
            .expect("approval");

        assert_eq!(result.result_type, "needs_approval");
        let approval = result.approval.expect("approval");
        assert_eq!(approval.id, "fs.write:/tmp/hostrun.txt");
        assert_eq!(approval.tool, "fs.write");
        assert_eq!(approval.summary, "Write 5 bytes to /tmp/hostrun.txt");
        assert_eq!(
            approval.args,
            json!({ "path": "/tmp/hostrun.txt", "content": "hello" })
        );
    }

    #[test]
    fn public_fs_write_returns_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("fs.write('/tmp/hostrun.txt', 'hello');")
            .expect("approval");

        assert_eq!(result.result_type, "needs_approval");
        let approval = result.approval.expect("approval");
        assert_eq!(approval.id, "fs.write:/tmp/hostrun.txt");
        assert_eq!(approval.tool, "fs.write");
        assert_eq!(approval.summary, "Write 5 bytes to /tmp/hostrun.txt");
        assert_eq!(
            approval.args,
            json!({ "path": "/tmp/hostrun.txt", "content": "hello" })
        );
    }

    #[test]
    fn public_fs_read_returns_approval() {
        assert_fs_path_approval(
            "fs.read('/tmp/hostrun.txt');",
            "fs.read",
            "Read /tmp/hostrun.txt",
        );
    }

    #[test]
    fn public_fs_exists_returns_approval() {
        assert_fs_path_approval(
            "fs.exists('/tmp/hostrun.txt');",
            "fs.exists",
            "Check existence of /tmp/hostrun.txt",
        );
    }

    #[test]
    fn public_fs_remove_returns_approval() {
        assert_fs_path_approval(
            "fs.remove('/tmp/hostrun.txt');",
            "fs.remove",
            "Remove /tmp/hostrun.txt",
        );
    }

    #[test]
    fn public_rclone_deletefile_returns_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("rclone.deletefile('spaces:bucket/probe.txt');")
            .expect("approval");

        assert_eq!(result.result_type, "needs_approval");
        let approval = result.approval.expect("approval");
        assert_eq!(approval.id, "rclone.deletefile:spaces:bucket/probe.txt");
        assert_eq!(approval.tool, "rclone.deletefile");
        assert_eq!(approval.summary, "Delete spaces:bucket/probe.txt");
        assert_eq!(
            approval.args,
            json!({ "target": "spaces:bucket/probe.txt" })
        );
    }

    fn assert_fs_path_approval(code: &str, tool: &str, summary: &str) {
        let session = HostrunSession::new().expect("session");

        let result = session.eval(code).expect("approval");

        assert_eq!(result.result_type, "needs_approval");
        let approval = result.approval.expect("approval");
        assert_eq!(approval.id, format!("{tool}:/tmp/hostrun.txt"));
        assert_eq!(approval.tool, tool);
        assert_eq!(approval.summary, summary);
        assert_eq!(approval.args, json!({ "path": "/tmp/hostrun.txt" }));
    }

    #[test]
    fn cli_program_proxy_returns_lazy_command_builder() {
        let session = HostrunSession::new().expect("session");

        let result = session.eval("cli.dmidecode();").expect("builder");

        assert_eq!(result.result_type, "completed");
        assert_eq!(
            result.value,
            Some(json!({
                "program": "dmidecode",
                "args": []
            }))
        );
    }

    #[test]
    fn cli_command_builder_run_returns_command_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session.eval("cli.dmidecode().run();").expect("approval");

        assert_eq!(result.result_type, "needs_approval");
        let approval = result.approval.expect("approval");
        assert_eq!(approval.id, "cli.dmidecode:dmidecode");
        assert_eq!(approval.tool, "cli.dmidecode");
        assert_eq!(approval.summary, "Run dmidecode");
        assert_eq!(
            approval.args,
            json!({
                "program": "dmidecode",
                "args": []
            })
        );
    }

    #[test]
    fn cli_program_proxy_preserves_arguments() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("cli.rg('needle', 'src', { '--json': true }).run();")
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.rg");
        assert_eq!(
            approval.args,
            json!({
                "program": "rg",
                "args": ["needle", "src", { "--json": true }]
            })
        );
    }

    #[test]
    fn cli_command_builder_includes_io_metadata_in_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval(
                "cli.rg('needle', 'src')
                  .stdout.toFile('/tmp/matches.txt')
                  .stderr.toStdout()
                  .stdin.text('input')
                  .run();",
            )
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.rg");
        assert_eq!(
            approval.args,
            json!({
                "program": "rg",
                "args": ["needle", "src"],
                "stdout": { "type": "file", "path": "/tmp/matches.txt" },
                "stderr": { "type": "stdout" },
                "stdin": { "type": "text", "text": "input" }
            })
        );
    }

    #[test]
    fn cli_command_builder_can_pipe_from_named_stdout_handle() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval(
                "const source = cli.rclone('cat', 'spaces:bucket/index.txt');
                 cli.cat().stdin(source.stdout).combined.capture().run();",
            )
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.cat");
        assert_eq!(
            approval.args,
            json!({
                "program": "cat",
                "args": [],
                "stdin": {
                    "type": "stream",
                    "source": {
                        "stream": "stdout",
                        "command": {
                            "program": "rclone",
                            "args": ["cat", "spaces:bucket/index.txt"]
                        }
                    }
                },
                "combined": { "type": "capture" }
            })
        );
    }

    #[test]
    fn rclone_lsf_wrapper_builds_rclone_command() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("rclone.lsf('spaces:bucket', { recursive: true }).stdout.lines().run();")
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.rclone");
        assert_eq!(
            approval.args,
            json!({
                "program": "rclone",
                "args": ["lsf", "spaces:bucket", "--recursive"],
                "stdout": { "type": "lines" }
            })
        );
    }

    #[test]
    fn fd_files_wrapper_builds_fdfind_command() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval(
                "fd.files('/repo', { extension: 'rs', hidden: true, exclude: ['target'] }).run();",
            )
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.fdfind");
        assert_eq!(
            approval.args,
            json!({
                "program": "fdfind",
                "args": [
                    ".",
                    "--type",
                    "file",
                    "--extension",
                    "rs",
                    "--hidden",
                    "--exclude",
                    "target",
                    "/repo"
                ]
            })
        );
    }

    #[test]
    fn rg_search_wrapper_builds_rg_command() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval(
                "rg.search('needle', 'src', {
                    fixed: true,
                    ignoreCase: true,
                    json: true,
                    glob: '*.rs'
                }).run();",
            )
            .expect("approval");

        let approval = result.approval.expect("approval");
        assert_eq!(approval.tool, "cli.rg");
        assert_eq!(
            approval.args,
            json!({
                "program": "rg",
                "args": [
                    "--fixed-strings",
                    "--ignore-case",
                    "--json",
                    "--glob",
                    "*.rs",
                    "needle",
                    "src"
                ]
            })
        );
    }

    #[test]
    fn captures_console_messages_and_echoes_executed_code() {
        let session = HostrunSession::new().expect("session");

        let result = session
            .eval("console.log('hello', { ok: true }); console.debug('trace'); 42;")
            .expect("eval");

        assert_eq!(
            result.executed,
            "console.log('hello', { ok: true }); console.debug('trace'); 42;"
        );
        assert_eq!(result.value, Some(json!(42)));
        assert_eq!(
            serde_json::to_value(result.console).expect("console serializes"),
            json!([
                { "level": "log", "message": "hello {\"ok\":true}" },
                { "level": "debug", "message": "trace" }
            ])
        );
    }

    #[test]
    fn store_keeps_sessions_separate_and_persistent() {
        let mut store = HostrunSessionStore::new();

        let first = store
            .eval("session-1", "ctx.count = 41; ctx.count;")
            .expect("first");
        let second = store
            .eval("session-1", "ctx.count += 1; ctx.count;")
            .expect("second");
        let other = store
            .eval("session-2", "ctx.count ?? null;")
            .expect("other");

        assert_eq!(first.value, Some(json!(41)));
        assert_eq!(second.value, Some(json!(42)));
        assert_eq!(
            serde_json::to_value(other).expect("other result"),
            json!({
                "type": "completed",
                "executed": "ctx.count ?? null;",
                "value": null
            })
        );
    }
}
