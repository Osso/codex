use codex_app_server_protocol::AppSummary;
use codex_core::config::Config;
use codex_core::plugins::AppConnectorId;
use codex_exec_server::EnvironmentManager;

pub(super) async fn load_plugin_app_summaries(
    _config: &Config,
    _plugin_apps: &[AppConnectorId],
    _environment_manager: &EnvironmentManager,
) -> Vec<AppSummary> {
    Vec::new()
}
