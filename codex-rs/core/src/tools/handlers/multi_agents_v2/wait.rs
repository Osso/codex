use super::*;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::handlers::multi_agents_spec::WaitAgentTimeoutOptions;
use crate::tools::handlers::multi_agents_spec::create_wait_agent_tool_v2;
use crate::turn_timing::now_unix_timestamp_ms;
use codex_protocol::ThreadId;
use codex_tools::ToolSpec;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;
use tokio::time::timeout_at;

#[derive(Default)]
pub(crate) struct Handler {
    options: WaitAgentTimeoutOptions,
}

impl Handler {
    pub(crate) fn new(options: WaitAgentTimeoutOptions) -> Self {
        Self { options }
    }
}

impl ToolHandler for Handler {
    type Output = WaitAgentResult;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("wait_agent")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_wait_agent_tool_v2(self.options))
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
        let min_timeout_ms = turn
            .config
            .multi_agent_v2
            .min_wait_timeout_ms
            .clamp(1, MAX_WAIT_TIMEOUT_MS);
        let timeout_ms = validated_timeout_ms(args.timeout_ms, min_timeout_ms)?;

        let mut mailbox_seq_rx = session.subscribe_mailbox_seq();

        session
            .services
            .agent_control
            .register_session_root(session.conversation_id, &turn.session_source);
        let descendant_prefix = descendant_prefix(&turn);
        let descendant_statuses =
            list_descendant_agent_statuses(&session, &turn, &descendant_prefix).await?;
        if descendant_statuses.is_empty() {
            return Ok(WaitAgentResult::no_agents());
        }
        send_waiting_begin(&session, &turn, call_id.clone(), &descendant_statuses).await;

        let timed_out = if session.has_pending_mailbox_items().await {
            false
        } else {
            let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
            !wait_for_mailbox_change(&mut mailbox_seq_rx, deadline).await
        };
        let result = WaitAgentResult::from_timed_out(timed_out);
        let statuses = list_descendant_agent_statuses(&session, &turn, &descendant_prefix).await?;
        send_waiting_end(&session, &turn, call_id, statuses).await;

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

fn validated_timeout_ms(
    timeout_ms: Option<i64>,
    min_timeout_ms: i64,
) -> Result<i64, FunctionCallError> {
    match timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS) {
        ms if ms <= 0 => Err(FunctionCallError::RespondToModel(
            "timeout_ms must be greater than zero".to_owned(),
        )),
        ms => Ok(ms.clamp(min_timeout_ms, MAX_WAIT_TIMEOUT_MS)),
    }
}

fn descendant_prefix(turn: &TurnContext) -> String {
    let current_agent_path = turn
        .session_source
        .get_agent_path()
        .unwrap_or_else(AgentPath::root);
    format!("{current_agent_path}/")
}

async fn list_descendant_agent_statuses(
    session: &Session,
    turn: &TurnContext,
    descendant_prefix: &str,
) -> Result<HashMap<ThreadId, AgentStatus>, FunctionCallError> {
    let agents = session
        .services
        .agent_control
        .list_agents(&turn.session_source, None)
        .await
        .map_err(collab_spawn_error)?;
    let mut statuses = HashMap::new();
    for agent in agents {
        if !agent.agent_name.starts_with(descendant_prefix) {
            continue;
        }
        let thread_id = session
            .services
            .agent_control
            .resolve_agent_reference(
                session.conversation_id,
                &turn.session_source,
                &agent.agent_name,
            )
            .await
            .map_err(collab_spawn_error)?;
        statuses.insert(thread_id, agent.agent_status);
    }
    Ok(statuses)
}

async fn send_waiting_begin(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    descendant_statuses: &HashMap<ThreadId, AgentStatus>,
) {
    let mut receiver_thread_ids = descendant_statuses.keys().copied().collect::<Vec<_>>();
    receiver_thread_ids.sort_by_key(ToString::to_string);
    session
        .send_event(
            turn,
            CollabWaitingBeginEvent {
                started_at_ms: now_unix_timestamp_ms(),
                sender_thread_id: session.conversation_id,
                receiver_thread_ids,
                receiver_agents: Vec::new(),
                call_id,
            }
            .into(),
        )
        .await;
}

async fn send_waiting_end(
    session: &Session,
    turn: &TurnContext,
    call_id: String,
    statuses: HashMap<ThreadId, AgentStatus>,
) {
    let agent_statuses = build_wait_agent_statuses(&statuses, &[]);
    session
        .send_event(
            turn,
            CollabWaitingEndEvent {
                sender_thread_id: session.conversation_id,
                call_id,
                completed_at_ms: now_unix_timestamp_ms(),
                agent_statuses,
                statuses,
            }
            .into(),
        )
        .await;
}
