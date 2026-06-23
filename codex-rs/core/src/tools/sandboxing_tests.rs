use super::*;
use crate::sandboxing::SandboxPermissions;
use crate::tools::hook_names::HookToolName;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::NetworkAccess;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn bash_permission_request_payload_omits_missing_description() {
    assert_eq!(
        PermissionRequestPayload::bash("echo hi".to_string(), /*description*/ None),
        PermissionRequestPayload {
            tool_name: HookToolName::bash(),
            tool_input: json!({ "command": "echo hi" }),
        }
    );
}

#[test]
fn bash_permission_request_payload_includes_description_when_present() {
    assert_eq!(
        PermissionRequestPayload::bash(
            "echo hi".to_string(),
            Some("network-access example.com".to_string()),
        ),
        PermissionRequestPayload {
            tool_name: HookToolName::bash(),
            tool_input: json!({
                "command": "echo hi",
                "description": "network-access example.com",
            }),
        }
    );
}

#[test]
fn external_sandbox_skips_exec_approval_on_request() {
    let sandbox_policy = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Restricted,
    };
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::from(&sandbox_policy),
        ),
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
            pre_approved: false,
        }
    );
}

#[test]
fn restricted_sandbox_requires_exec_approval_on_request() {
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    assert_eq!(
        default_exec_approval_requirement(
            AskForApproval::OnRequest,
            &FileSystemSandboxPolicy::from(&sandbox_policy)
        ),
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn visible_approval_modes_follow_sandbox_matrix() {
    let visible_modes = [
        (
            "read-only",
            FileSystemSandboxPolicy::from(&SandboxPolicy::new_read_only_policy()),
        ),
        (
            "workspace-write",
            FileSystemSandboxPolicy::from(&SandboxPolicy::new_workspace_write_policy()),
        ),
        (
            "full-access",
            FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess),
        ),
    ];

    for (approval_label, approval_policy) in [
        ("ask-me", AskForApproval::OnRequest),
        ("llm-approved", AskForApproval::OnRequest),
    ] {
        for (sandbox_label, sandbox_policy) in &visible_modes {
            assert_eq!(
                default_exec_approval_requirement(approval_policy, sandbox_policy),
                ExecApprovalRequirement::NeedsApproval {
                    reason: None,
                    proposed_execpolicy_amendment: None,
                },
                "{approval_label} should ask in {sandbox_label}"
            );
        }
    }

    for (sandbox_label, sandbox_policy) in &visible_modes {
        assert_eq!(
            default_exec_approval_requirement(AskForApproval::Never, sandbox_policy),
            ExecApprovalRequirement::Forbidden {
                reason: "approval policy is Never".to_string(),
            },
            "never should reject in {sandbox_label}"
        );
        assert_eq!(
            default_exec_approval_requirement(AskForApproval::AutoApprove, sandbox_policy),
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
                pre_approved: true,
            },
            "auto-approve should pre-approve in {sandbox_label}"
        );
    }
}

#[test]
fn default_exec_approval_requirement_rejects_sandbox_prompt_when_granular_disables_it() {
    let policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: false,
        rules: true,
        skill_approval: true,
        request_permissions: true,
        mcp_elicitations: true,
    });

    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::from(&sandbox_policy));

    assert_eq!(
        requirement,
        ExecApprovalRequirement::Forbidden {
            reason: "approval policy disallowed sandbox approval prompt".to_string(),
        }
    );
}

#[test]
fn default_exec_approval_requirement_keeps_prompt_when_granular_allows_sandbox_approval() {
    let policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: true,
        rules: false,
        skill_approval: true,
        request_permissions: true,
        mcp_elicitations: false,
    });

    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let requirement =
        default_exec_approval_requirement(policy, &FileSystemSandboxPolicy::from(&sandbox_policy));

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn pre_tool_use_approval_skips_exec_approval_prompt() {
    let requirement = ExecApprovalRequirement::NeedsApproval {
        reason: Some("requires approval".to_string()),
        proposed_execpolicy_amendment: None,
    };

    assert_eq!(
        apply_pre_tool_use_approval(
            requirement,
            /*approved*/ true,
            /*approval_required*/ false
        ),
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
            pre_approved: true,
        }
    );
}

#[test]
fn missing_pre_tool_use_approval_preserves_exec_approval_prompt() {
    let requirement = ExecApprovalRequirement::NeedsApproval {
        reason: Some("requires approval".to_string()),
        proposed_execpolicy_amendment: None,
    };

    assert_eq!(
        apply_pre_tool_use_approval(
            requirement.clone(),
            /*approved*/ false,
            /*approval_required*/ false
        ),
        requirement
    );
}

#[test]
fn pre_tool_use_ask_forces_exec_approval_prompt() {
    let requirement = ExecApprovalRequirement::Skip {
        bypass_sandbox: false,
        proposed_execpolicy_amendment: None,
        pre_approved: false,
    };

    assert_eq!(
        apply_pre_tool_use_approval(
            requirement,
            /*approved*/ false,
            /*approval_required*/ true
        ),
        ExecApprovalRequirement::NeedsApproval {
            reason: Some("PreToolUse hook requested approval".to_string()),
            proposed_execpolicy_amendment: None,
        }
    );
}

#[test]
fn additional_permissions_allow_bypass_sandbox_first_attempt_when_execpolicy_skips() {
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::WithAdditionalPermissions,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
                pre_approved: false,
            },
            &FileSystemSandboxPolicy::default(),
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn guardian_bypasses_sandbox_for_explicit_escalation_on_first_attempt() {
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::RequireEscalated,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
                pre_approved: false,
            },
            &FileSystemSandboxPolicy::default(),
        ),
        SandboxOverride::BypassSandboxFirstAttempt
    );
}

#[test]
fn deny_read_blocks_explicit_escalation_but_preserves_policy_bypass() {
    let file_system_policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::GlobPattern {
            pattern: "**/*.env".to_string(),
        },
        access: FileSystemAccessMode::None,
    }]);

    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::RequireEscalated,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
                pre_approved: false,
            },
            &file_system_policy,
        ),
        SandboxOverride::NoOverride,
        "explicit escalation would drop deny-read filesystem policy, so keep the first attempt sandboxed",
    );
    assert_eq!(
        sandbox_override_for_first_attempt(
            SandboxPermissions::WithAdditionalPermissions,
            &ExecApprovalRequirement::Skip {
                bypass_sandbox: true,
                proposed_execpolicy_amendment: None,
                pre_approved: false,
            },
            &file_system_policy,
        ),
        SandboxOverride::BypassSandboxFirstAttempt,
        "exec-policy allow rules intentionally bypass sandbox even when deny-read entries exist",
    );
}
