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
    let cli_args = match args {
        Value::Array(args) => args,
        Value::Null => Vec::new(),
        other => vec![other],
    };
    let command = cli_command_summary(program, &cli_args);
    HostrunApprovalRequest {
        id: format!("cli.{program}:{command}"),
        tool: format!("cli.{program}"),
        summary: format!("Run {command}"),
        args: json!({
            "program": program,
            "args": cli_args,
        }),
    }
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

const HOSTRUN_BOOTSTRAP: &str = r#"
globalThis.ctx = globalThis.ctx ?? {};
globalThis.__hostrun_console = [];

globalThis.__hostrun_formatConsoleValue = function (value) {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch (_error) {
    return String(value);
  }
};

globalThis.__hostrun_consolePush = function (level, args) {
  globalThis.__hostrun_console.push({
    level,
    message: Array.from(args).map(globalThis.__hostrun_formatConsoleValue).join(" ")
  });
};

globalThis.console = {
  log: function (...args) { globalThis.__hostrun_consolePush("log", args); },
  info: function (...args) { globalThis.__hostrun_consolePush("info", args); },
  warn: function (...args) { globalThis.__hostrun_consolePush("warn", args); },
  error: function (...args) { globalThis.__hostrun_consolePush("error", args); },
  debug: function (...args) { globalThis.__hostrun_consolePush("debug", args); }
};

if (!Array.prototype.containing) {
  Object.defineProperty(Array.prototype, "containing", {
    value: function (needle) {
      return this.filter((value) => String(value).includes(String(needle)));
    },
    configurable: true,
    writable: true
  });
}

globalThis.__hostrun_invokeCapability = function (path, payload) {
  const response = JSON.parse(globalThis.__hostrun_invokeTool(path, JSON.stringify(payload ?? {})));
  if (response.type === "needs_approval") {
    throw new Error("__HOSTRUN_APPROVAL_REQUIRED__:" + JSON.stringify(response.approval));
  }
  if (response.type === "denied") {
    throw new Error(response.reason);
  }
  return response.value;
};

globalThis.__hostrun_toolProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_toolProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      const payload = args.length > 0 ? args[0] : {};
      return globalThis.__hostrun_invokeCapability(path, payload);
    }
  });
};

globalThis.tools = globalThis.__hostrun_toolProxy("");

globalThis.fs = {
  write: function (path, content) {
    return globalThis.__hostrun_invokeCapability("fs.write", { path, content });
  }
};

globalThis.rclone = {
  deletefile: function (target) {
    return globalThis.__hostrun_invokeCapability("rclone.deletefile", { target });
  }
};

globalThis.__hostrun_cliProxy = function (path) {
  return new Proxy(function () {}, {
    get(_target, property) {
      return globalThis.__hostrun_cliProxy(path ? path + "." + String(property) : String(property));
    },
    apply(_target, _thisArg, args) {
      const response = JSON.parse(globalThis.__hostrun_invokeTool("cli." + path, JSON.stringify(args)));
      if (response.type === "needs_approval") {
        throw new Error("__HOSTRUN_APPROVAL_REQUIRED__:" + JSON.stringify(response.approval));
      }
      if (response.type === "denied") {
        throw new Error(response.reason);
      }
      return response.value;
    }
  });
};

globalThis.cli = globalThis.__hostrun_cliProxy("");

globalThis.__hostrun_run = function (code) {
  globalThis.__hostrun_console = [];
  try {
    const value = (0, eval)(code);
    return JSON.stringify({
      type: "completed",
      executed: code,
      console: globalThis.__hostrun_console,
      value: value === undefined ? null : value
    });
  } catch (error) {
    const message = error && error.message ? String(error.message) : String(error);
    if (message.startsWith("__HOSTRUN_APPROVAL_REQUIRED__:")) {
      return message;
    }
    throw error;
  }
};
"#;

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

    #[test]
    fn cli_program_proxy_returns_command_approval() {
        let session = HostrunSession::new().expect("session");

        let result = session.eval("cli.dmidecode();").expect("approval");

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
            .eval("cli.rg('needle', 'src', { '--json': true });")
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
