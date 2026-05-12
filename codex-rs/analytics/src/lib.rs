//! No-op analytics stubs. The real telemetry pipeline has been removed;
//! all types and methods are preserved for API compatibility but do nothing.

use std::path::PathBuf;
use std::sync::Arc;

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerResponse;
use codex_login::AuthManager;
use codex_plugin::PluginTelemetryMetadata;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::protocol::GuardianAssessmentOutcome;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::GuardianRiskLevel;
use codex_protocol::protocol::GuardianUserAuthorization;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_protocol::protocol::SkillScope;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;

pub fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// TrackEventsContext
// ---------------------------------------------------------------------------
#[derive(Clone)]
pub struct TrackEventsContext {
    pub model_slug: String,
    pub thread_id: String,
    pub turn_id: String,
}

pub fn build_track_events_context(
    model_slug: String,
    thread_id: String,
    turn_id: String,
) -> TrackEventsContext {
    TrackEventsContext {
        model_slug,
        thread_id,
        turn_id,
    }
}

// ---------------------------------------------------------------------------
// InvocationType / SkillInvocation
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum InvocationType {
    Explicit,
    Implicit,
}

#[derive(Clone, Debug)]
pub struct SkillInvocation {
    pub skill_name: String,
    pub skill_scope: SkillScope,
    pub skill_path: PathBuf,
    pub invocation_type: InvocationType,
}

// ---------------------------------------------------------------------------
// AppInvocation / SubAgentThreadStartedInput
// ---------------------------------------------------------------------------
pub struct AppInvocation {
    pub connector_id: Option<String>,
    pub app_name: Option<String>,
    pub invocation_type: Option<InvocationType>,
}

#[derive(Clone)]
pub struct SubAgentThreadStartedInput {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub product_client_id: String,
    pub client_name: String,
    pub client_version: String,
    pub model: String,
    pub ephemeral: bool,
    pub subagent_source: SubAgentSource,
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// Compaction fact types
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum CompactionTrigger {
    Manual,
    Auto,
}

#[derive(Clone, Copy, Debug)]
pub enum CompactionReason {
    UserRequested,
    ContextLimit,
    ModelDownshift,
}

#[derive(Clone, Copy, Debug)]
pub enum CompactionImplementation {
    Responses,
    ResponsesCompact,
}

#[derive(Clone, Copy, Debug)]
pub enum CompactionPhase {
    StandaloneTurn,
    PreTurn,
    MidTurn,
}

#[derive(Clone, Copy, Debug)]
pub enum CompactionStrategy {
    Memento,
    PrefixCompaction,
}

#[derive(Clone, Copy, Debug)]
pub enum CompactionStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone)]
pub struct CodexCompactionEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub trigger: CompactionTrigger,
    pub reason: CompactionReason,
    pub implementation: CompactionImplementation,
    pub phase: CompactionPhase,
    pub strategy: CompactionStrategy,
    pub status: CompactionStatus,
    pub error: Option<String>,
    pub active_context_tokens_before: i64,
    pub active_context_tokens_after: i64,
    pub started_at: u64,
    pub completed_at: u64,
    pub duration_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Turn fact types
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum TurnSubmissionType {
    Default,
    Queued,
}

#[derive(Clone)]
pub struct TurnResolvedConfigFact {
    pub turn_id: String,
    pub thread_id: String,
    pub num_input_images: usize,
    pub submission_type: Option<TurnSubmissionType>,
    pub ephemeral: bool,
    pub session_source: codex_protocol::protocol::SessionSource,
    pub model: String,
    pub model_provider: String,
    pub sandbox_policy: codex_protocol::protocol::SandboxPolicy,
    pub reasoning_effort: Option<codex_protocol::openai_models::ReasoningEffort>,
    pub reasoning_summary: Option<codex_protocol::config_types::ReasoningSummary>,
    pub service_tier: Option<codex_protocol::config_types::ServiceTier>,
    pub approval_policy: codex_protocol::protocol::AskForApproval,
    pub approvals_reviewer: codex_protocol::config_types::ApprovalsReviewer,
    pub sandbox_network_access: bool,
    pub collaboration_mode: codex_protocol::config_types::ModeKind,
    pub personality: Option<codex_protocol::config_types::Personality>,
    pub is_first_turn: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum ThreadInitializationMode {
    New,
    Forked,
    Resumed,
}

#[derive(Clone)]
pub struct TurnTokenUsageFact {
    pub turn_id: String,
    pub thread_id: String,
    pub token_usage: TokenUsage,
}

#[derive(Clone, Copy, Debug)]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug)]
pub enum TurnSteerResult {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug)]
pub enum TurnSteerRejectionReason {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
    EmptyInput,
    InputTooLarge,
}

#[derive(Clone)]
pub struct CodexTurnSteerEvent {
    pub expected_turn_id: Option<String>,
    pub accepted_turn_id: Option<String>,
    pub num_input_images: usize,
    pub result: TurnSteerResult,
    pub rejection_reason: Option<TurnSteerRejectionReason>,
    pub created_at: u64,
}

// ---------------------------------------------------------------------------
// JSON-RPC error types
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum AnalyticsJsonRpcError {
    TurnSteer(TurnSteerRequestError),
    Input(InputError),
}

#[derive(Clone, Copy, Debug)]
pub enum TurnSteerRequestError {
    NoActiveTurn,
    ExpectedTurnMismatch,
    NonSteerableReview,
    NonSteerableCompact,
}

#[derive(Clone, Copy, Debug)]
pub enum InputError {
    Empty,
    TooLarge,
}

impl From<TurnSteerRequestError> for TurnSteerRejectionReason {
    fn from(error: TurnSteerRequestError) -> Self {
        match error {
            TurnSteerRequestError::NoActiveTurn => Self::NoActiveTurn,
            TurnSteerRequestError::ExpectedTurnMismatch => Self::ExpectedTurnMismatch,
            TurnSteerRequestError::NonSteerableReview => Self::NonSteerableReview,
            TurnSteerRequestError::NonSteerableCompact => Self::NonSteerableCompact,
        }
    }
}

impl From<InputError> for TurnSteerRejectionReason {
    fn from(error: InputError) -> Self {
        match error {
            InputError::Empty => Self::EmptyInput,
            InputError::TooLarge => Self::InputTooLarge,
        }
    }
}

// ---------------------------------------------------------------------------
// Hook fact type
// ---------------------------------------------------------------------------
pub struct HookRunFact {
    pub event_name: HookEventName,
    pub hook_source: HookSource,
    pub status: HookRunStatus,
}

// ---------------------------------------------------------------------------
// AppServerRpcTransport
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum AppServerRpcTransport {
    Stdio,
    Websocket,
    InProcess,
}

// ---------------------------------------------------------------------------
// Guardian review types
// ---------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub enum GuardianReviewDecision {
    Approved,
    Denied,
    Aborted,
}

#[derive(Clone, Copy, Debug)]
pub enum GuardianReviewTerminalStatus {
    Approved,
    Denied,
    Aborted,
    TimedOut,
    FailedClosed,
}

#[derive(Clone, Copy, Debug)]
pub enum GuardianReviewFailureReason {
    Timeout,
    Cancelled,
    PromptBuildError,
    SessionError,
    ParseError,
}

#[derive(Clone, Copy, Debug)]
pub enum GuardianReviewSessionKind {
    TrunkNew,
    TrunkReused,
    EphemeralForked,
}

#[derive(Clone, Copy, Debug)]
pub enum GuardianApprovalRequestSource {
    MainTurn,
    DelegatedSubagent,
}

#[derive(Clone, Debug)]
pub enum GuardianReviewedAction {
    Shell {
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
    },
    UnifiedExec {
        sandbox_permissions: SandboxPermissions,
        additional_permissions: Option<AdditionalPermissionProfile>,
        tty: bool,
    },
    Execve {
        source: GuardianCommandSource,
        program: String,
        additional_permissions: Option<AdditionalPermissionProfile>,
    },
    ApplyPatch {},
    NetworkAccess {
        protocol: NetworkApprovalProtocol,
        port: u16,
    },
    McpToolCall {
        server: String,
        tool_name: String,
        connector_id: Option<String>,
        connector_name: Option<String>,
        tool_title: Option<String>,
    },
    RequestPermissions {},
}

#[derive(Debug)]
pub struct GuardianReviewAnalyticsResult {
    pub decision: GuardianReviewDecision,
    pub terminal_status: GuardianReviewTerminalStatus,
    pub failure_reason: Option<GuardianReviewFailureReason>,
    pub risk_level: Option<GuardianRiskLevel>,
    pub user_authorization: Option<GuardianUserAuthorization>,
    pub outcome: Option<GuardianAssessmentOutcome>,
    pub guardian_thread_id: Option<String>,
    pub guardian_session_kind: Option<GuardianReviewSessionKind>,
    pub guardian_model: Option<String>,
    pub guardian_reasoning_effort: Option<String>,
    pub had_prior_review_context: Option<bool>,
    pub reviewed_action_truncated: bool,
    pub token_usage: Option<TokenUsage>,
    pub time_to_first_token_ms: Option<u64>,
}

impl GuardianReviewAnalyticsResult {
    pub fn without_session() -> Self {
        Self {
            decision: GuardianReviewDecision::Denied,
            terminal_status: GuardianReviewTerminalStatus::FailedClosed,
            failure_reason: None,
            risk_level: None,
            user_authorization: None,
            outcome: None,
            guardian_thread_id: None,
            guardian_session_kind: None,
            guardian_model: None,
            guardian_reasoning_effort: None,
            had_prior_review_context: None,
            reviewed_action_truncated: false,
            token_usage: None,
            time_to_first_token_ms: None,
        }
    }

    pub fn from_session(
        guardian_thread_id: String,
        guardian_session_kind: GuardianReviewSessionKind,
        guardian_model: String,
        guardian_reasoning_effort: Option<String>,
        had_prior_review_context: bool,
    ) -> Self {
        Self {
            guardian_thread_id: Some(guardian_thread_id),
            guardian_session_kind: Some(guardian_session_kind),
            guardian_model: Some(guardian_model),
            guardian_reasoning_effort,
            had_prior_review_context: Some(had_prior_review_context),
            ..Self::without_session()
        }
    }
}

#[allow(dead_code)]
pub struct GuardianReviewTrackContext {
    thread_id: String,
    turn_id: String,
    review_id: String,
    target_item_id: Option<String>,
    approval_request_source: GuardianApprovalRequestSource,
    reviewed_action: GuardianReviewedAction,
    review_timeout_ms: u64,
    started_at: u64,
}

impl GuardianReviewTrackContext {
    pub fn new(
        thread_id: String,
        turn_id: String,
        review_id: String,
        target_item_id: Option<String>,
        approval_request_source: GuardianApprovalRequestSource,
        reviewed_action: GuardianReviewedAction,
        review_timeout_ms: u64,
    ) -> Self {
        Self {
            thread_id,
            turn_id,
            review_id,
            target_item_id,
            approval_request_source,
            reviewed_action,
            review_timeout_ms,
            started_at: now_unix_seconds(),
        }
    }
}

// ---------------------------------------------------------------------------
// AnalyticsEventsClient (no-op stub)
// ---------------------------------------------------------------------------
#[derive(Clone, Default)]
pub struct AnalyticsEventsClient;

impl AnalyticsEventsClient {
    pub fn new(
        _auth_manager: Arc<AuthManager>,
        _base_url: String,
        _analytics_enabled: Option<bool>,
    ) -> Self {
        Self
    }

    pub fn track_skill_invocations(
        &self,
        _tracking: TrackEventsContext,
        _invocations: Vec<SkillInvocation>,
    ) {
    }

    pub fn track_initialize(
        &self,
        _connection_id: u64,
        _params: InitializeParams,
        _product_client_id: String,
        _rpc_transport: AppServerRpcTransport,
    ) {
    }

    pub fn track_subagent_thread_started(&self, _input: SubAgentThreadStartedInput) {}

    pub fn track_guardian_review(
        &self,
        _tracking: &GuardianReviewTrackContext,
        _result: GuardianReviewAnalyticsResult,
    ) {
    }

    pub fn track_app_mentioned(
        &self,
        _tracking: TrackEventsContext,
        _mentions: Vec<AppInvocation>,
    ) {
    }

    pub fn track_request(
        &self,
        _connection_id: u64,
        _request_id: RequestId,
        _request: &ClientRequest,
    ) {
    }

    pub fn track_app_used(&self, _tracking: TrackEventsContext, _app: AppInvocation) {}

    pub fn track_hook_run(&self, _tracking: TrackEventsContext, _hook: HookRunFact) {}

    pub fn track_plugin_used(
        &self,
        _tracking: TrackEventsContext,
        _plugin: PluginTelemetryMetadata,
    ) {
    }

    pub fn track_compaction(&self, _event: CodexCompactionEvent) {}

    pub fn track_turn_resolved_config(&self, _fact: TurnResolvedConfigFact) {}

    pub fn track_turn_token_usage(&self, _fact: TurnTokenUsageFact) {}

    pub fn track_plugin_installed(&self, _plugin: PluginTelemetryMetadata) {}

    pub fn track_plugin_uninstalled(&self, _plugin: PluginTelemetryMetadata) {}

    pub fn track_plugin_enabled(&self, _plugin: PluginTelemetryMetadata) {}

    pub fn track_plugin_disabled(&self, _plugin: PluginTelemetryMetadata) {}

    pub fn track_response(
        &self,
        _connection_id: u64,
        _request_id: RequestId,
        _response: ClientResponsePayload,
    ) {
    }

    pub fn track_server_request(&self, _connection_id: u64, _request: ServerRequest) {}

    pub fn track_server_response(&self, _completed_at_ms: u64, _response: ServerResponse) {}

    pub fn track_server_request_aborted(&self, _completed_at_ms: u64, _request_id: RequestId) {}

    pub fn track_effective_permissions_approval_response<T>(
        &self,
        _completed_at_ms: u64,
        _request_id: RequestId,
        _response: T,
    ) {
    }

    pub fn track_error_response(
        &self,
        _connection_id: u64,
        _request_id: RequestId,
        _error: JSONRPCErrorError,
        _error_type: Option<AnalyticsJsonRpcError>,
    ) {
    }

    pub fn track_notification(&self, _notification: ServerNotification) {}
}
