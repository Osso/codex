//! No-op stub shim for the rollout-trace crate.
//!
//! The full trace implementation has been removed. All public types remain
//! so that call-sites in `codex-core` continue to compile. Every operation
//! is a zero-cost no-op.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::SessionSource;
use serde::Deserialize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Conventional reduced-state cache name (kept for any remaining callers).
pub const REDUCED_STATE_FILE_NAME: &str = "state.json";

/// Environment variable that would have enabled trace-bundle recording.
pub const CODEX_ROLLOUT_TRACE_ROOT_ENV: &str = "CODEX_ROLLOUT_TRACE_ROOT";

// ---------------------------------------------------------------------------
// Payload / raw-event stubs
// ---------------------------------------------------------------------------

pub type RawPayloadId = String;
pub type RawEventSeq = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPayloadRef {
    pub raw_payload_id: RawPayloadId,
    pub kind: RawPayloadKind,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum RawPayloadKind {
    InferenceRequest,
    InferenceResponse,
    CompactionRequest,
    CompactionCheckpoint,
    CompactionResponse,
    ToolInvocation,
    ToolResult,
    ToolRuntimeEvent,
    TerminalRuntimeEvent,
    ProtocolEvent,
    SessionMetadata,
    AgentResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RawToolCallRequester {
    Model,
    CodeCell { runtime_cell_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTraceEvent {
    pub schema_version: u32,
    pub seq: RawEventSeq,
    pub wall_time_unix_ms: i64,
    pub rollout_id: String,
    pub thread_id: Option<String>,
    pub codex_turn_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawTraceEventContext {
    pub thread_id: Option<String>,
    pub codex_turn_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Reduced model types (kept for test-file compat)
// ---------------------------------------------------------------------------

pub type AgentThreadId = String;
pub type CodexTurnId = String;
pub type ConversationItemId = String;
pub type InferenceCallId = String;
pub type ToolCallId = String;
pub type ModelVisibleCallId = String;
pub type CodeModeRuntimeToolId = String;
pub type CodeCellId = String;
pub type CompactionId = String;
pub type CompactionRequestId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStatus {
    Running,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolCallRequester {
    Model,
    CodeCell { code_cell_id: CodeCellId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RolloutTrace {
    pub schema_version: u32,
    pub trace_id: String,
    pub rollout_id: String,
    pub started_at_unix_ms: i64,
    pub ended_at_unix_ms: Option<i64>,
    pub status: Option<RolloutStatus>,
    pub root_thread_id: AgentThreadId,
    pub tool_calls: BTreeMap<ToolCallId, ToolCall>,
    pub code_cells: BTreeMap<CodeCellId, ()>,
    pub raw_payloads: BTreeMap<RawPayloadId, RawPayloadRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_call_id: ToolCallId,
    pub model_visible_call_id: Option<ModelVisibleCallId>,
    pub code_mode_runtime_tool_id: Option<CodeModeRuntimeToolId>,
    pub requester: ToolCallRequester,
    pub execution: ExecutionWindow,
    pub raw_invocation_payload_id: Option<RawPayloadId>,
    pub raw_result_payload_id: Option<RawPayloadId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionWindow {
    pub started_at_unix_ms: i64,
    pub started_seq: RawEventSeq,
    pub ended_at_unix_ms: Option<i64>,
    pub ended_seq: Option<RawEventSeq>,
    pub status: ExecutionStatus,
}

// ---------------------------------------------------------------------------
// ThreadStartedTraceMetadata — kept because core and tests construct it
// ---------------------------------------------------------------------------

pub struct ThreadStartedTraceMetadata {
    pub thread_id: String,
    pub agent_path: String,
    pub task_name: Option<String>,
    pub nickname: Option<String>,
    pub agent_role: Option<String>,
    pub session_source: SessionSource,
    pub cwd: PathBuf,
    pub rollout_path: Option<PathBuf>,
    pub model: String,
    pub provider_name: String,
    pub approval_policy: String,
    pub sandbox_policy: String,
}

// ---------------------------------------------------------------------------
// AgentResultTracePayload — kept because core passes it to thread context
// ---------------------------------------------------------------------------

pub struct AgentResultTracePayload<'a> {
    pub child_agent_path: &'a str,
    pub message: &'a str,
    pub status: &'a AgentStatus,
}

// ---------------------------------------------------------------------------
// CompactionCheckpointTracePayload / CompactionTraceAttempt
// ---------------------------------------------------------------------------

pub struct CompactionCheckpointTracePayload<'a> {
    pub input_history: &'a [codex_protocol::models::ResponseItem],
    pub replacement_history: &'a [codex_protocol::models::ResponseItem],
}

#[derive(Clone, Debug)]
pub struct CompactionTraceAttempt;

impl CompactionTraceAttempt {
    /// Record the compaction result (ok or err).
    pub fn record_result<T>(&self, _result: T) {}
    pub fn record_failed(&self, _error: impl std::fmt::Display) {}
}

#[derive(Clone, Debug)]
pub struct CompactionTraceContext;

impl CompactionTraceContext {
    pub fn disabled() -> Self {
        Self
    }

    /// Start one compaction attempt. The payload is eagerly passed but ignored.
    pub fn start_attempt(&self, _payload: &impl Serialize) -> CompactionTraceAttempt {
        CompactionTraceAttempt
    }

    pub fn record_installed(&self, _payload: &CompactionCheckpointTracePayload<'_>) {}
}

// ---------------------------------------------------------------------------
// InferenceTraceAttempt / InferenceTraceContext
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct InferenceTraceAttempt;

impl InferenceTraceAttempt {
    pub fn disabled() -> Self {
        Self
    }

    pub fn record_started(&self, _payload: &impl Serialize) {}

    pub fn record_completed(
        &self,
        _response_id: &str,
        _token_usage: &Option<codex_protocol::protocol::TokenUsage>,
        _output_items: &[codex_protocol::models::ResponseItem],
    ) {
    }

    pub fn record_failed(&self, _error: impl std::fmt::Display) {}
}

#[derive(Clone, Debug)]
pub struct InferenceTraceContext;

impl InferenceTraceContext {
    pub fn disabled() -> Self {
        Self
    }

    pub fn start_attempt(&self) -> InferenceTraceAttempt {
        InferenceTraceAttempt
    }
}

// ---------------------------------------------------------------------------
// CodeCellTraceContext
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CodeCellTraceContext;

impl CodeCellTraceContext {
    pub fn record_initial_response(&self, _response: &codex_code_mode::RuntimeResponse) {}
    pub fn record_ended(&self, _response: &codex_code_mode::RuntimeResponse) {}
}

// ---------------------------------------------------------------------------
// ToolDispatch types
// ---------------------------------------------------------------------------

use codex_protocol::models::PermissionProfile;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::models::SearchToolCallParams;
use serde_json::Value as JsonValue;

pub struct ToolDispatchInvocation {
    pub thread_id: AgentThreadId,
    pub codex_turn_id: CodexTurnId,
    pub tool_call_id: ToolCallId,
    pub tool_name: String,
    pub tool_namespace: Option<String>,
    pub requester: ToolDispatchRequester,
    pub payload: ToolDispatchPayload,
}

pub enum ToolDispatchRequester {
    Model {
        model_visible_call_id: ModelVisibleCallId,
    },
    CodeCell {
        runtime_cell_id: String,
        runtime_tool_call_id: CodeModeRuntimeToolId,
    },
}

pub enum ToolDispatchPayload {
    Function {
        arguments: String,
    },
    ToolSearch {
        arguments: SearchToolCallParams,
    },
    Custom {
        input: String,
    },
    LocalShell {
        command: Vec<String>,
        workdir: Option<String>,
        timeout_ms: Option<u64>,
        sandbox_permissions: Option<SandboxPermissions>,
        prefix_rule: Option<Vec<String>>,
        additional_permissions: Option<PermissionProfile>,
        justification: Option<String>,
    },
    Mcp {
        server: String,
        tool: String,
        raw_arguments: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolDispatchResult {
    DirectResponse { response_item: ResponseInputItem },
    CodeModeResponse { value: JsonValue },
}

#[derive(Clone, Debug)]
pub struct ToolDispatchTraceContext;

impl ToolDispatchTraceContext {
    pub fn is_enabled(&self) -> bool {
        false
    }

    pub fn record_completed(&self, _status: ExecutionStatus, _result: ToolDispatchResult) {}

    pub fn record_failed(&self, _error: impl std::fmt::Display) {}
}

// ---------------------------------------------------------------------------
// ThreadTraceContext — the main no-op handle used everywhere in core
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ThreadTraceContext;

impl ThreadTraceContext {
    pub fn disabled() -> Self {
        Self
    }

    /// Test-only constructor. Returns disabled trace in stub form.
    pub fn start_root_or_disabled(_metadata: ThreadStartedTraceMetadata) -> Self {
        Self
    }

    /// Test-only constructor. Returns disabled trace in stub form.
    pub fn start_root_in_root_for_test(
        _root: &Path,
        _metadata: ThreadStartedTraceMetadata,
    ) -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn is_enabled(&self) -> bool {
        false
    }

    pub fn start_child_thread_trace_or_disabled(
        &self,
        _metadata: ThreadStartedTraceMetadata,
    ) -> Self {
        Self
    }

    pub fn record_ended(&self, _status: RolloutStatus) {}

    pub fn record_protocol_event(&self, _event: &codex_protocol::protocol::EventMsg) {}

    pub fn record_codex_turn_event(
        &self,
        _default_turn_id: &str,
        _event: &codex_protocol::protocol::EventMsg,
    ) {
    }

    pub fn record_tool_call_event(
        &self,
        _codex_turn_id: impl Into<CodexTurnId>,
        _event: &codex_protocol::protocol::EventMsg,
    ) {
    }

    pub fn record_agent_result_interaction(
        &self,
        _child_codex_turn_id: impl Into<CodexTurnId>,
        _parent_thread_id: impl Into<AgentThreadId>,
        _payload: &AgentResultTracePayload<'_>,
    ) {
    }

    pub fn record_codex_turn_started(&self, _codex_turn_id: impl Into<CodexTurnId>) {}

    pub fn start_code_cell_trace(
        &self,
        _codex_turn_id: impl Into<CodexTurnId>,
        _runtime_cell_id: impl Into<String>,
        _model_visible_call_id: impl Into<String>,
        _source_js: impl Into<String>,
    ) -> CodeCellTraceContext {
        CodeCellTraceContext
    }

    pub fn code_cell_trace_context(
        &self,
        _codex_turn_id: impl Into<CodexTurnId>,
        _runtime_cell_id: impl Into<String>,
    ) -> CodeCellTraceContext {
        CodeCellTraceContext
    }

    pub fn start_tool_dispatch_trace(
        &self,
        _invocation: impl FnOnce() -> Option<ToolDispatchInvocation>,
    ) -> ToolDispatchTraceContext {
        ToolDispatchTraceContext
    }

    pub fn inference_trace_context(
        &self,
        _codex_turn_id: impl Into<CodexTurnId>,
        _model: impl Into<String>,
        _provider_name: impl Into<String>,
    ) -> InferenceTraceContext {
        InferenceTraceContext
    }

    pub fn compaction_trace_context(
        &self,
        _codex_turn_id: impl Into<CodexTurnId>,
        _compaction_id: impl Into<CompactionId>,
        _model: impl Into<String>,
        _provider_name: impl Into<String>,
    ) -> CompactionTraceContext {
        CompactionTraceContext
    }
}

// ---------------------------------------------------------------------------
// replay_bundle stub — always errors; used only in deleted test code
// ---------------------------------------------------------------------------

pub fn replay_bundle(_bundle_dir: &Path) -> anyhow::Result<RolloutTrace> {
    anyhow::bail!("rollout trace implementation has been removed")
}
