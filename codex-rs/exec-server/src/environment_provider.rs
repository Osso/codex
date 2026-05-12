use async_trait::async_trait;

use crate::Environment;
use crate::ExecServerError;
use crate::environment::CODEX_EXEC_SERVER_URL_ENV_VAR;
use crate::environment::LOCAL_ENVIRONMENT_ID;

/// Lists the concrete environments available to Codex.
///
/// Implementations own a startup snapshot containing both the available
/// environment list in configured order and the default environment
/// selection. Providers can return provider-owned environments; `include_local`
/// controls whether `EnvironmentManager` should add the local environment to
/// the snapshot.
#[async_trait]
pub trait EnvironmentProvider: Send + Sync {
    /// Returns the provider-owned environment startup snapshot.
    async fn snapshot(&self) -> Result<EnvironmentProviderSnapshot, ExecServerError>;
}

#[derive(Clone, Debug)]
pub struct EnvironmentProviderSnapshot {
    pub environments: Vec<(String, Environment)>,
    pub default: EnvironmentDefault,
    pub include_local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvironmentDefault {
    Disabled,
    EnvironmentId(String),
}

/// Default provider backed by `CODEX_EXEC_SERVER_URL`.
///
/// This fork treats any non-empty value other than `none` as a legacy remote
/// URL and ignores it, keeping the local environment as the default.
#[derive(Clone, Debug)]
pub struct DefaultEnvironmentProvider {
    exec_server_url: Option<String>,
}

impl DefaultEnvironmentProvider {
    /// Builds a provider from an already-read raw `CODEX_EXEC_SERVER_URL` value.
    pub fn new(exec_server_url: Option<String>) -> Self {
        Self { exec_server_url }
    }

    /// Builds a provider by reading `CODEX_EXEC_SERVER_URL`.
    pub fn from_env() -> Self {
        Self::new(std::env::var(CODEX_EXEC_SERVER_URL_ENV_VAR).ok())
    }

    pub(crate) fn snapshot_inner(&self) -> EnvironmentProviderSnapshot {
        let (_exec_server_url, disabled) = normalize_exec_server_url(self.exec_server_url.clone());
        let default = if disabled {
            EnvironmentDefault::Disabled
        } else {
            EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string())
        };

        EnvironmentProviderSnapshot {
            environments: Vec::new(),
            default,
            include_local: true,
        }
    }
}

#[async_trait]
impl EnvironmentProvider for DefaultEnvironmentProvider {
    async fn snapshot(&self) -> Result<EnvironmentProviderSnapshot, ExecServerError> {
        Ok(self.snapshot_inner())
    }
}

pub(crate) fn normalize_exec_server_url(exec_server_url: Option<String>) -> (Option<String>, bool) {
    match exec_server_url.as_deref().map(str::trim) {
        None | Some("") => (None, false),
        Some(url) if url.eq_ignore_ascii_case("none") => (None, true),
        Some(url) => (Some(url.to_string()), false),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn default_provider_requests_local_environment_when_url_is_missing() {
        let provider = DefaultEnvironmentProvider::new(/*exec_server_url*/ None);
        let snapshot = provider.snapshot().await.expect("environments");
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let environments: HashMap<_, _> = environments.into_iter().collect();

        assert!(include_local);
        assert!(!environments.contains_key(LOCAL_ENVIRONMENT_ID));
        assert_eq!(
            default,
            EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string())
        );
    }

    #[tokio::test]
    async fn default_provider_requests_local_environment_when_url_is_empty() {
        let provider = DefaultEnvironmentProvider::new(Some(String::new()));
        let snapshot = provider.snapshot().await.expect("environments");
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let environments: HashMap<_, _> = environments.into_iter().collect();

        assert!(include_local);
        assert!(!environments.contains_key(LOCAL_ENVIRONMENT_ID));
        assert_eq!(
            default,
            EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string())
        );
    }

    #[tokio::test]
    async fn default_provider_omits_local_environment_for_none_value() {
        let provider = DefaultEnvironmentProvider::new(Some("none".to_string()));
        let snapshot = provider.snapshot().await.expect("environments");
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let environments: HashMap<_, _> = environments.into_iter().collect();

        assert!(include_local);
        assert!(!environments.contains_key(LOCAL_ENVIRONMENT_ID));
        assert_eq!(default, EnvironmentDefault::Disabled);
    }

    #[tokio::test]
    async fn default_provider_ignores_websocket_url() {
        let provider = DefaultEnvironmentProvider::new(Some("ws://127.0.0.1:8765".to_string()));
        let snapshot = provider.snapshot().await.expect("environments");
        let EnvironmentProviderSnapshot {
            environments,
            default,
            include_local,
        } = snapshot;
        let environments: HashMap<_, _> = environments.into_iter().collect();

        assert!(include_local);
        assert!(!environments.contains_key(LOCAL_ENVIRONMENT_ID));
        assert_eq!(
            default,
            EnvironmentDefault::EnvironmentId(LOCAL_ENVIRONMENT_ID.to_string())
        );
    }

    #[tokio::test]
    async fn default_provider_ignores_normalized_exec_server_url() {
        let provider = DefaultEnvironmentProvider::new(Some(" ws://127.0.0.1:8765 ".to_string()));
        let snapshot = provider.snapshot().await.expect("environments");
        let environments: HashMap<_, _> = snapshot.environments.into_iter().collect();

        assert!(snapshot.include_local);
        assert!(environments.is_empty());
    }
}
