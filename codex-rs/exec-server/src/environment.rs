use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::HttpClient;
use crate::client::http_client::ReqwestHttpClient;
use crate::environment_provider::DefaultEnvironmentProvider;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProvider;
use crate::environment_provider::EnvironmentProviderSnapshot;
use crate::environment_toml::environment_provider_from_codex_home;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to the Codex runtime.
///
/// `EnvironmentManager` is a shared registry for concrete environments. Its
/// default constructor preserves this fork's local-only exec-server behavior
/// while provider-based construction accepts a provider-supplied snapshot.
///
/// Setting `CODEX_EXEC_SERVER_URL=none` disables environment access by leaving
/// the default environment unset while still keeping an explicit local
/// environment available through `local_environment()`. Callers use
/// `default_environment().is_some()` as the signal for model-facing
/// shell/filesystem tool availability.
#[derive(Debug)]
pub struct EnvironmentManager {
    default_environment: Option<String>,
    environments: RwLock<HashMap<String, Arc<Environment>>>,
    local_environment: Arc<Environment>,
}

pub const LOCAL_ENVIRONMENT_ID: &str = "local";

#[derive(Clone, Debug)]
pub struct EnvironmentManagerArgs {
    pub local_runtime_paths: ExecServerRuntimePaths,
}

impl EnvironmentManagerArgs {
    pub fn new(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            local_runtime_paths,
        }
    }
}

impl EnvironmentManager {
    /// Builds a test-only manager without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            default_environment: Some(LOCAL_ENVIRONMENT_ID.to_string()),
            environments: RwLock::new(HashMap::from([(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::new(Environment::default_for_tests()),
            )])),
            local_environment: Arc::new(Environment::default_for_tests()),
        }
    }

    /// Builds a test-only manager with environment access disabled.
    pub fn disabled_for_tests(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            default_environment: None,
            environments: RwLock::new(HashMap::new()),
            local_environment: Arc::new(Environment::local(local_runtime_paths)),
        }
    }

    /// Builds a test-only manager from a raw exec-server URL value.
    pub async fn create_for_tests(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        Self::from_default_provider_url(exec_server_url, local_runtime_paths).await
    }

    /// Builds a manager from `CODEX_EXEC_SERVER_URL` and local runtime paths
    /// used when creating local filesystem helpers.
    pub async fn new(args: EnvironmentManagerArgs) -> Self {
        let EnvironmentManagerArgs {
            local_runtime_paths,
        } = args;
        let exec_server_url = std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok();
        Self::from_default_provider_url(exec_server_url, local_runtime_paths).await
    }

    /// Builds a manager from `CODEX_HOME` and local runtime paths used when
    /// creating local filesystem helpers.
    ///
    /// If `CODEX_HOME/environments.toml` is present, it defines the configured
    /// environments. Otherwise this preserves the legacy
    /// `CODEX_EXEC_SERVER_URL` behavior.
    pub async fn from_codex_home(
        codex_home: impl AsRef<std::path::Path>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError> {
        let provider = environment_provider_from_codex_home(codex_home.as_ref())?;
        Self::from_provider(provider.as_ref(), local_runtime_paths).await
    }

    /// Builds a manager from the legacy environment-variable provider without
    /// reading user config files from `CODEX_HOME`.
    pub async fn from_env(
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError> {
        let provider = DefaultEnvironmentProvider::from_env();
        Self::from_provider(&provider, local_runtime_paths).await
    }

    async fn from_default_provider_url(
        exec_server_url: Option<String>,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Self {
        let provider = DefaultEnvironmentProvider::new(exec_server_url);
        match Self::from_provider(&provider, local_runtime_paths).await {
            Ok(manager) => manager,
            Err(err) => panic!("default provider should create valid environments: {err}"),
        }
    }

    /// Builds a manager from a provider-supplied startup snapshot.
    pub async fn from_provider<P>(
        provider: &P,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError>
    where
        P: EnvironmentProvider + ?Sized,
    {
        Self::from_provider_snapshot(provider.snapshot().await?, local_runtime_paths)
    }

    fn from_provider_snapshot(
        snapshot: EnvironmentProviderSnapshot,
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError> {
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let mut environment_map =
            HashMap::with_capacity(environments.len() + usize::from(include_local));
        let local_environment = Arc::new(Environment::local(local_runtime_paths));
        if include_local {
            environment_map.insert(
                LOCAL_ENVIRONMENT_ID.to_string(),
                Arc::clone(&local_environment),
            );
        }
        for (id, environment) in environments {
            if id.is_empty() {
                return Err(ExecServerError::Protocol(
                    "environment id cannot be empty".to_string(),
                ));
            }
            if id == LOCAL_ENVIRONMENT_ID {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{LOCAL_ENVIRONMENT_ID}` is reserved for EnvironmentManager"
                )));
            }
            if environment_map
                .insert(id.clone(), Arc::new(environment))
                .is_some()
            {
                return Err(ExecServerError::Protocol(format!(
                    "environment id `{id}` is duplicated"
                )));
            }
        }
        let default_environment = match default {
            EnvironmentDefault::Disabled => None,
            EnvironmentDefault::EnvironmentId(environment_id) => {
                if !environment_map.contains_key(&environment_id) {
                    return Err(ExecServerError::Protocol(format!(
                        "default environment `{environment_id}` is not configured"
                    )));
                }
                Some(environment_id)
            }
        };
        Ok(Self {
            default_environment,
            environments: RwLock::new(environment_map),
            local_environment,
        })
    }

    /// Returns the default environment instance.
    pub fn default_environment(&self) -> Option<Arc<Environment>> {
        self.default_environment
            .as_deref()
            .and_then(|environment_id| self.get_environment(environment_id))
    }

    /// Returns the id of the default environment.
    pub fn default_environment_id(&self) -> Option<&str> {
        self.default_environment.as_deref()
    }

    /// Returns the ordered environment ids used for new thread startup.
    pub fn default_environment_ids(&self) -> Vec<String> {
        let Some(default_environment_id) = self.default_environment.as_ref() else {
            return Vec::new();
        };
        let environments = self
            .environments
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut environment_ids = Vec::with_capacity(environments.len());
        environment_ids.push(default_environment_id.clone());
        environment_ids.extend(
            environments
                .keys()
                .filter(|environment_id| *environment_id != default_environment_id)
                .cloned(),
        );
        environment_ids
    }

    /// Returns the local environment instance used for internal runtime work.
    pub fn local_environment(&self) -> Arc<Environment> {
        Arc::clone(&self.local_environment)
    }

    /// Returns a named environment instance.
    pub fn get_environment(&self, environment_id: &str) -> Option<Arc<Environment>> {
        self.environments
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(environment_id)
            .cloned()
    }

    /// Rejects dynamic remote environment registration; this fork removed the
    /// exec-server remote backend.
    pub fn upsert_environment(
        &self,
        environment_id: String,
        exec_server_url: String,
    ) -> Result<(), ExecServerError> {
        let environment_id = environment_id.trim();
        if environment_id.is_empty() {
            return Err(ExecServerError::Protocol(
                "environment id cannot be empty".to_string(),
            ));
        }
        if exec_server_url.trim().is_empty() {
            return Err(ExecServerError::Protocol(
                "remote environment requires an exec-server url".to_string(),
            ));
        }
        Err(ExecServerError::Protocol(
            "remote exec-server environments are not supported by this fork".to_string(),
        ))
    }
}

/// Concrete execution/filesystem environment selected for a session.
///
/// This bundles the selected backend metadata together with the local runtime
/// paths used by filesystem helpers.
#[derive(Clone)]
pub struct Environment {
    exec_backend: Arc<dyn ExecBackend>,
    filesystem: Arc<dyn ExecutorFileSystem>,
    http_client: Arc<dyn HttpClient>,
    local_runtime_paths: Option<ExecServerRuntimePaths>,
}

impl Environment {
    /// Builds a test-only local environment without configured sandbox helper paths.
    pub fn default_for_tests() -> Self {
        Self {
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::unsandboxed()),
            http_client: Arc::new(ReqwestHttpClient),
            local_runtime_paths: None,
        }
    }
}

impl std::fmt::Debug for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Environment")
            .field("is_remote", &false)
            .finish_non_exhaustive()
    }
}

impl Environment {
    /// Builds an environment with the given local runtime paths.
    pub fn create(local_runtime_paths: ExecServerRuntimePaths) -> Result<Self, ExecServerError> {
        Ok(Self::local(local_runtime_paths))
    }

    /// Builds a test-only environment without configured sandbox helper paths.
    pub fn create_for_tests() -> Result<Self, ExecServerError> {
        Ok(Self::default_for_tests())
    }

    pub(crate) fn local(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            exec_backend: Arc::new(LocalProcess::default()),
            filesystem: Arc::new(LocalFileSystem::with_runtime_paths(
                local_runtime_paths.clone(),
            )),
            http_client: Arc::new(ReqwestHttpClient),
            local_runtime_paths: Some(local_runtime_paths),
        }
    }

    /// Always returns false — the remote backend has been removed.

    pub fn is_remote(&self) -> bool {
        false
    }

    pub fn local_runtime_paths(&self) -> Option<&ExecServerRuntimePaths> {
        self.local_runtime_paths.as_ref()
    }

    pub fn get_exec_backend(&self) -> Arc<dyn ExecBackend> {
        Arc::clone(&self.exec_backend)
    }

    pub fn get_http_client(&self) -> Arc<dyn HttpClient> {
        Arc::clone(&self.http_client)
    }

    pub fn get_filesystem(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.filesystem)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Environment;
    use super::EnvironmentManager;
    use super::EnvironmentManagerArgs;
    use super::LOCAL_ENVIRONMENT_ID;

    use crate::ExecServerError;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::environment_provider::EnvironmentDefault;
    use crate::environment_provider::EnvironmentProvider;
    use crate::environment_provider::EnvironmentProviderSnapshot;
    use pretty_assertions::assert_eq;

    struct TestEnvironmentProvider {
        snapshot: EnvironmentProviderSnapshot,
    }

    #[async_trait::async_trait]
    impl EnvironmentProvider for TestEnvironmentProvider {
        async fn snapshot(&self) -> Result<EnvironmentProviderSnapshot, ExecServerError> {
            Ok(self.snapshot.clone())
        }
    }

    fn test_runtime_paths() -> ExecServerRuntimePaths {
        ExecServerRuntimePaths::new(
            std::env::current_exe().expect("current exe"),
            /*codex_linux_sandbox_exe*/ None,
        )
        .expect("runtime paths")
    }

    #[tokio::test]
    async fn create_local_environment_does_not_connect() {
        let environment = Environment::create(test_runtime_paths()).expect("create environment");

        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn environment_manager_normalizes_empty_url() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            local_runtime_paths: test_runtime_paths(),
        })
        .await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
        ));
        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn environment_manager_treats_disabled_as_no_default() {
        let manager =
            EnvironmentManager::create_for_tests(Some("none".to_string()), test_runtime_paths())
                .await;

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn environment_manager_default_environment_caches_environment() {
        let manager = EnvironmentManager::default_for_tests();

        let first = manager.default_environment().expect("default environment");
        let second = manager.default_environment().expect("default environment");

        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(
            &first.get_filesystem(),
            &second.get_filesystem()
        ));
    }

    #[tokio::test]
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = test_runtime_paths();
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            local_runtime_paths: runtime_paths.clone(),
        })
        .await;

        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
        ));
        assert!(Arc::ptr_eq(&environment, &manager.local_environment()));
        assert!(!environment.is_remote());
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
    }

    #[tokio::test]
    async fn environment_manager_keeps_local_lookup_when_default_disabled() {
        let manager =
            EnvironmentManager::create_for_tests(Some("none".to_string()), test_runtime_paths())
                .await;

        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
        assert!(
            !manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );
    }

    #[tokio::test]
    async fn get_environment_returns_none_for_unknown_id() {
        let manager = EnvironmentManager::default_for_tests();

        assert!(manager.get_environment("does-not-exist").is_none());
    }

    #[tokio::test]
    async fn environment_manager_rejects_named_remote_environment() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        let err = manager
            .upsert_environment("executor-a".to_string(), "ws://127.0.0.1:8765".to_string())
            .expect_err("remote environment should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: remote exec-server environments are not supported by this fork"
        );
        assert!(manager.get_environment("executor-a").is_none());
    }

    #[tokio::test]
    async fn environment_manager_rejects_empty_remote_environment_url() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        let err = manager
            .upsert_environment("executor-a".to_string(), String::new())
            .expect_err("empty URL should fail");

        assert_eq!(
            err.to_string(),
            "exec-server protocol error: remote environment requires an exec-server url"
        );
    }

    #[tokio::test]
    async fn default_environment_has_ready_local_executor() {
        let environment = Environment::default_for_tests();

        let response = environment
            .get_exec_backend()
            .start(crate::ExecParams {
                process_id: ProcessId::from("default-env-proc"),
                argv: vec!["true".to_string()],
                cwd: std::env::current_dir().expect("read current dir"),
                env_policy: None,
                env: Default::default(),
                tty: false,
                pipe_stdin: false,
                arg0: None,
            })
            .await
            .expect("start process");

        assert_eq!(response.process.process_id().as_str(), "default-env-proc");
    }

    #[tokio::test]
    async fn test_environment_rejects_sandboxed_filesystem_without_runtime_paths() {
        let environment = Environment::default_for_tests();
        let path = codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(
            std::env::current_exe().expect("current exe").as_path(),
        )
        .expect("absolute current exe");
        let sandbox = crate::FileSystemSandboxContext::from_permission_profile(
            codex_protocol::models::PermissionProfile::from_runtime_permissions(
                &codex_protocol::permissions::FileSystemSandboxPolicy::restricted(Vec::new()),
                codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
            ),
        );

        let err = environment
            .get_filesystem()
            .read_file(&path, Some(&sandbox))
            .await
            .expect_err("sandboxed read should require runtime paths");

        assert_eq!(
            err.to_string(),
            "sandboxed filesystem operations require configured runtime paths"
        );
    }
}
