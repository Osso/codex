use super::ConnectionSessionState;
use super::MessageProcessor;
use super::MessageProcessorArgs;
use crate::analytics_utils::analytics_events_client_from_config;
use crate::config_manager::ConfigManager;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::AppServerTransport;
use anyhow::Result;
use app_test_support::create_mock_responses_server_repeating_assistant;
use app_test_support::write_mock_responses_config_toml;
use codex_analytics::AppServerRpcTransport;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudRequirementsLoader;
use codex_config::LoaderOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::LoaderOverrides;
use codex_exec_server::EnvironmentManager;
use codex_login::AuthManager;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use pretty_assertions::assert_eq;
use serial_test::serial;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc;
use wiremock::MockServer;

const TEST_CONNECTION_ID: ConnectionId = ConnectionId(7);

fn request_from_client_request(request: ClientRequest) -> JSONRPCRequest {
    serde_json::from_value(serde_json::to_value(request).expect("serialize client request"))
        .expect("client request should convert to JSON-RPC")
}

struct TracingHarness {
    _server: MockServer,
    _codex_home: TempDir,
    processor: Arc<MessageProcessor>,
    outgoing_rx: mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
    session: Arc<ConnectionSessionState>,
}

impl TracingHarness {
    async fn new() -> Result<Self> {
        let server = create_mock_responses_server_repeating_assistant("Done").await;
        let codex_home = TempDir::new()?;
        let config = Arc::new(build_test_config(codex_home.path(), &server.uri()).await?);
        let (processor, outgoing_rx) = build_test_processor(config);
        let mut harness = Self {
            _server: server,
            _codex_home: codex_home,
            processor,
            outgoing_rx,
            session: Arc::new(ConnectionSessionState::new(origin)),
        };

        let _: InitializeResponse = harness
            .request(
                ClientRequest::Initialize {
                    request_id: RequestId::Integer(1),
                    params: InitializeParams {
                        client_info: ClientInfo {
                            name: "codex-app-server-tests".to_string(),
                            title: None,
                            version: "0.1.0".to_string(),
                        },
                        capabilities: Some(InitializeCapabilities {
                            experimental_api: true,
                            ..Default::default()
                        }),
                    },
                },
                /*trace*/ None,
            )
            .await;
        assert!(harness.session.initialized());

        Ok(harness)
    }

    async fn shutdown(self) {
        self.processor.shutdown_threads().await;
        self.processor.drain_background_tasks().await;
    }

    async fn request<T>(&mut self, request: ClientRequest, trace: Option<W3cTraceContext>) -> T
    where
        T: serde::de::DeserializeOwned,
    {
        let request_id = match request.id() {
            RequestId::Integer(request_id) => *request_id,
            request_id => panic!("expected integer request id in test harness, got {request_id:?}"),
        };
        let mut request = request_from_client_request(request);
        request.trace = trace;

        self.processor
            .process_request(
                TEST_CONNECTION_ID,
                request,
                &AppServerTransport::Stdio,
                Arc::clone(&self.session),
            )
            .await;
        read_response(&mut self.outgoing_rx, request_id).await
    }

    async fn start_thread(
        &mut self,
        request_id: i64,
        trace: Option<W3cTraceContext>,
    ) -> ThreadStartResponse {
        let response = self
            .request(
                ClientRequest::ThreadStart {
                    request_id: RequestId::Integer(request_id),
                    params: ThreadStartParams {
                        ephemeral: Some(true),
                        ..ThreadStartParams::default()
                    },
                },
                trace,
            )
            .await;
        read_thread_started_notification(&mut self.outgoing_rx).await;
        response
    }
}

async fn build_test_config(codex_home: &Path, server_uri: &str) -> Result<Config> {
    write_mock_responses_config_toml(
        codex_home,
        server_uri,
        &BTreeMap::new(),
        /*auto_compact_limit*/ 8_192,
        Some(false),
        "mock_provider",
        "compact",
    )?;

    Ok(ConfigBuilder::default()
        .codex_home(codex_home.to_path_buf())
        .build()
        .await?)
}

async fn build_test_processor(
    config: Arc<Config>,
) -> (
    Arc<MessageProcessor>,
    mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
) {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(16);
    let auth_manager =
        AuthManager::shared_from_config(config.as_ref(), /*enable_codex_api_key_env*/ false).await;
    let config_manager = ConfigManager::new(
        config.codex_home.to_path_buf(),
        Vec::new(),
        LoaderOverrides::default(),
        Arg0DispatchPaths::default(),
        Arc::new(codex_config::NoopThreadConfigLoader),
    );
    let analytics_events_client =
        analytics_events_client_from_config(Arc::clone(&auth_manager), config.as_ref());
    let outgoing = Arc::new(OutgoingMessageSender::new(
        outgoing_tx,
        analytics_events_client.clone(),
    ));
    let processor = Arc::new(MessageProcessor::new(MessageProcessorArgs {
        outgoing,
        analytics_events_client,
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        config_manager,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        log_db: None,
        state_db: None,
        config_warnings: Vec::new(),
        session_source: SessionSource::VSCode,
        auth_manager,
        installation_id: "11111111-1111-4111-8111-111111111111".to_string(),
        rpc_transport: AppServerRpcTransport::Stdio,
        remote_control_handle: None,
        plugin_startup_tasks: crate::PluginStartupTasks::Start,
    }));
    (processor, outgoing_rx)
}

async fn read_response<T: serde::de::DeserializeOwned>(
    outgoing_rx: &mut mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
    request_id: i64,
) -> T {
    loop {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), outgoing_rx.recv())
            .await
            .expect("timed out waiting for response")
            .expect("outgoing channel closed");
        let crate::outgoing_message::OutgoingEnvelope::ToConnection {
            connection_id,
            message,
            ..
        } = envelope
        else {
            continue;
        };
        if connection_id != TEST_CONNECTION_ID {
            continue;
        }
        let crate::outgoing_message::OutgoingMessage::Response(response) = message else {
            continue;
        };
        if response.id != RequestId::Integer(request_id) {
            continue;
        }
        return serde_json::from_value(response.result)
            .expect("response payload should deserialize");
    }
}

async fn read_thread_started_notification(
    outgoing_rx: &mut mpsc::Receiver<crate::outgoing_message::OutgoingEnvelope>,
) {
    loop {
        let envelope = tokio::time::timeout(std::time::Duration::from_secs(5), outgoing_rx.recv())
            .await
            .expect("timed out waiting for thread/started notification")
            .expect("outgoing channel closed");
        match envelope {
            crate::outgoing_message::OutgoingEnvelope::ToConnection {
                connection_id,
                message,
                ..
            } => {
                if connection_id != TEST_CONNECTION_ID {
                    continue;
                }
                let crate::outgoing_message::OutgoingMessage::AppServerNotification(notification) =
                    message
                else {
                    continue;
                };
                if matches!(
                    notification,
                    codex_app_server_protocol::ServerNotification::ThreadStarted(_)
                ) {
                    return;
                }
            }
            crate::outgoing_message::OutgoingEnvelope::Broadcast { message } => {
                let crate::outgoing_message::OutgoingMessage::AppServerNotification(notification) =
                    message
                else {
                    continue;
                };
                if matches!(
                    notification,
                    codex_app_server_protocol::ServerNotification::ThreadStarted(_)
                ) {
                    return;
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
#[serial(app_server_tracing)]
async fn remote_control_origin_rejects_device_key_requests() -> Result<()> {
    let mut harness = TracingHarness::new_with_origin(ConnectionOrigin::RemoteControl).await?;

    let error = harness
        .request_error(
            ClientRequest::DeviceKeySign {
                request_id: RequestId::Integer(20_004),
                params: DeviceKeySignParams {
                    key_id: "dk_123".to_string(),
                    payload: DeviceKeySignPayload::RemoteControlClientConnection {
                        nonce: "nonce-123".to_string(),
                        audience:
                            RemoteControlClientConnectionAudience::RemoteControlClientWebsocket,
                        session_id: "wssess_123".to_string(),
                        target_origin: "https://chatgpt.com".to_string(),
                        target_path: "/api/codex/remote/control/client".to_string(),
                        account_user_id: "acct_123".to_string(),
                        client_id: "cli_123".to_string(),
                        token_expires_at: 4_102_444_800,
                        token_sha256_base64url: "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU"
                            .to_string(),
                        scopes: vec!["remote_control_controller_websocket".to_string()],
                    },
                },
            },
            /*trace*/ None,
        )
        .await;

    assert_eq!(error.code, crate::error_code::INVALID_REQUEST_ERROR_CODE);
    assert_eq!(
        error.message,
        "device/key/sign is not available over remote transports"
    );

    harness.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[serial(app_server_tracing)]
async fn thread_start_creates_thread() -> Result<()> {
    let mut harness = TracingHarness::new().await?;
    let response: ThreadStartResponse = harness
        .start_thread(/*request_id*/ 20_002, /*trace*/ None)
        .await;
    assert!(!response.thread.id.is_empty());
    harness.shutdown().await;
    Ok(())
}
