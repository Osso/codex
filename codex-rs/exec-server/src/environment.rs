use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::ExecutorFileSystem;
use crate::HttpClient;
use crate::client::http_client::ReqwestHttpClient;
use crate::client_api::ExecServerTransportParams;
use crate::environment_provider::DefaultEnvironmentProvider;
use crate::environment_provider::EnvironmentDefault;
use crate::environment_provider::EnvironmentProvider;
use crate::environment_provider::EnvironmentProviderSnapshot;
use crate::environment_provider::normalize_exec_server_url;
use crate::environment_toml::environment_provider_from_codex_home;
use crate::local_file_system::LocalFileSystem;
use crate::local_process::LocalProcess;
use crate::process::ExecBackend;

pub const CODEX_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_EXEC_SERVER_URL";

/// Owns the execution/filesystem environments available to the Codex runtime.
///
/// `EnvironmentManager` is a shared registry for concrete environments. It
/// always creates a local environment under [`LOCAL_ENVIRONMENT_ID`] as the
/// default environment, unless `disabled` is set in `EnvironmentManagerArgs`
/// (or `CODEX_EXEC_SERVER_URL=none` in the environment).

///
/// When `disabled` is true the default environment is unset while the local
/// environment remains available for internal callers by id. Callers use

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
    /// When true, no default environment is set (tools disabled).
    pub disabled: bool,

    pub local_runtime_paths: ExecServerRuntimePaths,
}

impl EnvironmentManagerArgs {
    pub fn new(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        Self {
            disabled: false,
            local_runtime_paths,
        }
    }

    pub fn from_env(local_runtime_paths: ExecServerRuntimePaths) -> Self {
        // Honour CODEX_EXEC_SERVER_URL=none as a "disabled" signal for
        // backwards compatibility. Any non-empty, non-"none" value is ignored
        // now that the remote backend has been removed.
        let env_val = std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).unwrap_or_default();
        let disabled = env_val.trim().eq_ignore_ascii_case("none");
        Self {
            disabled,

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

    /// Builds a manager from args.
    pub fn new(args: EnvironmentManagerArgs) -> Self {

        let EnvironmentManagerArgs {
            disabled,

            local_runtime_paths,
        } = args;
        let environments = HashMap::from([(
            LOCAL_ENVIRONMENT_ID.to_string(),
            Arc::new(Environment::local(local_runtime_paths)),
        )]);
        let default_environment = if disabled {
            None
        } else {
            Some(LOCAL_ENVIRONMENT_ID.to_string())

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

    /// Adds or replaces a named remote environment without changing the
    /// manager's default environment selection.
    pub fn upsert_environment(
        &self,
        environment_id: String,
        exec_server_url: String,
    ) -> Result<(), ExecServerError> {
        if environment_id.is_empty() {
            return Err(ExecServerError::Protocol(
                "environment id cannot be empty".to_string(),
            ));
        }
        let (exec_server_url, disabled) = normalize_exec_server_url(Some(exec_server_url));
        if disabled {
            return Err(ExecServerError::Protocol(
                "remote environment cannot use disabled exec-server url".to_string(),
            ));
        }
        let Some(exec_server_url) = exec_server_url else {
            return Err(ExecServerError::Protocol(
                "remote environment requires an exec-server url".to_string(),
            ));
        };
        let environment = Environment::remote_inner(
            exec_server_url,
            self.local_environment.local_runtime_paths.clone(),
        );
        self.environments
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(environment_id, Arc::new(environment));
        Ok(())
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
    pub fn create(
        local_runtime_paths: ExecServerRuntimePaths,
    ) -> Result<Self, ExecServerError> {
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
    use super::LOCAL_ENVIRONMENT_ID;

    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::environment_provider::EnvironmentDefault;
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
            disabled: false,
            local_runtime_paths: test_runtime_paths(),
        });


        let environment = manager.default_environment().expect("default environment");
        assert_eq!(manager.default_environment_id(), Some(LOCAL_ENVIRONMENT_ID));
        assert!(Arc::ptr_eq(
            &environment,
            &manager
                .get_environment(LOCAL_ENVIRONMENT_ID)
                .expect("local environment")
                .is_remote()
        );

    }

    #[tokio::test]
    async fn environment_manager_treats_disabled_as_no_default() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            disabled: true,
            local_runtime_paths: test_runtime_paths(),
        });


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
            disabled: false,
            local_runtime_paths: runtime_paths.clone(),
        });


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
    async fn environment_manager_carries_local_runtime_paths() {
        let runtime_paths = test_runtime_paths();
        let manager = EnvironmentManager::create_for_tests(
            /*exec_server_url*/ None,
            runtime_paths.clone(),
        )
        .await;

        let environment = manager.local_environment();

        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            disabled: false,
            local_runtime_paths: environment

                .local_runtime_paths()
                .expect("local runtime paths")
                .clone(),
        )
        .await;
        let environment = manager.local_environment();
        assert_eq!(environment.local_runtime_paths(), Some(&runtime_paths));
    }

    #[tokio::test]
    async fn disabled_environment_manager_has_no_default_environment() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            disabled: true,
            local_runtime_paths: test_runtime_paths(),
        });


        assert!(manager.default_environment().is_none());
        assert_eq!(manager.default_environment_id(), None);
    }

    #[tokio::test]
    async fn environment_manager_keeps_local_lookup_when_default_disabled() {
        let manager = EnvironmentManager::new(EnvironmentManagerArgs {
            disabled: true,
            local_runtime_paths: test_runtime_paths(),
        });


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
    async fn environment_manager_upserts_named_remote_environment() {
        let manager = EnvironmentManager::disabled_for_tests(test_runtime_paths());

        manager
            .upsert_environment("executor-a".to_string(), "ws://127.0.0.1:8765".to_string())
            .expect("remote environment");
        let first = manager
            .get_environment("executor-a")
            .expect("first remote environment");
        assert!(first.is_remote());
        assert_eq!(first.exec_server_url(), Some("ws://127.0.0.1:8765"));
        assert_eq!(manager.default_environment_id(), None);

        manager
            .upsert_environment("executor-a".to_string(), "ws://127.0.0.1:9876".to_string())
            .expect("updated remote environment");
        let second = manager
            .get_environment("executor-a")
            .expect("second remote environment");
        assert!(second.is_remote());
        assert_eq!(second.exec_server_url(), Some("ws://127.0.0.1:9876"));
        assert!(!Arc::ptr_eq(&first, &second));
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
