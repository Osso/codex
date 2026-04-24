use super::*;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout_at;

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = WaitAgentResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            call_id,
            ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: WaitArgs = parse_arguments(&arguments)?;
        let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
        let timeout_ms = match timeout_ms {
            ms if ms <= 0 => {
                return Err(FunctionCallError::RespondToModel(
                    "timeout_ms must be greater than zero".to_owned(),
                ));
            }
            ms => ms.clamp(MIN_WAIT_TIMEOUT_MS, MAX_WAIT_TIMEOUT_MS),
        };

        let mut mailbox_seq_rx = session.subscribe_mailbox_seq();

        session
            .services
            .agent_control
            .register_session_root(session.conversation_id, &turn.session_source);
        let current_agent_path = turn
            .session_source
            .get_agent_path()
            .unwrap_or_else(AgentPath::root);
        let descendant_prefix = format!("{current_agent_path}/");
        let has_descendant_agents = session
            .services
            .agent_control
            .list_agents(&turn.session_source, None)
            .await
            .map_err(collab_spawn_error)?
            .into_iter()
            .any(|agent| agent.agent_name.starts_with(&descendant_prefix));
        if !has_descendant_agents {
            return Ok(WaitAgentResult::no_agents());
        }

        session
            .send_event(
                &turn,
                CollabWaitingBeginEvent {
                    sender_thread_id: session.conversation_id,
                    receiver_thread_ids: Vec::new(),
                    receiver_agents: Vec::new(),
                    call_id: call_id.clone(),
                }
                .into(),
            )
            .await;

        let timed_out = if session.has_pending_mailbox_items().await {
            false
        } else {
            let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
            !wait_for_mailbox_change(&mut mailbox_seq_rx, deadline).await
        };
        let result = WaitAgentResult::from_timed_out(timed_out);

        session
            .send_event(
                &turn,
                CollabWaitingEndEvent {
                    sender_thread_id: session.conversation_id,
                    call_id,
                    agent_statuses: Vec::new(),
                    statuses: HashMap::new(),
                }
                .into(),
            )
            .await;

        Ok(result)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaitArgs {
    timeout_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct WaitAgentResult {
    pub(crate) message: String,
    pub(crate) timed_out: bool,
}

impl WaitAgentResult {
    fn no_agents() -> Self {
        Self {
            message: "No agents available yet.".to_string(),
            timed_out: false,
        }
    }

    fn from_timed_out(timed_out: bool) -> Self {
        let message = if timed_out {
            "Wait timed out."
        } else {
            "Wait completed."
        };
        Self {
            message: message.to_string(),
            timed_out,
        }
    }
}

impl ToolOutput for WaitAgentResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "wait_agent")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, /*success*/ None, "wait_agent")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "wait_agent")
    }
}

async fn wait_for_mailbox_change(
    mailbox_seq_rx: &mut tokio::sync::watch::Receiver<u64>,
    deadline: Instant,
) -> bool {
    match timeout_at(deadline, mailbox_seq_rx.changed()).await {
        Ok(Ok(())) => true,
        Ok(Err(_)) | Err(_) => false,
    }
}
