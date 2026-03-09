use async_trait::async_trait;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde_json::Value;
use serde_json::to_value;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::client_common::tools::ToolSearchOutputNamespace;
use crate::client_common::tools::ToolSearchOutputTool;
use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;
use crate::mcp_connection_manager::qualify_responses_api_tool_name;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::spec::mcp_tool_to_deferred_openai_tool;

pub struct ToolSearchHandler;

pub(crate) const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub(crate) const DEFAULT_LIMIT: usize = 8;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct ToolSearchArgs {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Clone)]
struct ToolEntry {
    name: String,
    info: ToolInfo,
    search_text: String,
}

impl ToolEntry {
    fn new(name: String, info: ToolInfo) -> Self {
        let input_keys = info
            .tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let search_text = build_search_text(&name, &info, &input_keys);
        Self {
            name,
            info,
            search_text,
        }
    }
}

#[async_trait]
impl ToolHandler for ToolSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            turn,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::ToolSearch { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SEARCH_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: ToolSearchArgs = serde_json::from_value(arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse tool_search arguments: {err}"
            ))
        })?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit;

        let mcp_tools = session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;

        let connectors = connectors::with_app_enabled_state(
            connectors::accessible_connectors_from_mcp_tools(&mcp_tools),
            &turn.config,
        );
        let mcp_tools = filter_codex_apps_mcp_tools(mcp_tools, &connectors);
        let mcp_tools = connectors::filter_codex_apps_tools_by_policy(mcp_tools, &turn.config);

        let mut entries: Vec<ToolEntry> = mcp_tools
            .into_iter()
            .map(|(name, info)| ToolEntry::new(name, info))
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        if entries.is_empty() {
            return Ok(ToolOutput::ToolSearch { tools: Vec::new() });
        }

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| Document::new(idx, entry.search_text.clone()))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        let results = search_engine.search(query, limit);

        let matched_entries = results
            .into_iter()
            .filter_map(|result| entries.get(result.document.id))
            .collect::<Vec<_>>();
        let tools =
            serialize_tool_search_output_tools(&matched_entries, &connectors).map_err(|err| {
                FunctionCallError::Fatal(format!("failed to encode tool_search output: {err}"))
            })?;

        Ok(ToolOutput::ToolSearch { tools })
    }
}

fn serialize_tool_search_output_tools(
    matched_entries: &[&ToolEntry],
    connectors: &[AppInfo],
) -> Result<Vec<Value>, serde_json::Error> {
    let connectors_by_id = connectors
        .iter()
        .map(|connector| (connector.id.as_str(), connector))
        .collect::<HashMap<_, _>>();
    let mut namespace_positions: HashMap<&str, usize> = HashMap::new();
    let mut namespaces: Vec<ToolSearchOutputNamespace> = Vec::new();

    for entry in matched_entries {
        let Some(connector_id) = entry.info.connector_id.as_deref() else {
            continue;
        };
        let tool = ToolSearchOutputTool::Function(mcp_tool_to_deferred_openai_tool(
            entry.name.clone(),
            entry.info.tool.clone(),
        )?);

        if let Some(index) = namespace_positions.get(connector_id).copied() {
            namespaces[index].tools.push(tool);
            continue;
        }

        let description = connectors_by_id
            .get(connector_id)
            .and_then(|connector| connector.description.clone())
            .or_else(|| entry.info.connector_description.clone())
            .or_else(|| {
                connectors_by_id
                    .get(connector_id)
                    .map(|connector| connector.name.trim())
                    .filter(|name| !name.is_empty())
                    .map(|name| format!("Tools for working with {name}."))
            })
            .or_else(|| {
                entry
                    .info
                    .connector_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(|name| format!("Tools for working with {name}."))
            })
            .unwrap_or_default();
        let namespace_name = qualify_responses_api_tool_name(
            &entry.info.server_name,
            derive_namespace_stem(
                matched_entries,
                connector_id,
                connectors_by_id.get(connector_id).copied(),
            )
            .as_str(),
        );
        namespaces.push(ToolSearchOutputNamespace {
            name: namespace_name,
            description,
            tools: vec![tool],
        });
        namespace_positions.insert(connector_id, namespaces.len() - 1);
    }

    namespaces
        .into_iter()
        .map(|namespace| to_value(ToolSearchOutputTool::Namespace(namespace)))
        .collect()
}

fn derive_namespace_stem(
    matched_entries: &[&ToolEntry],
    connector_id: &str,
    connector: Option<&AppInfo>,
) -> String {
    let connector_entries = matched_entries
        .iter()
        .copied()
        .filter(|entry| entry.info.connector_id.as_deref() == Some(connector_id))
        .collect::<Vec<_>>();
    if connector_entries.is_empty() {
        return connector_id.to_string();
    }

    let raw_tool_names = connector_entries
        .iter()
        .map(|entry| entry.info.tool_name.as_str())
        .collect::<Vec<_>>();

    for candidate in [
        connector.map(|connector| connector.name.as_str()),
        connector_entries
            .iter()
            .find_map(|entry| entry.info.connector_name.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        let normalized = normalize_namespace_candidate(candidate);
        if !normalized.is_empty()
            && raw_tool_names
                .iter()
                .all(|name| tool_name_matches_namespace(name, &normalized))
        {
            return normalized;
        }
    }

    let mut stems = HashMap::new();
    for raw_tool_name in raw_tool_names {
        let stem = namespace_stem_from_tool_name(raw_tool_name);
        *stems.entry(stem).or_insert(0usize) += 1;
    }

    stems
        .into_iter()
        .max_by(|(left_stem, left_count), (right_stem, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| left_stem.len().cmp(&right_stem.len()))
                .then_with(|| right_stem.cmp(left_stem))
        })
        .map(|(stem, _)| stem)
        .unwrap_or_else(|| connector_id.to_string())
}

fn normalize_namespace_candidate(candidate: &str) -> String {
    let mut normalized = String::with_capacity(candidate.len());
    let mut last_was_separator = false;

    for character in candidate.trim().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push('_');
            last_was_separator = true;
        }
    }

    normalized.trim_matches('_').to_string()
}

fn tool_name_matches_namespace(tool_name: &str, namespace: &str) -> bool {
    tool_name == namespace
        || tool_name
            .strip_prefix(namespace)
            .is_some_and(|suffix| suffix.starts_with('_'))
}

fn namespace_stem_from_tool_name(tool_name: &str) -> String {
    if let Some((namespace, _)) = tool_name.rsplit_once("__") {
        return namespace.to_string();
    }
    if let Some((namespace, _)) = tool_name.split_once('_') {
        return namespace.to_string();
    }
    tool_name.to_string()
}

fn filter_codex_apps_mcp_tools(
    mut mcp_tools: HashMap<String, ToolInfo>,
    connectors: &[AppInfo],
) -> HashMap<String, ToolInfo> {
    let enabled_connectors: HashSet<&str> = connectors
        .iter()
        .filter(|connector| connector.is_enabled)
        .map(|connector| connector.id.as_str())
        .collect();

    mcp_tools.retain(|_, tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return false;
        }

        tool.connector_id
            .as_deref()
            .is_some_and(|connector_id| enabled_connectors.contains(connector_id))
    });
    mcp_tools
}

fn build_search_text(name: &str, info: &ToolInfo, input_keys: &[String]) -> String {
    let mut parts = vec![
        name.to_string(),
        info.tool_name.clone(),
        info.server_name.clone(),
    ];

    if let Some(title) = info.tool.title.as_deref()
        && !title.trim().is_empty()
    {
        parts.push(title.to_string());
    }

    if let Some(description) = info.tool.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(connector_name) = info.connector_name.as_deref()
        && !connector_name.trim().is_empty()
    {
        parts.push(connector_name.to_string());
    }

    if let Some(connector_description) = info.connector_description.as_deref()
        && !connector_description.trim().is_empty()
    {
        parts.push(connector_description.to_string());
    }

    if !input_keys.is_empty() {
        parts.extend(input_keys.iter().cloned());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AppInfo;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use serde_json::json;
    use std::sync::Arc;

    fn make_connector(id: &str, name: &str, description: Option<&str>, enabled: bool) -> AppInfo {
        AppInfo {
            id: id.to_string(),
            name: name.to_string(),
            description: description.map(str::to_string),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: enabled,
        }
    }

    fn make_tool(
        qualified_name: &str,
        server_name: &str,
        tool_name: &str,
        connector_id: Option<&str>,
    ) -> (String, ToolInfo) {
        (
            qualified_name.to_string(),
            ToolInfo {
                server_name: server_name.to_string(),
                tool_name: tool_name.to_string(),
                tool: Tool {
                    name: tool_name.to_string().into(),
                    title: None,
                    description: Some(format!("Test tool: {tool_name}").into()),
                    input_schema: Arc::new(JsonObject::default()),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: connector_id.map(str::to_string),
                connector_name: connector_id.map(str::to_string),
                connector_description: None,
            },
        )
    }

    #[test]
    fn filter_codex_apps_mcp_tools_keeps_enabled_apps_only() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__calendar_create_event",
                CODEX_APPS_MCP_SERVER_NAME,
                "calendar_create_event",
                Some("calendar"),
            ),
            make_tool(
                "mcp__codex_apps__drive_search",
                CODEX_APPS_MCP_SERVER_NAME,
                "drive_search",
                Some("drive"),
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);
        let connectors = vec![
            make_connector("calendar", "calendar", None, false),
            make_connector("drive", "drive", None, true),
        ];

        let mut filtered: Vec<String> = filter_codex_apps_mcp_tools(mcp_tools, &connectors)
            .into_keys()
            .collect();
        filtered.sort();

        assert_eq!(filtered, vec!["mcp__codex_apps__drive_search".to_string()]);
    }

    #[test]
    fn filter_codex_apps_mcp_tools_drops_apps_without_connector_id() {
        let mcp_tools = HashMap::from([
            make_tool(
                "mcp__codex_apps__unknown",
                CODEX_APPS_MCP_SERVER_NAME,
                "unknown",
                None,
            ),
            make_tool("mcp__rmcp__echo", "rmcp", "echo", None),
        ]);

        let mut filtered: Vec<String> = filter_codex_apps_mcp_tools(
            mcp_tools,
            &[make_connector("calendar", "calendar", None, true)],
        )
        .into_keys()
        .collect();
        filtered.sort();

        assert_eq!(filtered, Vec::<String>::new());
    }

    #[test]
    fn serialize_tool_search_output_tools_groups_results_by_namespace() {
        let connectors = vec![
            make_connector("calendar", "Calendar", Some("Plan events"), true),
            make_connector("gmail", "Gmail", Some("Read mail"), true),
        ];
        let entries = [
            ToolEntry::new(
                "mcp__codex_apps__calendar_create_event".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "calendar_create_event".to_string(),
                    tool: Tool {
                        name: "calendar_create_event".to_string().into(),
                        title: None,
                        description: Some("Create a calendar event.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    connector_description: Some("Plan events".to_string()),
                },
            ),
            ToolEntry::new(
                "mcp__codex_apps__gmail_read_email".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "gmail_read_email".to_string(),
                    tool: Tool {
                        name: "gmail_read_email".to_string().into(),
                        title: None,
                        description: Some("Read an email.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("gmail".to_string()),
                    connector_name: Some("Gmail".to_string()),
                    connector_description: Some("Read mail".to_string()),
                },
            ),
            ToolEntry::new(
                "mcp__codex_apps__calendar_list_events".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "calendar_list_events".to_string(),
                    tool: Tool {
                        name: "calendar_list_events".to_string().into(),
                        title: None,
                        description: Some("List calendar events.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    connector_description: Some("Plan events".to_string()),
                },
            ),
        ];

        let tools = serialize_tool_search_output_tools(
            &[&entries[0], &entries[1], &entries[2]],
            &connectors,
        )
        .expect("serialize tool search output");

        assert_eq!(
            tools,
            vec![
                json!({
                    "type": "namespace",
                    "name": "mcp__codex_apps__calendar",
                    "description": "Plan events",
                    "tools": [
                        {
                            "type": "function",
                            "name": "mcp__codex_apps__calendar_create_event",
                            "description": "Create a calendar event.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "type": "function",
                            "name": "mcp__codex_apps__calendar_list_events",
                            "description": "List calendar events.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                }),
                json!({
                    "type": "namespace",
                    "name": "mcp__codex_apps__gmail",
                    "description": "Read mail",
                    "tools": [
                        {
                            "type": "function",
                            "name": "mcp__codex_apps__gmail_read_email",
                            "description": "Read an email.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                })
            ]
        );
    }

    #[test]
    fn serialize_tool_search_output_tools_falls_back_to_connector_name_description() {
        let connectors = vec![make_connector("connector_gmail_456", "Gmail", None, true)];
        let entries = [ToolEntry::new(
            "mcp__codex_apps__gmail_batch_read_email".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "gmail_batch_read_email".to_string(),
                tool: Tool {
                    name: "gmail_batch_read_email".to_string().into(),
                    title: None,
                    description: Some("Read multiple emails.".into()),
                    input_schema: Arc::new(JsonObject::from_iter([(
                        "type".to_string(),
                        json!("object"),
                    )])),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: Some("connector_gmail_456".to_string()),
                connector_name: Some("Gmail".to_string()),
                connector_description: None,
            },
        )];

        let tools =
            serialize_tool_search_output_tools(&[&entries[0]], &connectors).expect("serialize");

        assert_eq!(
            tools,
            vec![json!({
                "type": "namespace",
                "name": "mcp__codex_apps__gmail",
                "description": "Tools for working with Gmail.",
                "tools": [
                    {
                        "type": "function",
                        "name": "mcp__codex_apps__gmail_batch_read_email",
                        "description": "Read multiple emails.",
                        "strict": false,
                        "defer_loading": true,
                        "parameters": {
                            "type": "object",
                            "properties": {}
                        }
                    }
                ]
            })]
        );
    }
}
