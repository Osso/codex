//! Detect and pretty-print inter-agent communication payloads that get
//! serialized as assistant-role messages.
//!
//! When a subagent finishes, the core enqueues an `InterAgentCommunication`
//! onto the parent mailbox. During turn drain the whole struct is serialized
//! to JSON and injected as a `ResponseInputItem::Message { role: "assistant" }`
//! so the model sees author/recipient/trigger_turn context. Without this
//! helper that raw JSON bubbles up to the TUI as an agent message cell.

use codex_protocol::protocol::InterAgentCommunication;

const SUBAGENT_NOTIFICATION_OPEN: &str = "<subagent_notification>";
const SUBAGENT_NOTIFICATION_CLOSE: &str = "</subagent_notification>";

/// If `raw` is a serialized `InterAgentCommunication`, return a compact
/// human-readable rendering. Returns `None` for normal agent text.
pub(crate) fn pretty_inter_agent_message(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let communication: InterAgentCommunication = serde_json::from_str(trimmed).ok()?;
    Some(format_communication(&communication))
}

fn format_communication(comm: &InterAgentCommunication) -> String {
    let body = render_body(comm.content.trim());
    let arrow = if comm.trigger_turn { "→" } else { "↷" };
    let mut out = format!(
        "✉ {author} {arrow} {recipient}",
        author = comm.author.as_str(),
        arrow = arrow,
        recipient = comm.recipient.as_str(),
    );
    if !comm.other_recipients.is_empty() {
        let others: Vec<&str> = comm
            .other_recipients
            .iter()
            .map(codex_protocol::AgentPath::as_str)
            .collect();
        out.push_str(&format!(" (cc: {})", others.join(", ")));
    }
    if !body.is_empty() {
        out.push('\n');
        out.push_str(&body);
    }
    out
}

fn render_body(content: &str) -> String {
    if let Some(inner) = strip_tag(
        content,
        SUBAGENT_NOTIFICATION_OPEN,
        SUBAGENT_NOTIFICATION_CLOSE,
    ) && let Some(pretty) = format_subagent_notification(inner)
    {
        return pretty;
    }
    content.to_string()
}

fn strip_tag<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let rest = text.trim().strip_prefix(open)?;
    let inner = rest.strip_suffix(close)?;
    Some(inner.trim())
}

fn format_subagent_notification(inner: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(inner).ok()?;
    let agent_path = value.get("agent_path")?.as_str()?;
    let status = value.get("status")?;
    let (label, detail) = describe_status(status);
    match detail {
        Some(text) if !text.is_empty() => Some(format!("subagent {agent_path} {label}: {text}")),
        _ => Some(format!("subagent {agent_path} {label}")),
    }
}

fn describe_status(status: &serde_json::Value) -> (String, Option<String>) {
    match status {
        serde_json::Value::String(label) => (label.clone(), None),
        serde_json::Value::Object(map) => map
            .iter()
            .next()
            .map(|(key, value)| (key.clone(), value.as_str().map(str::to_string)))
            .unwrap_or_else(|| ("unknown".to_string(), None)),
        _ => ("unknown".to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::AgentPath;
    use codex_protocol::protocol::AgentStatus;
    use pretty_assertions::assert_eq;

    fn serialize(comm: &InterAgentCommunication) -> String {
        serde_json::to_string(comm).expect("serialize communication")
    }

    #[test]
    fn pretty_prints_subagent_notification() {
        let author = AgentPath::try_from("/root/worker").expect("author path");
        let recipient = AgentPath::root();
        let content = SUBAGENT_NOTIFICATION_OPEN.to_string()
            + &serde_json::json!({
                "agent_path": author.as_str(),
                "status": AgentStatus::Completed(Some("No edits applied.".to_string())),
            })
            .to_string()
            + SUBAGENT_NOTIFICATION_CLOSE;
        let comm = InterAgentCommunication::new(author, recipient, Vec::new(), content, true);

        let rendered = pretty_inter_agent_message(&serialize(&comm)).expect("pretty");
        assert_eq!(
            rendered,
            "✉ /root/worker → /root\nsubagent /root/worker completed: No edits applied."
        );
    }

    #[test]
    fn pretty_prints_plain_inter_agent_message() {
        let author = AgentPath::try_from("/root/worker").expect("author path");
        let recipient = AgentPath::root();
        let comm = InterAgentCommunication::new(
            author,
            recipient,
            Vec::new(),
            "queued child update".to_string(),
            false,
        );

        let rendered = pretty_inter_agent_message(&serialize(&comm)).expect("pretty");
        assert_eq!(rendered, "✉ /root/worker ↷ /root\nqueued child update");
    }

    #[test]
    fn returns_none_for_non_communication_json() {
        assert!(pretty_inter_agent_message("hello world").is_none());
        assert!(pretty_inter_agent_message("{\"unrelated\":true}").is_none());
    }
}
