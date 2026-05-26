use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;

pub(crate) fn thread_extensions() -> Arc<ExtensionRegistry<Config>> {
    let mut builder = ExtensionRegistryBuilder::<Config>::new();
    codex_git_attribution::install(&mut builder);
    codex_hostrun::install_feature_gated(&mut builder, |config| {
        config.features.enabled(Feature::Hostrun)
    });
    Arc::new(builder.build())
}
