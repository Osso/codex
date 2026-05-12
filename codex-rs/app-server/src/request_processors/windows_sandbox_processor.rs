use super::*;

#[derive(Clone)]
pub(crate) struct WindowsSandboxRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    config_manager: ConfigManager,
}

impl WindowsSandboxRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        config_manager: ConfigManager,
    ) -> Self {
        Self {
            outgoing,
            config,
            config_manager,
        }
    }

    pub(crate) async fn windows_sandbox_readiness(
        &self,
    ) -> Result<WindowsSandboxReadinessResponse, JSONRPCErrorError> {
        Ok(determine_windows_sandbox_readiness(&self.config))
    }

    pub(crate) async fn windows_sandbox_setup_start(
        &self,
        request_id: &ConnectionRequestId,
        params: WindowsSandboxSetupStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.windows_sandbox_setup_start_inner(request_id, params)
            .await
            .map(|()| None)
    }

    async fn windows_sandbox_setup_start_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: WindowsSandboxSetupStartParams,
    ) -> Result<(), JSONRPCErrorError> {
        self.outgoing
            .send_response(
                request_id.clone(),
                WindowsSandboxSetupStartResponse { started: true },
            )
            .await;

        let mode = params.mode;
        let config = Arc::clone(&self.config);
        let config_manager = self.config_manager.clone();
        let command_cwd = params
            .cwd
            .map(PathBuf::from)
            .unwrap_or_else(|| config.cwd.to_path_buf());
        let outgoing = Arc::clone(&self.outgoing);
        let connection_id = request_id.connection_id;

        tokio::spawn(async move {
            let derived_config = config_manager
                .load_for_cwd(
                    /*request_overrides*/ None,
                    ConfigOverrides {
                        cwd: Some(command_cwd.clone()),
                        ..Default::default()
                    },
                    Some(command_cwd.clone()),
                )
                .await;
            let setup_result: anyhow::Result<()> = match derived_config {
                Ok(_config) => Err(anyhow::anyhow!(
                    "Windows sandbox setup is not supported by this fork"
                )),
                Err(err) => Err(err.into()),
            };
            let notification = WindowsSandboxSetupCompletedNotification {
                mode,
                success: setup_result.is_ok(),
                error: setup_result.err().map(|err| err.to_string()),
            };
            outgoing
                .send_server_notification_to_connections(
                    &[connection_id],
                    ServerNotification::WindowsSandboxSetupCompleted(notification),
                )
                .await;
        });
        Ok(())
    }
}

fn determine_windows_sandbox_readiness(config: &Config) -> WindowsSandboxReadinessResponse {
    if !cfg!(windows) {
        return WindowsSandboxReadinessResponse {
            status: WindowsSandboxReadiness::NotConfigured,
        };
    }

    determine_windows_sandbox_readiness_from_state(
        windows_sandbox_level_from_config(config),
        /*sandbox_setup_is_complete*/ false,
    )
}

fn determine_windows_sandbox_readiness_from_state(
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_setup_is_complete: bool,
) -> WindowsSandboxReadinessResponse {
    let status = match windows_sandbox_level {
        WindowsSandboxLevel::Disabled => WindowsSandboxReadiness::NotConfigured,
        WindowsSandboxLevel::RestrictedToken => WindowsSandboxReadiness::Ready,
        WindowsSandboxLevel::Elevated => {
            if sandbox_setup_is_complete {
                WindowsSandboxReadiness::Ready
            } else {
                WindowsSandboxReadiness::UpdateRequired
            }
        }
    };

    WindowsSandboxReadinessResponse { status }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determine_windows_sandbox_readiness_reports_not_configured_when_disabled() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Disabled,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::NotConfigured);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_ready_for_unelevated_mode() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::RestrictedToken,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::Ready);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_ready_for_complete_elevated_mode() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Elevated,
            /*sandbox_setup_is_complete*/ true,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::Ready);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_update_required_when_elevated_setup_is_stale() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Elevated,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::UpdateRequired);
    }
}
