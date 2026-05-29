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

const HOSTRUN_EVAL_DESCRIPTION: &str = "\
Evaluate synchronous JavaScript in a persistent Hostrun QuickJS session. \
Do not use await. Hostrun helpers return values directly in this runtime. \
This is not Deno, Node.js, or a browser: do not use Deno.*, process.*, \
require/import, fetch, DOM APIs, or Web APIs unless Hostrun explicitly provides them. \
Use Hostrun helpers for host access: host.cwd()/host.cd(), fs, cli, run, http, rg, fd, sqlite, kubectl, and tools. \
Use tools.file.replace(path, { from, to }) for exact targeted file edits; it requires one match by default. Use tools.file.patch(diff) or tools.file.patch(path, diff) for unified diffs. \
Use tools.browser for browser-cli Chrome/CDP automation: tools.browser.open(url).run(), tools.browser.get('title').text(), tools.browser.snapshot({ mini: true }).text(). \
Prefer Hostrun JavaScript over shell loops for HTTP polling, retries, and response parsing. \
Polling example: for (let i = 0; i < 30; i++) { const html = http.get(url, { headers: { 'User-Agent': 'Mozilla/5.0' }, tls: { acceptInvalidCerts: true } }).text(); const tag = html.match(/<script type=\"module\" src=\"[^\"]*bundle[^\"]*\"/)?.[0] ?? ''; if (tag.includes('globalcomix-frontend.nyc3.cdn')) { tag; break; } run.sleep('2'); } \
Correct command examples: run.dmidecode('-t', 'system'); cli.git('status').in('/repo').stdout.text(); tools.sudo(cli.dmidecode('-t', 'system')).run(). \
Never call run('dmidecode -t system') or await run(...). run is a program proxy, not a shell parser. \
For privileged commands use tools.sudo(cli.someCommand(...)).run(); it captures stdout and stderr by default. cli.sudo(...) and run.sudo(...) invoke the sudo binary literally.";

const HOSTRUN_CODE_DESCRIPTION: &str = "\
Synchronous JavaScript code for Hostrun QuickJS. Do not use await. No Deno, Node.js, browser, DOM, require/import, process.*, or Deno.* APIs. \
Use Hostrun helpers such as host.cwd(), fs, cli, run, http, rg, fd, sqlite, kubectl, and tools. \
Use tools.file.replace(path, { from, to }) for exact targeted file edits; it requires one match by default. Use tools.file.patch(diff) or tools.file.patch(path, diff) for unified diffs. \
Use tools.browser for browser-cli Chrome/CDP automation: tools.browser.open(url).run(), tools.browser.get('title').text(), tools.browser.snapshot({ mini: true }).text(). \
Prefer Hostrun JavaScript over shell loops for HTTP polling, retries, and response parsing. \
Polling example: for (let i = 0; i < 30; i++) { const html = http.get(url, { headers: { 'User-Agent': 'Mozilla/5.0' }, tls: { acceptInvalidCerts: true } }).text(); const tag = html.match(/<script type=\"module\" src=\"[^\"]*bundle[^\"]*\"/)?.[0] ?? ''; if (tag.includes('globalcomix-frontend.nyc3.cdn')) { tag; break; } run.sleep('2'); } \
Correct command examples: run.dmidecode('-t', 'system'); cli.git('status').in('/repo').stdout.text(); tools.sudo(cli.dmidecode('-t', 'system')).run(). \
Never call run('dmidecode -t system') or await run(...). run is a program proxy, not a shell parser. \
For privileged commands use tools.sudo(cli.someCommand(...)).run(); it captures stdout and stderr by default. cli.sudo(...) and run.sudo(...) invoke the sudo binary literally.";

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
        Cow::Borrowed(HOSTRUN_EVAL_DESCRIPTION),
        Arc::new(hostrun_eval_input_schema()),
    );
    tool.output_schema = Some(Arc::new(hostrun_eval_output_schema()));
    tool
}

fn hostrun_eval_input_schema() -> JsonObject {
    let properties = json!({
        "code": {
            "type": "string",
            "description": HOSTRUN_CODE_DESCRIPTION
        },
        "session_id": {
            "type": "string",
            "description": "Optional stable session id. Defaults to \"default\"."
        }
    });

    json_object(json!({
        "type": "object",
        "properties": properties,
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
        assert!(description.contains("Prefer Hostrun JavaScript over shell loops"));
        assert!(description.contains("acceptInvalidCerts"));
        assert!(description.contains("tools.browser.open(url).run()"));
        assert!(description.contains("tools.browser.snapshot({ mini: true }).text()"));
        assert!(description.contains("tools.file.replace(path, { from, to })"));
        assert!(description.contains("tools.file.patch(diff)"));
        assert!(description.contains("cli.git('status').in('/repo').stdout.text()"));
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
