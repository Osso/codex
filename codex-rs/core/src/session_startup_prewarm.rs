use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use crate::client::ModelClientSession;
use crate::session::INITIAL_SUBMIT_ID;
use crate::session::session::Session;
use crate::session::turn::build_prompt;
use crate::session::turn::built_tools;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::BaseInstructions;

pub(crate) struct SessionStartupPrewarmHandle {
    task: JoinHandle<CodexResult<ModelClientSession>>,
    started_at: Instant,
    timeout: Duration,
}

pub(crate) enum SessionStartupPrewarmResolution {
    Cancelled,
    Ready(Box<ModelClientSession>),
    Unavailable,
}

impl SessionStartupPrewarmHandle {
    pub(crate) fn new(
        task: JoinHandle<CodexResult<ModelClientSession>>,
        started_at: Instant,
        timeout: Duration,
    ) -> Self {
        Self {
            task,
            started_at,
            timeout,
        }
    }

    async fn resolve(
        self,
        cancellation_token: &CancellationToken,
    ) -> SessionStartupPrewarmResolution {
        let Self {
            mut task,
            started_at,
            timeout,
        } = self;
        let age_at_first_turn = started_at.elapsed();
        let remaining = timeout.saturating_sub(age_at_first_turn);

        if task.is_finished() {
            Self::resolution_from_join_result(task.await)
        } else {
            match tokio::select! {
                _ = cancellation_token.cancelled() => None,
                result = tokio::time::timeout(remaining, &mut task) => Some(result),
            } {
                Some(Ok(result)) => Self::resolution_from_join_result(result),
                Some(Err(_elapsed)) => {
                    task.abort();
                    info!("startup websocket prewarm timed out before the first turn could use it");
                    SessionStartupPrewarmResolution::Unavailable
                }
                None => {
                    task.abort();
                    SessionStartupPrewarmResolution::Cancelled
                }
            }
        }
    }

    fn resolution_from_join_result(
        result: std::result::Result<CodexResult<ModelClientSession>, tokio::task::JoinError>,
    ) -> SessionStartupPrewarmResolution {
        match result {
            Ok(Ok(prewarmed_session)) => {
                SessionStartupPrewarmResolution::Ready(Box::new(prewarmed_session))
            }
            Ok(Err(err)) => {
                warn!("startup websocket prewarm setup failed: {err:#}");
                SessionStartupPrewarmResolution::Unavailable
            }
            Err(err) => {
                warn!("startup websocket prewarm setup join failed: {err}");
                SessionStartupPrewarmResolution::Unavailable
            }
        }
    }
}

impl Session {
    pub(crate) async fn schedule_startup_prewarm(self: &Arc<Self>, base_instructions: String) {
        let websocket_connect_timeout = self.provider().await.websocket_connect_timeout();
        let started_at = Instant::now();
        let startup_prewarm_session = Arc::clone(self);
        let startup_prewarm = tokio::spawn(async move {
            schedule_startup_prewarm_inner(startup_prewarm_session, base_instructions).await
        });
        self.set_session_startup_prewarm(SessionStartupPrewarmHandle::new(
            startup_prewarm,
            started_at,
            websocket_connect_timeout,
        ))
        .await;
    }

    pub(crate) async fn consume_startup_prewarm_for_regular_turn(
        &self,
        cancellation_token: &CancellationToken,
    ) -> SessionStartupPrewarmResolution {
        let Some(startup_prewarm) = self.take_session_startup_prewarm().await else {
            return SessionStartupPrewarmResolution::Unavailable;
        };
        startup_prewarm.resolve(cancellation_token).await
    }
}

async fn schedule_startup_prewarm_inner(
    session: Arc<Session>,
    base_instructions: String,
) -> CodexResult<ModelClientSession> {
    let startup_turn_context = session
        .new_default_turn_with_sub_id(INITIAL_SUBMIT_ID.to_owned())
        .await;
    let startup_cancellation_token = CancellationToken::new();
    let startup_router = built_tools(
        session.as_ref(),
        startup_turn_context.as_ref(),
        &[],
        &HashSet::new(),
        /*skills_outcome*/ None,
        &startup_cancellation_token,
    )
    .await?;
    let startup_prompt = build_prompt(
        Vec::new(),
        startup_router.as_ref(),
        startup_turn_context.as_ref(),
        BaseInstructions {
            text: base_instructions,
        },
    );
    let startup_turn_metadata_header = startup_turn_context
        .turn_metadata_state
        .current_header_value();
    let mut client_session = session.services.model_client.new_session();
    client_session
        .prewarm_websocket(
            &startup_prompt,
            &startup_turn_context.model_info,
            &startup_turn_context.session_telemetry,
            startup_turn_context.reasoning_effort,
            startup_turn_context.reasoning_summary,
            startup_turn_context.config.service_tier.clone(),
            startup_turn_metadata_header.as_deref(),
        )
        .await?;

    Ok(client_session)
}
