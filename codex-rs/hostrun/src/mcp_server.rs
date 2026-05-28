//! Claude Code MCP server for Hostrun.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::Mutex;

use rmcp::ErrorData as McpError;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;

use crate::HostrunSessionStore;

#[derive(Clone)]
pub struct HostrunMcpServer {
    sessions: Arc<Mutex<HostrunSessionStore>>,
    tools: Arc<Vec<Tool>>,
}

impl HostrunMcpServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HostrunSessionStore::new_auto_approve())),
            tools: Arc::new(vec![hostrun_eval_tool()]),
        }
    }

    fn eval(&self, args: HostrunEvalArgs) -> Result<CallToolResult, McpError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| McpError::internal_error("Hostrun session lock was poisoned", None))?;
        let result = sessions
            .eval(args.session_id.as_deref().unwrap_or("default"), &args.code)
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let structured_content = serde_json::to_value(&result).map_err(|error| {
            McpError::internal_error(format!("failed to encode Hostrun result: {error}"), None)
        })?;
        let content_text = serde_json::to_string_pretty(&structured_content).map_err(|error| {
            McpError::internal_error(format!("failed to render Hostrun result: {error}"), None)
        })?;

        Ok(CallToolResult {
            content: vec![Content::text(content_text)],
            structured_content: Some(structured_content),
            is_error: Some(false),
            meta: None,
        })
    }
}

impl Default for HostrunMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for HostrunMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = Arc::clone(&self.tools);
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "hostrun_eval" => self.eval(parse_eval_args(request.arguments)?),
            name => Err(McpError::invalid_params(
                format!("unknown Hostrun tool: {name}"),
                None,
            )),
        }
    }
}

pub async fn run_stdio_server() -> Result<(), Box<dyn std::error::Error>> {
    let server = HostrunMcpServer::new().serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}

fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}

fn hostrun_eval_tool() -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed("hostrun_eval"),
        Cow::Borrowed(
            "Evaluate synchronous JavaScript in a persistent Hostrun QuickJS session. \
             Do not use await. Hostrun helpers return values directly in this runtime. \
             This is not Deno, Node.js, or a browser: do not use Deno.*, process.*, \
             require/import, fetch, DOM APIs, or Web APIs unless Hostrun explicitly provides them. \
             Use Hostrun helpers for host access: host.cwd()/host.cd(), fs, cli, run, http, rg, fd, sqlite, kubectl, and tools. \
             Correct command examples: run.dmidecode('-t', 'system'); cli.dmidecode('-t', 'system').complete(); tools.sudo(cli.dmidecode('-t', 'system')).run(). \
             Never call run('dmidecode -t system') or await run(...). run is a program proxy, not a shell parser. \
             For privileged commands use tools.sudo(cli.someCommand(...)); cli.sudo(...) and run.sudo(...) invoke the sudo binary literally.",
        ),
        Arc::new(hostrun_eval_input_schema()),
    );
    tool.output_schema = Some(Arc::new(hostrun_eval_output_schema()));
    tool
}

fn hostrun_eval_input_schema() -> JsonObject {
    json_object(json!({
        "type": "object",
        "properties": {
            "code": {
                "type": "string",
                "description": "Synchronous JavaScript code for Hostrun QuickJS. Do not use await. No Deno, Node.js, browser, DOM, require/import, process.*, or Deno.* APIs. Use Hostrun helpers such as host.cwd(), fs, cli, run, http, rg, fd, sqlite, kubectl, and tools. Correct command examples: run.dmidecode('-t', 'system'); cli.dmidecode('-t', 'system').complete(); tools.sudo(cli.dmidecode('-t', 'system')).run(). Never call run('dmidecode -t system') or await run(...). run is a program proxy, not a shell parser. For privileged commands use tools.sudo(cli.someCommand(...)); cli.sudo(...) and run.sudo(...) invoke the sudo binary literally."
            },
            "session_id": {
                "type": "string",
                "description": "Optional stable session id. Defaults to \"default\"."
            }
        },
        "required": ["code"],
        "additionalProperties": false
    }))
}

fn hostrun_eval_output_schema() -> JsonObject {
    json_object(json!({
        "type": "object",
        "properties": {
            "type": { "type": "string" },
            "executed": { "type": "string" },
            "console": { "type": "array" },
            "value": {},
            "approval": {}
        },
        "required": ["type", "executed", "value"],
        "additionalProperties": true
    }))
}

fn json_object(value: Value) -> JsonObject {
    match value {
        Value::Object(object) => object,
        _ => JsonObject::new(),
    }
}

fn parse_eval_args(arguments: Option<JsonObject>) -> Result<HostrunEvalArgs, McpError> {
    let Some(arguments) = arguments else {
        return Err(McpError::invalid_params(
            "missing arguments for hostrun_eval",
            None,
        ));
    };

    serde_json::from_value(Value::Object(arguments.into_iter().collect()))
        .map_err(|error| McpError::invalid_params(error.to_string(), None))
}

#[derive(Deserialize)]
struct HostrunEvalArgs {
    code: String,
    session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::HostrunEvalArgs;
    use super::HostrunMcpServer;

    #[test]
    fn hostrun_eval_tool_schema_requires_code() {
        let server = HostrunMcpServer::new();
        let tool = &server.tools[0];

        assert_eq!(tool.name, "hostrun_eval");
        assert_eq!(tool.input_schema["required"], json!(["code"]));
        assert_eq!(tool.input_schema["additionalProperties"], json!(false));
    }

    #[test]
    fn hostrun_eval_tool_schema_warns_against_common_wrong_calls() {
        let server = HostrunMcpServer::new();
        let tool = &server.tools[0];
        let tool_description = tool.description.as_deref().expect("tool description");
        let description = format!(
            "{} {}",
            tool_description,
            tool.input_schema["properties"]["code"]["description"]
                .as_str()
                .expect("code description")
        );

        assert!(description.contains("Do not use await"));
        assert!(description.contains("Never call run('dmidecode -t system')"));
        assert!(description.contains("tools.sudo(cli.dmidecode('-t', 'system')).run()"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn hostrun_eval_reuses_session_state() {
        let server = HostrunMcpServer::new();

        let first = server
            .eval(args("ctx.count = 1; ctx.count;"))
            .expect("first eval");
        let second = server
            .eval(args("ctx.count += 1; ctx.count;"))
            .expect("second eval");

        assert_eq!(first.structured_content.unwrap()["value"], json!(1));
        assert_eq!(second.structured_content.unwrap()["value"], json!(2));
    }

    fn args(code: &str) -> HostrunEvalArgs {
        HostrunEvalArgs {
            code: code.to_string(),
            session_id: Some("test-session".to_string()),
        }
    }
}
