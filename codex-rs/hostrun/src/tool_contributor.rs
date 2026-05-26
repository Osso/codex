use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolBundle;
use codex_extension_api::ToolContributor;

use crate::HostrunToolConfig;
use crate::hostrun_tool_bundle;

pub const HOSTRUN_RUNNER_ENV: &str = "CODEX_HOSTRUN_RUNNER";

#[derive(Clone, Debug)]
pub struct HostrunToolContributor {
    runner: PathBuf,
}

impl HostrunToolContributor {
    pub fn new(runner: impl AsRef<Path>) -> Self {
        Self {
            runner: runner.as_ref().to_path_buf(),
        }
    }
}

impl ToolContributor for HostrunToolContributor {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<ToolBundle> {
        vec![hostrun_tool_bundle(HostrunToolConfig::new(&self.runner))]
    }
}

pub fn install<C>(registry: &mut ExtensionRegistryBuilder<C>, runner: impl AsRef<Path>) {
    registry.tool_contributor(Arc::new(HostrunToolContributor::new(runner)));
}

pub fn install_from_env<C>(registry: &mut ExtensionRegistryBuilder<C>) {
    let Some(runner) = env::var_os(HOSTRUN_RUNNER_ENV) else {
        return;
    };
    install(registry, PathBuf::from(runner));
}

#[cfg(test)]
mod tests {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ToolContributor;

    use super::HostrunToolContributor;
    use super::install;

    #[test]
    fn contributor_returns_hostrun_eval_bundle() {
        let contributor = HostrunToolContributor::new("/tmp/hostrun-runner");

        let tools = contributor.tools(&ExtensionData::new(), &ExtensionData::new());

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_name(), "hostrun_eval");
    }

    #[test]
    fn install_adds_hostrun_tool_contributor() {
        let mut builder = ExtensionRegistryBuilder::<()>::new();
        install(&mut builder, "/tmp/hostrun-runner");
        let registry = builder.build();

        let tools =
            registry.tool_contributors()[0].tools(&ExtensionData::new(), &ExtensionData::new());

        assert_eq!(tools[0].tool_name(), "hostrun_eval");
    }
}
