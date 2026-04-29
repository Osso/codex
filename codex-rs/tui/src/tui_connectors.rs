/// Shim that replaces the removed `codex-chatgpt::connectors` module.
///
/// The chatgpt.com directory listing (`list_all_connectors_with_options`) is
/// no longer available without the chatgpt crate; it returns an empty Vec.
/// The accessible-connectors path (MCP tools) is still functional and is
/// delegated directly to `codex-core::connectors`.
use std::collections::HashSet;

use codex_app_server_protocol::AppInfo;
use codex_connectors::filter::filter_disallowed_connectors;
use codex_connectors::merge::merge_connectors;
use codex_core::config::Config;
pub use codex_core::connectors::list_accessible_connectors_from_mcp_tools_with_options_and_status;
pub use codex_core::connectors::with_app_enabled_state;
use codex_login::default_client::originator;

/// Always returns an empty list — chatgpt.com directory enumeration removed.
pub async fn list_all_connectors_with_options(
    _config: &Config,
    _force_refetch: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    Ok(Vec::new())
}

pub fn merge_connectors_with_accessible(
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
    all_connectors_loaded: bool,
) -> Vec<AppInfo> {
    let accessible_connectors = if all_connectors_loaded {
        let connector_ids: HashSet<&str> = connectors
            .iter()
            .map(|connector| connector.id.as_str())
            .collect();
        accessible_connectors
            .into_iter()
            .filter(|connector| connector_ids.contains(connector.id.as_str()))
            .collect()
    } else {
        accessible_connectors
    };
    let merged = merge_connectors(connectors, accessible_connectors);
    filter_disallowed_connectors(merged, originator().value.as_str())
}
