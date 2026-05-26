//! Hostrun will provide a stateful, approval-readable host execution runtime.

mod tool_bundle;
mod tool_contributor;

use std::collections::BTreeMap;

use serde::Deserialize;
use serde::Serialize;

pub use tool_bundle::HostrunToolConfig;
pub use tool_bundle::hostrun_tool_bundle;
pub use tool_contributor::HOSTRUN_RUNNER_ENV;
pub use tool_contributor::HostrunToolContributor;
pub use tool_contributor::install;
pub use tool_contributor::install_from_env;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct HostrunSessionId(String);

impl HostrunSessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ApprovalRequestId(String);

impl ApprovalRequestId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct CreateSessionRequest {
    pub session_id: HostrunSessionId,
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
}

impl CreateSessionRequest {
    pub fn new(session_id: HostrunSessionId) -> Self {
        Self {
            session_id,
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ResetSessionRequest {
    pub session_id: HostrunSessionId,
}

impl ResetSessionRequest {
    pub fn new(session_id: HostrunSessionId) -> Self {
        Self { session_id }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct EvalRequest {
    pub session_id: HostrunSessionId,
    pub code: String,
}

impl EvalRequest {
    pub fn new(session_id: HostrunSessionId, code: impl Into<String>) -> Self {
        Self {
            session_id,
            code: code.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ApprovalRequest {
    pub id: ApprovalRequestId,
    pub session_id: HostrunSessionId,
    pub summary: String,
    pub operations: Vec<HostOperation>,
}

impl ApprovalRequest {
    pub fn new(
        id: ApprovalRequestId,
        session_id: HostrunSessionId,
        summary: impl Into<String>,
        operations: Vec<HostOperation>,
    ) -> Self {
        Self {
            id,
            session_id,
            summary: summary.into(),
            operations,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostOperation {
    ReadFile { path: String },
    WriteFile { path: String },
    RunCommand { program: String, args: Vec<String> },
    DeleteRemote { provider: String, target: String },
}

impl HostOperation {
    pub fn read_file(path: impl Into<String>) -> Self {
        Self::ReadFile { path: path.into() }
    }

    pub fn write_file(path: impl Into<String>) -> Self {
        Self::WriteFile { path: path.into() }
    }

    pub fn run_command(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::RunCommand {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    pub fn delete_remote(provider: impl Into<String>, target: impl Into<String>) -> Self {
        Self::DeleteRemote {
            provider: provider.into(),
            target: target.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ApprovalDecision {
    pub approved: bool,
    pub reason: Option<String>,
}

impl ApprovalDecision {
    pub fn approve() -> Self {
        Self {
            approved: true,
            reason: None,
        }
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalResult {
    Completed {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    NeedsApproval {
        approval: ApprovalRequest,
    },
}

impl EvalResult {
    pub fn completed(stdout: impl Into<String>, stderr: impl Into<String>, exit_code: i32) -> Self {
        Self::Completed {
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
        }
    }

    pub fn needs_approval(approval: ApprovalRequest) -> Self {
        Self::NeedsApproval { approval }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn eval_request_serializes_stable_session_and_code_shape() {
        let request = EvalRequest::new(HostrunSessionId::new("session-1"), "ctx.count = 1;");

        let value = serde_json::to_value(request).expect("eval request serializes");

        assert_eq!(
            value,
            json!({
                "session_id": "session-1",
                "code": "ctx.count = 1;"
            })
        );
    }

    #[test]
    fn create_session_request_serializes_lifecycle_configuration() {
        let request = CreateSessionRequest::new(HostrunSessionId::new("session-1"))
            .with_cwd("/home/osso/Repos/codex")
            .with_env("HOSTRUN_MODE", "test");

        let value = serde_json::to_value(request).expect("create session request serializes");

        assert_eq!(
            value,
            json!({
                "session_id": "session-1",
                "cwd": "/home/osso/Repos/codex",
                "env": {
                    "HOSTRUN_MODE": "test"
                }
            })
        );
    }

    #[test]
    fn reset_session_request_serializes_lifecycle_target() {
        let request = ResetSessionRequest::new(HostrunSessionId::new("session-1"));

        let value = serde_json::to_value(request).expect("reset session request serializes");

        assert_eq!(
            value,
            json!({
                "session_id": "session-1"
            })
        );
    }

    #[test]
    fn approval_request_serializes_file_command_and_remote_operations() {
        let approval = ApprovalRequest::new(
            ApprovalRequestId::new("approval-1"),
            HostrunSessionId::new("session-1"),
            "Delete leftover probe objects",
            vec![
                HostOperation::read_file("secrets/ipg-ingester-credentials.md"),
                HostOperation::write_file("/tmp/files.txt"),
                HostOperation::run_command("rclone", ["lsf", "spaces:bucket"]),
                HostOperation::delete_remote(
                    "rclone",
                    "spaces:globalcomix-publisher-uploads/probe.txt",
                ),
            ],
        );

        let value = serde_json::to_value(approval).expect("approval serializes");

        assert_eq!(
            value,
            json!({
                "id": "approval-1",
                "session_id": "session-1",
                "summary": "Delete leftover probe objects",
                "operations": [
                    { "type": "read_file", "path": "secrets/ipg-ingester-credentials.md" },
                    { "type": "write_file", "path": "/tmp/files.txt" },
                    { "type": "run_command", "program": "rclone", "args": ["lsf", "spaces:bucket"] },
                    { "type": "delete_remote", "provider": "rclone", "target": "spaces:globalcomix-publisher-uploads/probe.txt" }
                ]
            })
        );
    }

    #[test]
    fn denied_approval_decision_carries_reason() {
        let decision = ApprovalDecision::deny("remote delete is too broad");

        let value = serde_json::to_value(decision).expect("approval decision serializes");

        assert_eq!(
            value,
            json!({
                "approved": false,
                "reason": "remote delete is too broad"
            })
        );
    }

    #[test]
    fn eval_result_can_pause_for_approval() {
        let result = EvalResult::needs_approval(ApprovalRequest::new(
            ApprovalRequestId::new("approval-1"),
            HostrunSessionId::new("session-1"),
            "Write /tmp/files.txt",
            vec![HostOperation::write_file("/tmp/files.txt")],
        ));

        let value = serde_json::to_value(result).expect("eval result serializes");

        assert_eq!(
            value,
            json!({
                "type": "needs_approval",
                "approval": {
                    "id": "approval-1",
                    "session_id": "session-1",
                    "summary": "Write /tmp/files.txt",
                    "operations": [
                        { "type": "write_file", "path": "/tmp/files.txt" }
                    ]
                }
            })
        );
    }
}
