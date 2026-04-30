//! No-op replacement for the former OpenTelemetry provider wiring.
//!
//! All public items are kept for API compatibility with callers; they now
//! always return `None` / no-op values so codex-otel can be removed as a
//! dependency.

use crate::config::Config;
use crate::telemetry::OtelProvider;

/// Build an OpenTelemetry provider from the app Config.
///
/// Always returns `None` now that the otel crate has been removed.
pub fn build_provider(
    _config: &Config,
    _service_version: &str,
    _service_name_override: Option<&str>,
    _default_analytics_enabled: bool,
) -> Result<Option<OtelProvider>, Box<dyn std::error::Error>> {
    Ok(None)
}

/// Filter predicate kept for API compatibility; always returns `false`.
pub fn codex_export_filter(_meta: &tracing::Metadata<'_>) -> bool {
    false
}

pub fn record_process_start(otel: Option<&OtelProvider>, originator: &str) {
    let Some(metrics) = otel.and_then(OtelProvider::metrics) else {
        return;
    };
    let _ = codex_otel::record_process_start_once(metrics, originator);
}

pub fn install_sqlite_telemetry(otel: Option<&OtelProvider>, originator: &str) {
    let Some(metrics) = otel.and_then(OtelProvider::metrics) else {
        return;
    };
    let telemetry = codex_rollout::sqlite_telemetry_recorder(metrics.clone(), originator);
    let _ = codex_state::install_process_db_telemetry(telemetry);
}
