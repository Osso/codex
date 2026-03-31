use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SessionEndHookEnvelope {
    #[serde(default)]
    transcript_path: Option<String>,
    #[serde(default)]
    hook_event: Option<SessionEndHookEvent>,
}

#[derive(Debug, Deserialize)]
struct SessionEndHookEvent {
    #[serde(default)]
    transcript_path: Option<String>,
}

pub fn session_end_transcript_path_from_json(input: &str) -> Option<String> {
    let parsed: SessionEndHookEnvelope = serde_json::from_str(input).ok()?;
    parsed
        .transcript_path
        .or_else(|| parsed.hook_event.and_then(|event| event.transcript_path))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::session_end_transcript_path_from_json;

    #[test]
    fn reads_legacy_top_level_transcript_path() {
        let input = r#"{
            "session_id": "legacy",
            "transcript_path": "/tmp/legacy.jsonl",
            "hook_event_name": "SessionEnd"
        }"#;

        assert_eq!(
            session_end_transcript_path_from_json(input),
            Some("/tmp/legacy.jsonl".to_string())
        );
    }

    #[test]
    fn reads_codex_nested_transcript_path() {
        let input = r#"{
            "session_id": "codex",
            "hook_event_name": "SessionEnd",
            "hook_event": {
                "event_type": "session_end",
                "thread_id": "thread-1",
                "transcript_path": "/tmp/codex.jsonl"
            }
        }"#;

        assert_eq!(
            session_end_transcript_path_from_json(input),
            Some("/tmp/codex.jsonl".to_string())
        );
    }

    #[test]
    fn prefers_top_level_transcript_path_when_both_exist() {
        let input = r#"{
            "transcript_path": "/tmp/top-level.jsonl",
            "hook_event": {
                "transcript_path": "/tmp/nested.jsonl"
            }
        }"#;

        assert_eq!(
            session_end_transcript_path_from_json(input),
            Some("/tmp/top-level.jsonl".to_string())
        );
    }

    #[test]
    fn returns_none_for_missing_transcript_path() {
        let input = r#"{
            "session_id": "empty",
            "hook_event_name": "SessionEnd"
        }"#;

        assert_eq!(session_end_transcript_path_from_json(input), None);
    }

    #[test]
    fn returns_none_for_invalid_json() {
        assert_eq!(session_end_transcript_path_from_json("{"), None);
    }
}
