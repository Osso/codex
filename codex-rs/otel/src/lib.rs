//! No-op compatibility crate for the removed OpenTelemetry implementation.
//!
//! The osso fork does not export telemetry, but several crates still use the
//! old `codex_otel` API surface for counters, timers, and validation helpers.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use codex_app_server_protocol::AuthMode;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use thiserror::Error;
use tracing::Span;

pub use codex_utils_string::sanitize_metric_tag_value;

pub const ORIGINATOR_TAG: &str = "originator";
pub const HOOK_RUN_METRIC: &str = "codex.hook.run";
pub const HOOK_RUN_DURATION_METRIC: &str = "codex.hook.run.duration_ms";
pub const STARTUP_PREWARM_AGE_AT_FIRST_TURN_METRIC: &str =
    "codex.session.startup_prewarm.age_at_first_turn_ms";
pub const STARTUP_PREWARM_DURATION_METRIC: &str = "codex.session.startup_prewarm.duration_ms";
pub const THREAD_STARTED_METRIC: &str = "codex.thread.started";
pub const THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC: &str =
    "codex.thread.skills.description_truncated_chars";
pub const THREAD_SKILLS_ENABLED_TOTAL_METRIC: &str = "codex.thread.skills.enabled_total";
pub const THREAD_SKILLS_KEPT_TOTAL_METRIC: &str = "codex.thread.skills.kept_total";
pub const THREAD_SKILLS_TRUNCATED_METRIC: &str = "codex.thread.skills.truncated";
pub const CURATED_PLUGINS_STARTUP_SYNC_METRIC: &str = "codex.plugins.startup_sync";
pub const CURATED_PLUGINS_STARTUP_SYNC_FINAL_METRIC: &str = "codex.plugins.startup_sync.final";
pub const GOAL_BUDGET_LIMITED_METRIC: &str = "codex.goal.budget_limited";
pub const GOAL_COMPLETED_METRIC: &str = "codex.goal.completed";
pub const GOAL_CREATED_METRIC: &str = "codex.goal.created";
pub const GOAL_DURATION_SECONDS_METRIC: &str = "codex.goal.duration_seconds";
pub const GOAL_TOKEN_COUNT_METRIC: &str = "codex.goal.token_count";
pub const TOOL_CALL_UNIFIED_EXEC_METRIC: &str = "codex.tool_call.unified_exec";
pub const TURN_E2E_DURATION_METRIC: &str = "codex.turn.e2e_duration_ms";
pub const TURN_MEMORY_METRIC: &str = "codex.turn.memory";
pub const TURN_NETWORK_PROXY_METRIC: &str = "codex.turn.network_proxy";
pub const TURN_TOKEN_USAGE_METRIC: &str = "codex.turn.token_usage";
pub const TURN_TOOL_CALL_METRIC: &str = "codex.turn.tool_call";
pub const TURN_TTFM_DURATION_METRIC: &str = "codex.turn.ttfm_ms";
pub const TURN_TTFT_DURATION_METRIC: &str = "codex.turn.ttft_ms";

pub fn bounded_originator_tag_value(value: &str) -> &'static str {
    Box::leak(sanitize_metric_tag_value(value).into_boxed_str())
}

#[derive(Debug, Clone)]
pub enum ToolDecisionSource {
    AutomatedReviewer,
    Config,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryAuthMode {
    ApiKey,
    Chatgpt,
}

impl fmt::Display for TelemetryAuthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey => f.write_str("apikey"),
            Self::Chatgpt => f.write_str("chatgpt"),
        }
    }
}

impl From<AuthMode> for TelemetryAuthMode {
    fn from(value: AuthMode) -> Self {
        match value {
            AuthMode::ApiKey => Self::ApiKey,
            AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens | AuthMode::AgentIdentity => {
                Self::Chatgpt
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthEnvTelemetryMetadata {
    pub openai_api_key_env_present: bool,
    pub codex_api_key_env_present: bool,
    pub codex_api_key_env_enabled: bool,
    pub provider_env_key_name: Option<String>,
    pub provider_env_key_present: Option<bool>,
    pub refresh_token_url_override_present: bool,
}

#[derive(Debug, Default)]
pub struct Timer;

impl Timer {
    pub fn record(self, _tags: &[(&str, &str)]) {}
}

#[derive(Debug, Clone)]
pub struct MetricsClient;

impl MetricsClient {
    pub fn new(_config: MetricsConfig) -> Result<Self, MetricsError> {
        Ok(Self)
    }

    pub fn counter(
        &self,
        _name: &str,
        _inc: i64,
        _tags: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        Ok(())
    }

    pub fn record_duration(
        &self,
        _name: &str,
        _duration: Duration,
        _tags: &[(&str, &str)],
    ) -> Result<(), MetricsError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MetricsConfig;

impl MetricsConfig {
    pub fn in_memory(
        _name: &str,
        _service_name: &str,
        _service_version: &str,
        _exporter: MetricsExporterKind,
    ) -> Self {
        Self
    }
}

#[derive(Debug, Clone)]
pub enum MetricsExporterKind {
    InMemory,
}

#[derive(Debug, Error)]
#[error("metrics disabled")]
pub struct MetricsError;

#[derive(Debug, Clone, Default)]
pub struct SessionTelemetry;

#[allow(clippy::too_many_arguments)]
impl SessionTelemetry {
    pub fn new(
        _conversation_id: ThreadId,
        _model: &str,
        _slug: &str,
        _account_id: Option<String>,
        _account_email: Option<String>,
        _auth_mode: Option<TelemetryAuthMode>,
        _originator: String,
        _log_user_prompts: bool,
        _terminal_type: String,
        _session_source: SessionSource,
    ) -> Self {
        Self
    }

    pub fn with_auth_env(self, _auth_env: AuthEnvTelemetryMetadata) -> Self {
        self
    }

    pub fn with_model(self, _model: &str, _slug: &str) -> Self {
        self
    }

    pub fn with_metrics_service_name(self, _service_name: &str) -> Self {
        self
    }

    pub fn with_metrics(self, _metrics: MetricsClient) -> Self {
        self
    }

    pub fn with_metrics_without_metadata_tags(self, _metrics: MetricsClient) -> Self {
        self
    }

    pub fn with_metrics_config(self, _config: MetricsConfig) -> Result<Self, MetricsError> {
        Ok(self)
    }

    pub fn with_provider_metrics(self, _provider: &OtelProvider) -> Self {
        self
    }

    pub fn counter(&self, _name: &str, _inc: i64, _tags: &[(&str, &str)]) {}

    pub fn histogram(&self, _name: &str, _value: i64, _tags: &[(&str, &str)]) {}

    pub fn record_duration(&self, _name: &str, _duration: Duration, _tags: &[(&str, &str)]) {}

    pub fn record_startup_phase(&self, _phase: &str, _duration: Duration, _status: Option<&str>) {}

    pub fn start_timer(&self, _name: &str, _tags: &[(&str, &str)]) -> Result<Timer, MetricsError> {
        Err(MetricsError)
    }

    pub fn shutdown_metrics(&self) -> Result<(), MetricsError> {
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_api_request(
        &self,
        _attempt: u64,
        _status: Option<u16>,
        _error: Option<&str>,
        _duration: Duration,
        _auth_header_attached: bool,
        _auth_header_name: Option<&str>,
        _retry_after_unauthorized: bool,
        _recovery_mode: Option<&str>,
        _recovery_phase: Option<&str>,
        _endpoint: &str,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_websocket_connect(
        &self,
        _duration: Duration,
        _status: Option<u16>,
        _error: Option<&str>,
        _auth_header_attached: bool,
        _auth_header_name: Option<&str>,
        _retry_after_unauthorized: bool,
        _recovery_mode: Option<&str>,
        _recovery_phase: Option<&str>,
        _endpoint: &str,
        _connection_reused: bool,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
    ) {
    }

    pub fn record_websocket_request(
        &self,
        _duration: Duration,
        _error: Option<&str>,
        _connection_reused: bool,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_auth_recovery(
        &self,
        _mode: &str,
        _phase: &str,
        _result: &str,
        _request_id: Option<&str>,
        _cf_ray: Option<&str>,
        _auth_error: Option<&str>,
        _auth_error_code: Option<&str>,
        _recovery_reason: Option<&str>,
        _auth_state_changed: Option<bool>,
    ) {
    }

    pub fn log_sse_event<E>(&self, _result: &E, _duration: Duration) {}

    pub fn see_event_completed_failed<E>(&self, _err: &E) {}

    pub fn sse_event_completed(
        &self,
        _input_tokens: i64,
        _output_tokens: i64,
        _cached_input_tokens: Option<i64>,
        _reasoning_output_tokens: Option<i64>,
        _total_tokens: i64,
    ) {
    }

    pub fn responses_type(&self) -> Option<&str> {
        None
    }

    pub fn snapshot_metrics(&self) -> Option<()> {
        None
    }

    pub fn user_prompt<T>(&self, _items: T) {}

    #[allow(clippy::too_many_arguments)]
    pub fn conversation_starts<A, B, C, D, E, F, G, H, I>(
        &self,
        _provider: A,
        _reasoning_effort: B,
        _reasoning_summary: C,
        _context_window: D,
        _auto_compact_limit: E,
        _approval_policy: F,
        _sandbox_policy: G,
        _mcp_servers: H,
        _active_profile: I,
    ) {
    }

    pub fn record_responses<A, B>(&self, _handle: A, _event: B) {}

    #[allow(clippy::too_many_arguments)]
    pub fn tool_result_with_tags<A, B, C, D, E, F>(
        &self,
        _tool_name: A,
        _call_id: B,
        _log_payload: C,
        _duration: D,
        _success: E,
        _message: F,
        _metric_tags: &[(&str, &str)],
        _extra_trace_fields: &[(&str, &str)],
    ) {
    }

    pub async fn log_tool_result_with_tags<A, B, C, F, Fut>(
        &self,
        _tool_name: A,
        _call_id: B,
        _log_payload: C,
        _metric_tags: &[(&str, &str)],
        _extra_trace_fields: &[(&str, &str)],
        f: F,
    ) -> Fut::Output
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future,
    {
        f().await
    }

    pub fn log_tool_failed(&self, _tool: &str, _msg: &str) {}

    pub fn tool_decision<A, B, C>(
        &self,
        _tool_name: A,
        _call_id: B,
        _decision: C,
        _source: ToolDecisionSource,
    ) {
    }

    pub fn record_websocket_event<A>(&self, _result: A, _duration: Duration) {}

    pub fn runtime_metrics_summary(&self) -> Option<RuntimeMetricsSummary> {
        None
    }

    pub fn reset_runtime_metrics(&self) {}
}

pub struct OtelProvider;

impl OtelProvider {
    pub fn logger_layer(&self) -> Option<()> {
        None
    }

    pub fn tracing_layer(&self) -> Option<()> {
        None
    }

    pub fn metrics(&self) -> Option<&MetricsClient> {
        None
    }

    pub fn shutdown(self) {}
}

pub fn global() -> Option<MetricsClient> {
    None
}

pub fn start_global_timer(_name: &str, _tags: &[(&str, &str)]) -> Option<Timer> {
    None
}

pub fn record_process_start_once(
    _metrics: &MetricsClient,
    _originator: &str,
) -> Result<(), MetricsError> {
    Ok(())
}

pub fn current_span_w3c_trace_context() -> Option<W3cTraceContext> {
    None
}

pub fn span_w3c_trace_context(_span: &Span) -> Option<W3cTraceContext> {
    None
}

pub fn current_span_trace_id() -> Option<String> {
    None
}

pub fn context_from_w3c_trace_context(_trace: &W3cTraceContext) -> Option<()> {
    None
}

pub fn set_parent_from_w3c_trace_context(_span: &Span, _trace: &W3cTraceContext) -> bool {
    false
}

pub fn set_parent_from_context(_span: &Span, _context: ()) {}

pub fn traceparent_context_from_env() -> Option<()> {
    None
}

pub fn validate_span_attributes(
    _attributes: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_tracestate_member(
    _member_key: &str,
    _fields: &BTreeMap<String, String>,
) -> Result<(), ValidationError> {
    Ok(())
}

pub fn validate_tracestate_entries(
    _entries: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), ValidationError> {
    Ok(())
}

#[derive(Debug, Error)]
#[error("invalid telemetry config")]
pub struct ValidationError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeMetricTotals {
    pub count: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RuntimeMetricsSummary {
    pub tool_calls: RuntimeMetricTotals,
    pub api_calls: RuntimeMetricTotals,
    pub streaming_events: RuntimeMetricTotals,
    pub websocket_calls: RuntimeMetricTotals,
    pub websocket_events: RuntimeMetricTotals,
    pub responses_api_overhead_ms: f64,
    pub responses_api_inference_time_ms: f64,
    pub responses_api_engine_iapi_ttft_ms: f64,
    pub responses_api_engine_service_ttft_ms: f64,
    pub responses_api_engine_iapi_tbt_ms: f64,
    pub responses_api_engine_service_tbt_ms: f64,
    pub turn_ttft_ms: f64,
    pub turn_ttfm_ms: f64,
}

impl std::ops::Add for RuntimeMetricsSummary {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            tool_calls: RuntimeMetricTotals {
                count: self.tool_calls.count + rhs.tool_calls.count,
                duration_ms: self.tool_calls.duration_ms + rhs.tool_calls.duration_ms,
            },
            api_calls: RuntimeMetricTotals {
                count: self.api_calls.count + rhs.api_calls.count,
                duration_ms: self.api_calls.duration_ms + rhs.api_calls.duration_ms,
            },
            streaming_events: RuntimeMetricTotals {
                count: self.streaming_events.count + rhs.streaming_events.count,
                duration_ms: self.streaming_events.duration_ms + rhs.streaming_events.duration_ms,
            },
            websocket_calls: RuntimeMetricTotals {
                count: self.websocket_calls.count + rhs.websocket_calls.count,
                duration_ms: self.websocket_calls.duration_ms + rhs.websocket_calls.duration_ms,
            },
            websocket_events: RuntimeMetricTotals {
                count: self.websocket_events.count + rhs.websocket_events.count,
                duration_ms: self.websocket_events.duration_ms + rhs.websocket_events.duration_ms,
            },
            responses_api_overhead_ms: self.responses_api_overhead_ms
                + rhs.responses_api_overhead_ms,
            responses_api_inference_time_ms: self.responses_api_inference_time_ms
                + rhs.responses_api_inference_time_ms,
            responses_api_engine_iapi_ttft_ms: self.responses_api_engine_iapi_ttft_ms
                + rhs.responses_api_engine_iapi_ttft_ms,
            responses_api_engine_service_ttft_ms: self.responses_api_engine_service_ttft_ms
                + rhs.responses_api_engine_service_ttft_ms,
            responses_api_engine_iapi_tbt_ms: self.responses_api_engine_iapi_tbt_ms
                + rhs.responses_api_engine_iapi_tbt_ms,
            responses_api_engine_service_tbt_ms: self.responses_api_engine_service_tbt_ms
                + rhs.responses_api_engine_service_tbt_ms,
            turn_ttft_ms: self.turn_ttft_ms + rhs.turn_ttft_ms,
            turn_ttfm_ms: self.turn_ttfm_ms + rhs.turn_ttfm_ms,
        }
    }
}

impl std::ops::AddAssign for RuntimeMetricsSummary {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl RuntimeMetricsSummary {
    pub fn merge(&mut self, rhs: Self) {
        *self += rhs;
    }

    pub fn responses_api_summary(&self) -> RuntimeMetricsSummary {
        *self
    }

    pub fn is_empty(&self) -> bool {
        self.tool_calls.count == 0
            && self.api_calls.count == 0
            && self.streaming_events.count == 0
            && self.websocket_calls.count == 0
            && self.websocket_events.count == 0
            && self.responses_api_overhead_ms == 0.0
            && self.responses_api_inference_time_ms == 0.0
            && self.responses_api_engine_iapi_ttft_ms == 0.0
            && self.responses_api_engine_service_ttft_ms == 0.0
            && self.responses_api_engine_iapi_tbt_ms == 0.0
            && self.responses_api_engine_service_tbt_ms == 0.0
            && self.turn_ttft_ms == 0.0
            && self.turn_ttfm_ms == 0.0
    }
}
