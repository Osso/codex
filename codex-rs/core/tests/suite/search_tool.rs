#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexThread;
use codex_core::config::Config;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const SEARCH_TOOL_DESCRIPTION_SNIPPETS: [&str; 2] = [
    "MCP tools of the apps (Calendar) are hidden until you search for them with this tool (`tool_search`).",
    "Read the returned `tool_search_output.tools` namespaces to see the matching Apps tools grouped by app.",
];
const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const CALENDAR_CREATE_TOOL: &str = "mcp__codex_apps__calendar_create_event";
const CALENDAR_LIST_TOOL: &str = "mcp__codex_apps__calendar_list_events";

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn tool_search_description(body: &Value) -> Option<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("type").and_then(Value::as_str) == Some(TOOL_SEARCH_TOOL_NAME) {
                    tool.get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn tool_search_output_item(request: &ResponsesRequest, call_id: &str) -> Value {
    request.tool_search_output(call_id)
}

fn tool_search_output_tools(request: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    tool_search_output_item(request, call_id)
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn configure_apps(config: &mut Config, apps_base_url: &str) {
    config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    config
        .features
        .disable(Feature::AppsMcpGateway)
        .expect("test config should allow feature update");
    config.chatgpt_base_url = apps_base_url.to_string();
}

fn configured_builder(apps_base_url: String) -> TestCodexBuilder {
    test_codex().with_config(move |config| configure_apps(config, apps_base_url.as_str()))
}

async fn submit_user_input(thread: &Arc<CodexThread>, text: &str) -> Result<()> {
    thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_flag_adds_tool_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = body
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools array should exist");
    let tool_search = tools
        .iter()
        .find(|tool| tool.get("type").and_then(Value::as_str) == Some(TOOL_SEARCH_TOOL_NAME))
        .cloned()
        .expect("tool_search should be present");

    assert_eq!(
        tool_search,
        json!({
            "type": "tool_search",
            "execution": "client",
            "description": tool_search["description"].as_str().expect("description should exist"),
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query for apps tools."},
                    "limit": {"type": "number", "description": "Maximum number of tools to return (defaults to 8)."},
                },
                "required": ["query"],
                "additionalProperties": false,
            }
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_adds_discovery_instructions_to_tool_description() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let description = tool_search_description(&body).expect("tool_search description should exist");
    assert!(
        SEARCH_TOOL_DESCRIPTION_SNIPPETS
            .iter()
            .all(|snippet| description.contains(snippet)),
        "tool_search description should include the updated workflow: {description:?}"
    );
    assert!(
        !description.contains("remainder of the current session/thread"),
        "tool_search description should not mention legacy client-side persistence: {description:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_hides_apps_tools_without_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "hello tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME));
    assert!(!tools.iter().any(|name| name == CALENDAR_CREATE_TOOL));
    assert!(!tools.iter().any(|name| name == CALENDAR_LIST_TOOL));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_app_mentions_expose_apps_tools_without_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "Use [$calendar](app://calendar) and then call tools.",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(tools.iter().any(|name| name == CALENDAR_CREATE_TOOL));
    assert!(tools.iter().any(|name| name == CALENDAR_LIST_TOOL));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_search_returns_deferred_tools_without_follow_up_tool_injection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "tool-search-1";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(
                    call_id,
                    &json!({
                        "query": "create calendar event",
                        "limit": 1,
                    }),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;
    submit_user_input(&test.codex, "Find the calendar create tool").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);

    let first_request_tools = tool_names(&requests[0].body_json());
    assert!(
        first_request_tools
            .iter()
            .any(|name| name == TOOL_SEARCH_TOOL_NAME),
        "first request should advertise tool_search: {first_request_tools:?}"
    );
    assert!(
        !first_request_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "app tools should still be hidden before search: {first_request_tools:?}"
    );

    let output_item = tool_search_output_item(&requests[1], call_id);
    assert_eq!(
        output_item.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output_item.get("execution").and_then(Value::as_str),
        Some("client")
    );

    let tools = tool_search_output_tools(&requests[1], call_id);
    assert_eq!(
        tools,
        vec![json!({
            "type": "namespace",
            "name": "mcp__codex_apps__calendar",
            "description": "Plan events and manage your calendar.",
            "tools": [
                {
                    "type": "function",
                    "name": CALENDAR_CREATE_TOOL,
                    "description": "Create a calendar event.",
                    "strict": false,
                    "defer_loading": true,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "starts_at": {"type": "string"},
                            "timezone": {"type": "string"},
                            "title": {"type": "string"},
                        },
                        "required": ["title", "starts_at"],
                        "additionalProperties": false,
                    }
                }
            ]
        })]
    );

    let second_request_tools = tool_names(&requests[1].body_json());
    assert!(
        !second_request_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "follow-up request should rely on tool_search_output history, not tool injection: {second_request_tools:?}"
    );

    Ok(())
}
