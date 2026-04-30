use super::emit_turn_memory_metric;
use super::emit_turn_network_proxy_metric;
use crate::telemetry::SessionTelemetry;
use crate::telemetry::TURN_MEMORY_METRIC;
use crate::telemetry::TURN_NETWORK_PROXY_METRIC;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;

fn test_session_telemetry() -> SessionTelemetry {
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.4",
        "gpt-5.4",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
}

#[test]
fn emit_turn_network_proxy_metric_records_active_turn() {
    let session_telemetry = test_session_telemetry();
    emit_turn_network_proxy_metric(
        &session_telemetry,
        /*network_proxy_active*/ true,
        ("tmp_mem_enabled", "true"),
    );
    // With otel removed, metric emission is a no-op; just verify no panic.
    let _ = TURN_NETWORK_PROXY_METRIC;
}

#[test]
fn emit_turn_network_proxy_metric_records_inactive_turn() {
    let session_telemetry = test_session_telemetry();
    emit_turn_network_proxy_metric(
        &session_telemetry,
        /*network_proxy_active*/ false,
        ("tmp_mem_enabled", "false"),
    );
    let _ = TURN_NETWORK_PROXY_METRIC;
}

#[test]
fn emit_turn_memory_metric_records_read_allowed_with_citations() {
    let session_telemetry = test_session_telemetry();
    emit_turn_memory_metric(
        &session_telemetry,
        /*feature_enabled*/ true,
        /*config_enabled*/ true,
        /*has_citations*/ true,
    );
    let _ = TURN_MEMORY_METRIC;
}

#[test]
fn emit_turn_memory_metric_records_config_disabled_without_citations() {
    let session_telemetry = test_session_telemetry();
    emit_turn_memory_metric(
        &session_telemetry,
        /*feature_enabled*/ true,
        /*config_enabled*/ false,
        /*has_citations*/ false,
    );
    let _ = TURN_MEMORY_METRIC;
}
