/// Minimal no-op stub that replaced the `codex-feedback` ring-buffer crate.
///
/// The feedback-upload backend is not shipped in this fork.  All call sites that
/// previously constructed a `CodexFeedback` or called `logger_layer` /
/// `metadata_layer` / `snapshot` continue to compile against this stub without
/// changing their signatures.
#[derive(Clone, Default)]
pub struct CodexFeedback;

/// Stub snapshot returned by [`CodexFeedback::snapshot`].
pub struct FeedbackSnapshot {
    diagnostics: FeedbackDiagnostics,
}

impl FeedbackSnapshot {
    /// Returns an empty diagnostics object.
    pub fn feedback_diagnostics(&self) -> &FeedbackDiagnostics {
        &self.diagnostics
    }
}

impl Default for FeedbackSnapshot {
    fn default() -> Self {
        Self {
            diagnostics: FeedbackDiagnostics::default(),
        }
    }
}

/// Stub diagnostics — wraps a vec but the upload backend is removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackDiagnostics {
    diagnostics: Vec<FeedbackDiagnostic>,
}

impl FeedbackDiagnostics {
    pub fn new(diagnostics: Vec<FeedbackDiagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[FeedbackDiagnostic] {
        &self.diagnostics
    }
}

/// Stub diagnostic entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackDiagnostic {
    pub headline: String,
    pub details: Vec<String>,
}

pub const FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME: &str =
    "codex-connectivity-diagnostics.txt";

impl CodexFeedback {
    pub fn new() -> Self {
        Self
    }

    pub fn snapshot(&self, _session_id: impl std::fmt::Debug) -> FeedbackSnapshot {
        FeedbackSnapshot::default()
    }
}
