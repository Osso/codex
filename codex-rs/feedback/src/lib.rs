//! No-op compatibility crate for removed feedback upload integration.

use std::collections::BTreeMap;
use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;

#[derive(Clone, Default)]
pub struct CodexFeedback;

impl CodexFeedback {
    pub fn new() -> Self {
        Self
    }

    pub fn snapshot(&self, conversation_id: Option<ThreadId>) -> FeedbackSnapshot {
        FeedbackSnapshot {
            thread_id: conversation_id
                .map(|thread_id| thread_id.to_string())
                .unwrap_or_default(),
        }
    }
}

pub struct FeedbackSnapshot {
    pub thread_id: String,
}

impl FeedbackSnapshot {
    pub fn upload_feedback(self, _options: FeedbackUploadOptions<'_>) -> Result<(), String> {
        Ok(())
    }
}

pub struct FeedbackAttachmentPath {
    pub path: PathBuf,
    pub attachment_filename_override: Option<String>,
}

pub struct FeedbackUploadOptions<'a> {
    pub classification: &'a str,
    pub reason: Option<&'a str>,
    pub tags: Option<&'a BTreeMap<String, String>>,
    pub include_logs: bool,
    pub extra_attachment_paths: &'a [FeedbackAttachmentPath],
    pub session_source: Option<SessionSource>,
    pub logs_override: Option<Vec<u8>>,
}
