use std::path::PathBuf;
use std::sync::Arc;

use chrono::DateTime;
use chrono::SecondsFormat;
use chrono::Utc;
use codex_protocol::ThreadId;
use codex_protocol::models::SandboxPermissions;
use futures::future::BoxFuture;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;

pub type HookFn =
    Arc<dyn for<'a> Fn(&'a HookPayload) -> BoxFuture<'a, HookExecutionOutcome> + Send + Sync>;

#[derive(Debug)]
pub enum HookResult {
    /// Success: hook completed successfully.
    Success,
    /// FailedContinue: hook failed, but other subsequent hooks should still execute and the
    /// operation should continue.
    FailedContinue(Box<dyn std::error::Error + Send + Sync + 'static>),
    /// FailedAbort: hook failed, other subsequent hooks should not execute, and the operation
    /// should be aborted.
    FailedAbort(Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl HookResult {
    pub fn should_abort_operation(&self) -> bool {
        matches!(self, Self::FailedAbort(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookPermissionDecision {
    Allow { reason: String },
    Ask { reason: String },
    Deny { reason: String },
}

#[derive(Debug)]
pub struct HookExecutionOutcome {
    pub result: HookResult,
    pub permission_decision: Option<HookPermissionDecision>,
}

impl HookExecutionOutcome {
    pub fn success() -> Self {
        Self {
            result: HookResult::Success,
            permission_decision: None,
        }
    }
}

#[derive(Debug)]
pub struct HookResponse {
    pub hook_name: String,
    pub result: HookResult,
    pub permission_decision: Option<HookPermissionDecision>,
}

#[derive(Clone)]
pub struct Hook {
    pub name: String,
    pub func: HookFn,
}

impl Default for Hook {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            func: Arc::new(|_| Box::pin(async { HookExecutionOutcome::success() })),
        }
    }
}

impl Hook {
    pub async fn execute(&self, payload: &HookPayload) -> HookResponse {
        let outcome = (self.func)(payload).await;
        HookResponse {
            hook_name: self.name.clone(),
            result: outcome.result,
            permission_decision: outcome.permission_decision,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HookPayload {
    pub session_id: ThreadId,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    #[serde(serialize_with = "serialize_triggered_at")]
    pub triggered_at: DateTime<Utc>,
    pub hook_event: HookEvent,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CommandHookConfig {
    pub command: String,
    #[serde(default)]
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct HookRuleConfig {
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub commands: Vec<CommandHookConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct HooksToml {
    #[serde(default)]
    pub pre_tool_use: Vec<HookRuleConfig>,
    #[serde(default)]
    pub post_tool_use: Vec<HookRuleConfig>,
    #[serde(default)]
    pub user_prompt_submit: Vec<HookRuleConfig>,
    #[serde(default)]
    pub stop: Vec<HookRuleConfig>,
    #[serde(default)]
    pub session_end: Vec<HookRuleConfig>,
    #[serde(default)]
    pub subagent_start: Vec<HookRuleConfig>,
    #[serde(default)]
    pub subagent_stop: Vec<HookRuleConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventAfterAgent {
    pub thread_id: ThreadId,
    pub turn_id: String,
    pub input_messages: Vec<String>,
    pub last_assistant_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventUserPromptSubmit {
    pub turn_id: String,
    pub input_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HookToolKind {
    Function,
    Custom,
    LocalShell,
    Mcp,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookToolInputLocalShell {
    pub command: Vec<String>,
    pub workdir: Option<String>,
    pub timeout_ms: Option<u64>,
    pub sandbox_permissions: Option<SandboxPermissions>,
    pub prefix_rule: Option<Vec<String>>,
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "input_type", rename_all = "snake_case")]
pub enum HookToolInput {
    Function {
        arguments: String,
    },
    Custom {
        input: String,
    },
    LocalShell {
        params: HookToolInputLocalShell,
    },
    Mcp {
        server: String,
        tool: String,
        arguments: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookEventAfterToolUse {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub tool_kind: HookToolKind,
    pub tool_input: HookToolInput,
    pub executed: bool,
    pub success: bool,
    pub duration_ms: u64,
    pub mutating: bool,
    pub sandbox: String,
    pub sandbox_policy: String,
    pub output_preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventSessionEnd {
    pub thread_id: ThreadId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventSubagentStart {
    pub thread_id: ThreadId,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct HookEventSubagentStop {
    pub thread_id: ThreadId,
}

fn serialize_triggered_at<S>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Secs, true))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum HookEvent {
    PreToolUse {
        #[serde(flatten)]
        event: HookEventAfterToolUse,
    },
    AfterAgent {
        #[serde(flatten)]
        event: HookEventAfterAgent,
    },
    AfterToolUse {
        #[serde(flatten)]
        event: HookEventAfterToolUse,
    },
    UserPromptSubmit {
        #[serde(flatten)]
        event: HookEventUserPromptSubmit,
    },
    SessionEnd {
        #[serde(flatten)]
        event: HookEventSessionEnd,
    },
    SubagentStart {
        #[serde(flatten)]
        event: HookEventSubagentStart,
    },
    SubagentStop {
        #[serde(flatten)]
        event: HookEventSubagentStop,
    },
}

impl HookEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::PreToolUse { .. } => "pre_tool_use",
            Self::AfterAgent { .. } => "after_agent",
            Self::AfterToolUse { .. } => "after_tool_use",
            Self::UserPromptSubmit { .. } => "user_prompt_submit",
            Self::SessionEnd { .. } => "session_end",
            Self::SubagentStart { .. } => "subagent_start",
            Self::SubagentStop { .. } => "subagent_stop",
        }
    }

    pub fn matcher_subject(&self) -> Option<&str> {
        match self {
            Self::PreToolUse { event } | Self::AfterToolUse { event } => Some(&event.tool_name),
            _ => None,
        }
    }

    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::PreToolUse { event } | Self::AfterToolUse { event } => Some(&event.tool_name),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::models::SandboxPermissions;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::CommandHookConfig;
    use super::HookEvent;
    use super::HookEventAfterAgent;
    use super::HookEventAfterToolUse;
    use super::HookPayload;
    use super::HookRuleConfig;
    use super::HookToolInput;
    use super::HookToolInputLocalShell;
    use super::HookToolKind;
    use super::HooksToml;

    #[test]
    fn hook_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let thread_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            cwd: PathBuf::from("tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id,
                    turn_id: "turn-1".to_string(),
                    input_messages: vec!["hello".to_string()],
                    last_assistant_message: Some("hi".to_string()),
                },
            },
        };

        let actual = serde_json::to_value(payload).expect("serialize hook payload");
        let expected = json!({
            "session_id": session_id.to_string(),
            "cwd": "tmp",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event": {
                "event_type": "after_agent",
                "thread_id": thread_id.to_string(),
                "turn_id": "turn-1",
                "input_messages": ["hello"],
                "last_assistant_message": "hi",
            },
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn after_tool_use_payload_serializes_stable_wire_shape() {
        let session_id = ThreadId::new();
        let payload = HookPayload {
            session_id,
            cwd: PathBuf::from("tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-2".to_string(),
                    call_id: "call-1".to_string(),
                    tool_name: "local_shell".to_string(),
                    tool_kind: HookToolKind::LocalShell,
                    tool_input: HookToolInput::LocalShell {
                        params: HookToolInputLocalShell {
                            command: vec!["cargo".to_string(), "fmt".to_string()],
                            workdir: Some("codex-rs".to_string()),
                            timeout_ms: Some(60_000),
                            sandbox_permissions: Some(SandboxPermissions::UseDefault),
                            justification: None,
                            prefix_rule: None,
                        },
                    },
                    executed: true,
                    success: true,
                    duration_ms: 42,
                    mutating: true,
                    sandbox: "none".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    output_preview: "ok".to_string(),
                },
            },
        };

        let actual = serde_json::to_value(payload).expect("serialize hook payload");
        let expected = json!({
            "session_id": session_id.to_string(),
            "cwd": "tmp",
            "triggered_at": "2025-01-01T00:00:00Z",
            "hook_event": {
                "event_type": "after_tool_use",
                "turn_id": "turn-2",
                "call_id": "call-1",
                "tool_name": "local_shell",
                "tool_kind": "local_shell",
                "tool_input": {
                    "input_type": "local_shell",
                    "params": {
                        "command": ["cargo", "fmt"],
                        "workdir": "codex-rs",
                        "timeout_ms": 60000,
                        "sandbox_permissions": "use_default",
                        "justification": null,
                        "prefix_rule": null,
                    },
                },
                "executed": true,
                "success": true,
                "duration_ms": 42,
                "mutating": true,
                "sandbox": "none",
                "sandbox_policy": "danger-full-access",
                "output_preview": "ok",
            },
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn hooks_toml_defaults_to_empty_rules() {
        assert_eq!(
            HooksToml::default(),
            HooksToml {
                pre_tool_use: Vec::new(),
                post_tool_use: Vec::new(),
                user_prompt_submit: Vec::new(),
                stop: Vec::new(),
                session_end: Vec::new(),
                subagent_start: Vec::new(),
                subagent_stop: Vec::new(),
            }
        );
    }

    #[test]
    fn hooks_toml_deserializes_rule_lists() {
        let toml = r#"
            [[pre_tool_use]]
            matcher = "Bash"

            [[pre_tool_use.commands]]
            command = "/tmp/hook"
            timeout_sec = 5
        "#;
        let actual: HooksToml = toml::from_str(toml).expect("deserialize hooks");
        assert_eq!(
            actual,
            HooksToml {
                pre_tool_use: vec![HookRuleConfig {
                    matcher: Some("Bash".to_string()),
                    commands: vec![CommandHookConfig {
                        command: "/tmp/hook".to_string(),
                        timeout_sec: Some(5),
                    }],
                }],
                ..HooksToml::default()
            }
        );
    }
}
