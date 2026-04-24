use codex_protocol::mcp::CallToolResult;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

/// Arguments passed to a configured MCP permission-prompt tool.
///
/// This intentionally uses snake_case keys to match the `permission_prompt_tool`
/// compatibility contract:
/// `{tool_name, input, tool_use_id?}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionPromptRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// Parsed decision body returned by the configured permission-prompt tool.
///
/// The decision body is expected to be encoded as JSON text inside a
/// `CallToolResult` single text content item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "behavior", rename_all = "lowercase")]
pub enum PermissionPromptDecision {
    Allow {
        #[serde(
            rename = "updatedInput",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        updated_input: Option<serde_json::Value>,
        #[serde(
            rename = "updatedPermissions",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    Deny {
        message: String,
        #[serde(
            rename = "updatedPermissions",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PermissionUpdate {
    #[serde(rename = "addRules")]
    AddRules {
        destination: PermissionDestination,
        behavior: PermissionRuleBehavior,
        rules: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDestination {
    Session,
    UserSettings,
    ProjectSettings,
    LocalSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionRuleBehavior {
    Allow,
    Deny,
}

#[derive(Debug, Error)]
pub enum PermissionPromptContractError {
    #[error("permission prompt tool returned is_error=true")]
    ToolReturnedError,
    #[error("permission prompt tool result must contain exactly one content item, found {count}")]
    ExpectedSingleContentItem { count: usize },
    #[error("permission prompt tool result must contain a single text content item")]
    ExpectedSingleTextContentItem,
    #[error("permission prompt decision JSON was invalid: {0}")]
    InvalidDecisionJson(#[from] serde_json::Error),
}

impl PermissionPromptRequest {
    pub fn to_tool_arguments(self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

impl PermissionPromptDecision {
    /// Encode this decision in the `CallToolResult` contract used by
    /// permission-prompt MCP tools:
    /// one `text` content item containing a JSON string body.
    pub fn to_call_tool_result(&self) -> Result<CallToolResult, serde_json::Error> {
        let decision_body = serde_json::to_string(self)?;
        Ok(CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": decision_body,
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        })
    }

    /// Parse a decision from a `CallToolResult` produced by a permission-prompt tool.
    pub fn from_call_tool_result(
        result: &CallToolResult,
    ) -> Result<Self, PermissionPromptContractError> {
        if result.is_error == Some(true) {
            return Err(PermissionPromptContractError::ToolReturnedError);
        }

        if result.content.len() != 1 {
            return Err(PermissionPromptContractError::ExpectedSingleContentItem {
                count: result.content.len(),
            });
        }

        #[derive(Deserialize)]
        struct TextContent {
            #[serde(rename = "type")]
            content_type: String,
            text: String,
        }

        let text_content = serde_json::from_value::<TextContent>(result.content[0].clone())
            .map_err(|_| PermissionPromptContractError::ExpectedSingleTextContentItem)?;
        if text_content.content_type != "text" {
            return Err(PermissionPromptContractError::ExpectedSingleTextContentItem);
        }

        Ok(serde_json::from_str::<PermissionPromptDecision>(
            &text_content.text,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn request_arguments_use_contract_shape() {
        let request = PermissionPromptRequest {
            tool_name: "exec_command".to_string(),
            input: serde_json::json!({ "cmd": "echo hi" }),
            tool_use_id: Some("toolu_123".to_string()),
        };

        let arguments = request
            .to_tool_arguments()
            .expect("request should serialize");
        assert_eq!(
            arguments,
            serde_json::json!({
                "tool_name": "exec_command",
                "input": { "cmd": "echo hi" },
                "tool_use_id": "toolu_123",
            })
        );
    }

    #[test]
    fn request_arguments_omit_tool_use_id_when_absent() {
        let request = PermissionPromptRequest {
            tool_name: "exec_command".to_string(),
            input: serde_json::json!({ "cmd": "echo hi" }),
            tool_use_id: None,
        };

        let arguments = request
            .to_tool_arguments()
            .expect("request should serialize");
        assert_eq!(
            arguments,
            serde_json::json!({
                "tool_name": "exec_command",
                "input": { "cmd": "echo hi" },
            })
        );
    }

    #[test]
    fn allow_decision_serializes_to_single_text_call_tool_result() {
        let decision = PermissionPromptDecision::Allow {
            updated_input: Some(serde_json::json!({ "sandbox_permissions": "require_escalated" })),
            updated_permissions: Some(vec![PermissionUpdate::AddRules {
                destination: PermissionDestination::Session,
                behavior: PermissionRuleBehavior::Allow,
                rules: vec![serde_json::json!({
                    "toolName": "exec_command",
                    "ruleContent": "git status",
                })],
            }]),
        };

        let result = decision
            .to_call_tool_result()
            .expect("decision should serialize");
        let parsed = PermissionPromptDecision::from_call_tool_result(&result)
            .expect("decision should parse");

        assert_eq!(parsed, decision);
    }

    #[test]
    fn deny_decision_parses_from_text_body() {
        let result = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": r#"{"behavior":"deny","message":"blocked by policy"}"#,
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let decision = PermissionPromptDecision::from_call_tool_result(&result)
            .expect("decision should parse");
        assert_eq!(
            decision,
            PermissionPromptDecision::Deny {
                message: "blocked by policy".to_string(),
                updated_permissions: None,
            }
        );
    }

    #[test]
    fn parsing_rejects_non_text_content() {
        let result = CallToolResult {
            content: vec![serde_json::json!({
                "type": "image",
                "data": "abc",
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let error = PermissionPromptDecision::from_call_tool_result(&result)
            .expect_err("non-text content should be rejected");
        assert!(matches!(
            error,
            PermissionPromptContractError::ExpectedSingleTextContentItem
        ));
    }

    #[test]
    fn parsing_rejects_multiple_content_items() {
        let result = CallToolResult {
            content: vec![
                serde_json::json!({
                    "type": "text",
                    "text": r#"{"behavior":"allow"}"#,
                }),
                serde_json::json!({
                    "type": "text",
                    "text": r#"{"behavior":"deny","message":"no"}"#,
                }),
            ],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let error = PermissionPromptDecision::from_call_tool_result(&result)
            .expect_err("multiple content items should be rejected");
        assert!(matches!(
            error,
            PermissionPromptContractError::ExpectedSingleContentItem { count: 2 }
        ));
    }
}
