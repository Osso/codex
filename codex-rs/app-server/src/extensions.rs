use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use tracing::warn;

pub(crate) fn thread_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut builder = ExtensionRegistryBuilder::<Config>::new();
    codex_git_attribution::install(&mut builder);
    if let Err(error) = codex_hostrun::install_managed(&mut builder) {
        warn!("Hostrun tool unavailable: {error}");
    }
    Arc::new(builder.build())
}
