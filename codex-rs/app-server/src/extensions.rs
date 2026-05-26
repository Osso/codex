use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;

pub(crate) fn thread_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut builder = ExtensionRegistryBuilder::<Config>::new();
    codex_git_attribution::install(&mut builder);
    codex_hostrun::install_from_env(&mut builder);
    Arc::new(builder.build())
}
