use std::env;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartContributor;
use codex_extension_api::ToolBundle;
use codex_extension_api::ToolContributor;

use crate::HostrunToolConfig;
use crate::embedded_hostrun_tool_bundle;
use crate::hostrun_tool_bundle;

pub const HOSTRUN_RUNNER_ENV: &str = "CODEX_HOSTRUN_RUNNER";
const HOSTRUN_JS_PACKAGE: &str = "@openai/codex-hostrun-js";

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

pub fn install_managed<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
) -> Result<PathBuf, HostrunRunnerLifecycleError> {
    let runner = if let Some(runner) = env::var_os(HOSTRUN_RUNNER_ENV) {
        PathBuf::from(runner)
    } else {
        HostrunRunnerLifecycle::managed_package().ensure_runner()?
    };
    install(registry, &runner);
    Ok(runner)
}

pub fn install_feature_gated<C>(registry: &mut ExtensionRegistryBuilder<C>, enabled: fn(&C) -> bool)
where
    C: Send + Sync + 'static,
{
    let contributor = Arc::new(HostrunFeatureGatedContributor { enabled });
    registry.thread_start_contributor(contributor.clone());
    registry.tool_contributor(contributor);
}

struct HostrunFeatureGatedContributor<C> {
    enabled: fn(&C) -> bool,
}

impl<C> ThreadStartContributor<C> for HostrunFeatureGatedContributor<C>
where
    C: 'static,
{
    fn contribute(&self, config: &C, _session_store: &ExtensionData, thread_store: &ExtensionData) {
        if !(self.enabled)(config) {
            thread_store.insert(HostrunFeatureState::disabled());
            return;
        }

        thread_store.insert(HostrunFeatureState::enabled());
    }
}

impl<C> ToolContributor for HostrunFeatureGatedContributor<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<ToolBundle> {
        let Some(state) = thread_store.get::<HostrunFeatureState>() else {
            return Vec::new();
        };
        if !state.enabled {
            return Vec::new();
        }

        vec![embedded_hostrun_tool_bundle()]
    }
}

#[derive(Clone, Debug)]
struct HostrunFeatureState {
    enabled: bool,
}

impl HostrunFeatureState {
    fn disabled() -> Self {
        Self { enabled: false }
    }

    fn enabled() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug)]
pub struct HostrunRunnerLifecycle {
    workspace_root: PathBuf,
    package_dir: PathBuf,
}

impl HostrunRunnerLifecycle {
    pub fn new(workspace_root: impl AsRef<Path>, package_dir: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
            package_dir: package_dir.as_ref().to_path_buf(),
        }
    }

    pub fn managed_package() -> Self {
        let hostrun_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = hostrun_crate
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| hostrun_crate.clone());
        Self::new(workspace_root, hostrun_crate.join("js"))
    }

    pub fn runner_path(&self) -> PathBuf {
        self.package_dir.join("dist").join("cli.js")
    }

    pub fn ensure_runner(&self) -> Result<PathBuf, HostrunRunnerLifecycleError> {
        let runner = self.runner_path();
        if runner.is_file() {
            return Ok(runner);
        }

        self.build_runner()?;
        if runner.is_file() {
            return Ok(runner);
        }

        Err(HostrunRunnerLifecycleError::RunnerMissingAfterBuild { path: runner })
    }

    fn build_runner(&self) -> Result<(), HostrunRunnerLifecycleError> {
        let output = Command::new("npx")
            .args(["pnpm", "--filter", HOSTRUN_JS_PACKAGE, "build"])
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|source| HostrunRunnerLifecycleError::BuildSpawnFailed { source })?;

        if output.status.success() {
            return Ok(());
        }

        Err(HostrunRunnerLifecycleError::BuildFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

#[derive(Debug)]
pub enum HostrunRunnerLifecycleError {
    BuildSpawnFailed { source: std::io::Error },
    BuildFailed { status: String, stderr: String },
    RunnerMissingAfterBuild { path: PathBuf },
}

impl fmt::Display for HostrunRunnerLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildSpawnFailed { source } => {
                write!(formatter, "failed to start Hostrun JS build: {source}")
            }
            Self::BuildFailed { status, stderr } => {
                write!(formatter, "Hostrun JS build failed with {status}: {stderr}")
            }
            Self::RunnerMissingAfterBuild { path } => {
                write!(
                    formatter,
                    "Hostrun JS build finished but runner is missing at {}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for HostrunRunnerLifecycleError {}

#[cfg(test)]
mod tests {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ToolContributor;
    use tempfile::TempDir;

    use super::HostrunRunnerLifecycle;
    use super::HostrunToolContributor;
    use super::install;
    use super::install_feature_gated;

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

    #[test]
    fn managed_lifecycle_uses_existing_built_runner() {
        let temp_dir = TempDir::new().expect("temp dir");
        let workspace_root = temp_dir.path().join("repo");
        let package_dir = workspace_root.join("codex-rs").join("hostrun").join("js");
        let runner = package_dir.join("dist").join("cli.js");
        std::fs::create_dir_all(runner.parent().expect("runner parent")).expect("create dist");
        std::fs::write(&runner, "#!/usr/bin/env node\n").expect("write runner");
        let lifecycle = HostrunRunnerLifecycle::new(&workspace_root, &package_dir);

        assert_eq!(lifecycle.ensure_runner().expect("runner exists"), runner);
    }

    #[test]
    fn feature_gated_install_hides_hostrun_when_disabled() {
        let mut builder = ExtensionRegistryBuilder::<bool>::new();
        install_feature_gated(&mut builder, |enabled| *enabled);
        let registry = builder.build();
        let session_store = ExtensionData::new();
        let thread_store = ExtensionData::new();

        registry.thread_start_contributors()[0].contribute(&false, &session_store, &thread_store);
        let tools = registry.tool_contributors()[0].tools(&session_store, &thread_store);

        assert!(tools.is_empty());
    }
}
