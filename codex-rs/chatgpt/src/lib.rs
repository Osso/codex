//! No-op compatibility crate for removed ChatGPT workspace integrations.

pub mod workspace_settings {
    use std::sync::Arc;

    use codex_core::config::Config;
    use codex_login::CodexAuth;

    #[derive(Default)]
    pub struct WorkspaceSettingsCache;

    pub async fn codex_plugins_enabled_for_workspace(
        _config: &Config,
        _auth: Option<&CodexAuth>,
        _cache: Option<&Arc<WorkspaceSettingsCache>>,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }
}

pub mod connectors {
    use codex_app_server_protocol::AppInfo;
    use codex_core::config::Config;
    use codex_exec_server::EnvironmentManager;
    use codex_plugin::AppConnectorId;

    pub struct AccessibleConnectors {
        pub connectors: Vec<AppInfo>,
        pub codex_apps_ready: bool,
    }

    pub async fn list_all_connectors_with_options(
        _config: &Config,
        _force_refetch: bool,
    ) -> anyhow::Result<Vec<AppInfo>> {
        Ok(Vec::new())
    }

    pub async fn list_accessible_connectors_from_mcp_tools_with_environment_manager(
        _config: &Config,
        _force_refetch: bool,
        _environment_manager: &EnvironmentManager,
    ) -> anyhow::Result<AccessibleConnectors> {
        Ok(AccessibleConnectors {
            connectors: Vec::new(),
            codex_apps_ready: false,
        })
    }

    pub async fn list_cached_all_connectors(_config: &Config) -> Option<Vec<AppInfo>> {
        None
    }

    pub async fn list_cached_accessible_connectors_from_mcp_tools(
        _config: &Config,
    ) -> Option<Vec<AppInfo>> {
        None
    }

    pub fn connectors_for_plugin_apps(
        connectors: Vec<AppInfo>,
        plugin_apps: &[AppConnectorId],
    ) -> Vec<AppInfo> {
        if plugin_apps.is_empty() {
            return connectors;
        }
        connectors
            .into_iter()
            .filter(|connector| {
                plugin_apps
                    .iter()
                    .any(|plugin_app| plugin_app.0 == connector.id)
            })
            .collect()
    }

    pub fn merge_connectors_with_accessible(
        mut all_connectors: Vec<AppInfo>,
        accessible_connectors: Vec<AppInfo>,
        _all_connectors_loaded: bool,
    ) -> Vec<AppInfo> {
        all_connectors.extend(accessible_connectors);
        all_connectors
    }

    pub fn with_app_enabled_state(connectors: Vec<AppInfo>, _config: &Config) -> Vec<AppInfo> {
        connectors
    }
}
