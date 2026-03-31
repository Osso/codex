use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use codex_apply_patch::ApplyPatchFileChange;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerStack;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
#[cfg(test)]
use codex_protocol::protocol::HookEventName;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;

use crate::engine::ClaudeHooksEngine;
use crate::engine::CommandShell;
use crate::events::session_start::SessionStartOutcome;
use crate::events::session_start::SessionStartRequest;
use crate::events::stop::StopOutcome;
use crate::events::stop::StopRequest;
use crate::types::CommandHookConfig;
use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookExecutionOutcome;
use crate::types::HookPayload;
use crate::types::HookPermissionDecision;
use crate::types::HookResponse;
use crate::types::HookResult;
use crate::types::HookRuleConfig;
use crate::types::HookToolInput;
use crate::types::HooksToml;

#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;

#[cfg(test)]
use chrono::TimeZone;
#[cfg(test)]
use chrono::Utc;
#[cfg(test)]
use codex_protocol::ThreadId;

#[cfg(test)]
use crate::types::HookEventAfterAgent;
#[cfg(test)]
use crate::types::HookEventAfterToolUse;
#[cfg(test)]
use crate::types::HookEventUserPromptSubmit;
#[cfg(test)]
use crate::types::HookToolKind;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
    pub feature_enabled: bool,
    pub config_layer_stack: Option<ConfigLayerStack>,
    pub shell_program: Option<String>,
    pub shell_args: Vec<String>,
    pub hooks: HooksToml,
}

#[derive(Clone)]
pub struct Hooks {
    pre_tool_use: Vec<Hook>,
    after_agent: Vec<Hook>,
    after_tool_use: Vec<Hook>,
    user_prompt_submit: Vec<Hook>,
    session_end: Vec<Hook>,
    subagent_start: Vec<Hook>,
    subagent_stop: Vec<Hook>,
    engine: ClaudeHooksEngine,
}

impl Default for Hooks {
    fn default() -> Self {
        Self::new(HooksConfig::default())
    }
}

impl Hooks {
    pub fn new(config: HooksConfig) -> Self {
        let toml_session_start = crate::engine::discovery::discover_toml_session_start_handlers(
            Path::new(CONFIG_TOML_FILE),
            &config.hooks.session_start,
        );
        let after_agent = config
            .legacy_notify_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(crate::notify_hook)
            .into_iter()
            .collect();
        let engine = ClaudeHooksEngine::new(
            config.feature_enabled,
            config.config_layer_stack.as_ref(),
            CommandShell {
                program: config.shell_program.unwrap_or_default(),
                args: config.shell_args,
            },
            toml_session_start.handlers,
            toml_session_start.warnings,
        );
        Self {
            pre_tool_use: build_command_hooks("pre_tool_use", &config.hooks.pre_tool_use),
            after_agent,
            after_tool_use: build_command_hooks("post_tool_use", &config.hooks.post_tool_use),
            user_prompt_submit: build_command_hooks(
                "user_prompt_submit",
                &config.hooks.user_prompt_submit,
            ),
            session_end: build_command_hooks("session_end", &config.hooks.session_end),
            subagent_start: build_command_hooks("subagent_start", &config.hooks.subagent_start),
            subagent_stop: build_command_hooks("subagent_stop", &config.hooks.subagent_stop),
            engine,
        }
    }

    pub fn startup_warnings(&self) -> &[String] {
        self.engine.warnings()
    }

    fn hooks_for_event(&self, hook_event: &HookEvent) -> &[Hook] {
        match hook_event {
            HookEvent::PreToolUse { .. } => &self.pre_tool_use,
            HookEvent::AfterAgent { .. } => &self.after_agent,
            HookEvent::AfterToolUse { .. } => &self.after_tool_use,
            HookEvent::UserPromptSubmit { .. } => &self.user_prompt_submit,
            HookEvent::SessionEnd { .. } => &self.session_end,
            HookEvent::SubagentStart { .. } => &self.subagent_start,
            HookEvent::SubagentStop { .. } => &self.subagent_stop,
        }
    }

    pub async fn dispatch(&self, hook_payload: HookPayload) -> Vec<HookResponse> {
        let hooks = self.hooks_for_event(&hook_payload.hook_event);
        let mut outcomes = Vec::with_capacity(hooks.len());
        for hook in hooks {
            let outcome = hook.execute(&hook_payload).await;
            let should_abort_operation = outcome.result.should_abort_operation();
            outcomes.push(outcome);
            if should_abort_operation {
                break;
            }
        }

        outcomes
    }

    pub fn preview_session_start(
        &self,
        request: &SessionStartRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_session_start(request)
    }

    pub async fn run_session_start(
        &self,
        request: SessionStartRequest,
        turn_id: Option<String>,
    ) -> SessionStartOutcome {
        self.engine.run_session_start(request, turn_id).await
    }

    pub fn preview_stop(
        &self,
        request: &StopRequest,
    ) -> Vec<codex_protocol::protocol::HookRunSummary> {
        self.engine.preview_stop(request)
    }

    pub async fn run_stop(&self, request: StopRequest) -> StopOutcome {
        self.engine.run_stop(request).await
    }
}

pub fn command_from_argv(argv: &[String]) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    if program.is_empty() {
        return None;
    }
    let mut command = Command::new(program);
    command.args(args);
    Some(command)
}

fn build_command_hooks(prefix: &str, rules: &[HookRuleConfig]) -> Vec<Hook> {
    rules
        .iter()
        .enumerate()
        .flat_map(|(rule_index, rule)| {
            rule.commands
                .iter()
                .enumerate()
                .map(move |(command_index, command)| {
                    command_hook(
                        prefix,
                        rule_index,
                        command_index,
                        rule.matcher.clone(),
                        command.clone(),
                    )
                })
        })
        .collect()
}

fn command_hook(
    prefix: &str,
    rule_index: usize,
    command_index: usize,
    matcher: Option<String>,
    command: CommandHookConfig,
) -> Hook {
    Hook {
        name: format!("{prefix}:{rule_index}:{command_index}"),
        func: Arc::new(move |payload| {
            let matcher = matcher.clone();
            let command = command.clone();
            Box::pin(async move {
                if !matches_hook(matcher.as_deref(), &payload.hook_event) {
                    return HookExecutionOutcome::success();
                }
                run_command_hook(&command.command, command.timeout_sec, payload).await
            })
        }),
    }
}

fn matches_hook(matcher: Option<&str>, event: &HookEvent) -> bool {
    let Some(matcher) = matcher.map(str::trim) else {
        return true;
    };
    if matcher.is_empty() {
        return true;
    }
    let Some(subject) = event.matcher_subject() else {
        return true;
    };
    matcher
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| matcher_part_matches_subject(part, subject))
}

fn matcher_part_matches_subject(matcher: &str, subject: &str) -> bool {
    if matcher == subject {
        return true;
    }

    match matcher {
        "Bash" => matches!(
            subject,
            "shell" | "local_shell" | "shell_command" | "exec_command"
        ),
        "Write" | "Edit" => matches!(subject, "apply_patch"),
        _ => false,
    }
}

async fn run_command_hook(
    command_text: &str,
    timeout_sec: Option<u64>,
    payload: &HookPayload,
) -> HookExecutionOutcome {
    let mut command = shell_command(command_text);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CODEX_HOOK_EVENT", payload.hook_event.name());
    if let Some(tool_name) = payload.hook_event.tool_name() {
        command.env("CLAUDE_TOOL_NAME", tool_name);
    }

    let payload_json = match serde_json::to_vec(&command_hook_input(payload)) {
        Ok(payload_json) => payload_json,
        Err(err) => {
            return HookExecutionOutcome {
                result: HookResult::FailedContinue(err.into()),
                permission_decision: None,
                updated_input: None,
            };
        }
    };

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            return HookExecutionOutcome {
                result: HookResult::FailedContinue(err.into()),
                permission_decision: None,
                updated_input: None,
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let input = payload_json.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(&input).await;
        });
    }

    let output = match timeout_sec {
        Some(seconds) => {
            match timeout(Duration::from_secs(seconds), child.wait_with_output()).await {
                Ok(Ok(output)) => output,
                Ok(Err(err)) => {
                    return HookExecutionOutcome {
                        result: HookResult::FailedContinue(err.into()),
                        permission_decision: None,
                        updated_input: None,
                    };
                }
                Err(_) => {
                    return HookExecutionOutcome {
                        result: HookResult::FailedContinue(
                            io::Error::new(
                                io::ErrorKind::TimedOut,
                                format!("hook timed out after {seconds}s"),
                            )
                            .into(),
                        ),
                        permission_decision: None,
                        updated_input: None,
                    };
                }
            }
        }
        None => match child.wait_with_output().await {
            Ok(output) => output,
            Err(err) => {
                return HookExecutionOutcome {
                    result: HookResult::FailedContinue(err.into()),
                    permission_decision: None,
                    updated_input: None,
                };
            }
        },
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let status = output.status;
        let message = if stderr.is_empty() {
            format!("hook command exited with {status}")
        } else {
            format!("hook command exited with {status}: {stderr}")
        };
        return HookExecutionOutcome {
            result: HookResult::FailedContinue(io::Error::other(message).into()),
            permission_decision: None,
            updated_input: None,
        };
    }

    parse_command_output(&output.stdout)
}

fn shell_command(command_text: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(command_text);
        command
    } else {
        let mut command = Command::new("sh");
        command.arg("-lc").arg(command_text);
        command
    }
}

fn parse_command_output(stdout: &[u8]) -> HookExecutionOutcome {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return HookExecutionOutcome::success();
    }
    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        return HookExecutionOutcome::success();
    };

    if json.get("decision").and_then(Value::as_str) == Some("block") {
        let reason = json
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("hook blocked operation");
        return HookExecutionOutcome {
            result: HookResult::FailedAbort(io::Error::other(reason.to_string()).into()),
            permission_decision: Some(HookPermissionDecision::Deny {
                reason: reason.to_string(),
            }),
            updated_input: None,
        };
    }

    let hook_specific = json.get("hookSpecificOutput").and_then(Value::as_object);
    let maybe_permission = hook_specific
        .and_then(parse_permission_decision)
        .or_else(|| json.as_object().and_then(parse_permission_decision));
    let mut updated_input = hook_specific
        .and_then(|obj| obj.get("updatedInput"))
        .cloned();
    if updated_input.is_none()
        && let Some(additional_context) = hook_specific
            .and_then(|obj| obj.get("additionalContext"))
            .and_then(Value::as_str)
    {
        updated_input = Some(serde_json::json!({
            "additional_context": additional_context
        }));
    }
    if let Some(permission_decision) = maybe_permission {
        let result = match &permission_decision {
            HookPermissionDecision::Deny { reason } => {
                HookResult::FailedAbort(io::Error::other(reason.clone()).into())
            }
            HookPermissionDecision::Allow { .. } | HookPermissionDecision::Ask { .. } => {
                HookResult::Success
            }
        };
        return HookExecutionOutcome {
            result,
            permission_decision: Some(permission_decision),
            updated_input,
        };
    }
    if updated_input.is_some() {
        return HookExecutionOutcome {
            result: HookResult::Success,
            permission_decision: None,
            updated_input,
        };
    }

    HookExecutionOutcome::success()
}

fn parse_permission_decision(
    obj: &serde_json::Map<String, Value>,
) -> Option<HookPermissionDecision> {
    let decision = obj.get("permissionDecision").and_then(Value::as_str)?;
    let reason = obj
        .get("permissionDecisionReason")
        .and_then(Value::as_str)
        .unwrap_or(match decision {
            "allow" => "hook allowed operation",
            "ask" => "hook requires approval",
            "deny" => "hook denied operation",
            _ => "hook returned a permission decision",
        })
        .to_string();

    match decision {
        "allow" => Some(HookPermissionDecision::Allow { reason }),
        "ask" => Some(HookPermissionDecision::Ask { reason }),
        "deny" => Some(HookPermissionDecision::Deny { reason }),
        _ => None,
    }
}

fn command_hook_input(payload: &HookPayload) -> Value {
    let mut value =
        serde_json::to_value(payload).unwrap_or_else(|_| Value::Object(Default::default()));
    let Value::Object(ref mut obj) = value else {
        return value;
    };

    obj.insert(
        "hook_event_name".to_string(),
        Value::String(claude_hook_event_name(&payload.hook_event).to_string()),
    );

    if let Some(tool_name) = claude_tool_name(&payload.hook_event) {
        obj.insert("tool_name".to_string(), Value::String(tool_name));
    }

    if let Some(tool_input) = claude_tool_input(payload) {
        obj.insert("tool_input".to_string(), tool_input);
    }

    if let HookEvent::UserPromptSubmit { event } = &payload.hook_event {
        obj.insert(
            "prompt".to_string(),
            Value::String(event.input_messages.join("\n\n")),
        );
    }

    value
}

fn claude_hook_event_name(event: &HookEvent) -> &'static str {
    match event {
        HookEvent::PreToolUse { .. } => "PreToolUse",
        HookEvent::AfterAgent { .. } => "Stop",
        HookEvent::AfterToolUse { .. } => "PostToolUse",
        HookEvent::UserPromptSubmit { .. } => "UserPromptSubmit",
        HookEvent::SessionEnd { .. } => "SessionEnd",
        HookEvent::SubagentStart { .. } => "SubagentStart",
        HookEvent::SubagentStop { .. } => "SubagentStop",
    }
}

fn claude_tool_name(event: &HookEvent) -> Option<String> {
    match event {
        HookEvent::PreToolUse { event } | HookEvent::AfterToolUse { event } => Some(
            match event.tool_name.as_str() {
                "shell" | "local_shell" | "shell_command" | "exec_command" => "Bash",
                "apply_patch" => "Write",
                other => other,
            }
            .to_string(),
        ),
        _ => None,
    }
}

fn claude_tool_input(payload: &HookPayload) -> Option<Value> {
    let after_tool_use = match &payload.hook_event {
        HookEvent::PreToolUse { event } | HookEvent::AfterToolUse { event } => event,
        _ => return None,
    };
    let tool_input = &after_tool_use.tool_input;

    match tool_input {
        HookToolInput::LocalShell { params } => Some(serde_json::json!({
            "command": shell_join(&params.command),
            "cwd": params.workdir,
        })),
        HookToolInput::Custom { input } => {
            if after_tool_use.tool_name == "apply_patch" {
                apply_patch_tool_input(input, &payload.cwd)
            } else {
                Some(serde_json::json!({
                    "file_path": input,
                }))
            }
        }
        HookToolInput::Mcp {
            server,
            tool,
            arguments,
        } => {
            if server == "regex-replace" && tool == "regex_replace" {
                let parsed = serde_json::from_str::<Value>(arguments).ok();
                Some(serde_json::json!({
                    "dry_run": parsed
                        .as_ref()
                        .and_then(|value| value.get("dry_run"))
                        .and_then(Value::as_bool),
                }))
            } else {
                None
            }
        }
        HookToolInput::Function { arguments } => {
            function_tool_input(&after_tool_use.tool_name, arguments)
        }
    }
}

fn apply_patch_tool_input(input: &str, cwd: &std::path::Path) -> Option<Value> {
    let argv = vec!["apply_patch".to_string(), input.to_string()];
    match codex_apply_patch::maybe_parse_apply_patch_verified(&argv, cwd) {
        MaybeApplyPatchVerified::Body(action) => {
            let mut changes = action.changes().iter();
            let (path, change) = changes.next()?;
            if changes.next().is_some() {
                return None;
            }

            match change {
                ApplyPatchFileChange::Add { content } => Some(serde_json::json!({
                    "file_path": path.display().to_string(),
                    "content": content,
                })),
                ApplyPatchFileChange::Delete { .. } => None,
                ApplyPatchFileChange::Update {
                    unified_diff: _,
                    move_path,
                    new_content,
                } => {
                    let file_path = move_path.as_deref().unwrap_or(path);
                    Some(serde_json::json!({
                        "file_path": file_path.display().to_string(),
                        "content": new_content,
                    }))
                }
            }
        }
        MaybeApplyPatchVerified::CorrectnessError(_)
        | MaybeApplyPatchVerified::ShellParseError(_)
        | MaybeApplyPatchVerified::NotApplyPatch => None,
    }
}

fn function_tool_input(tool_name: &str, arguments: &str) -> Option<Value> {
    match tool_name {
        "shell" => serde_json::from_str::<ShellToolCallParams>(arguments)
            .ok()
            .map(|params| {
                serde_json::json!({
                    "command": shell_join(&params.command),
                    "cwd": params.workdir,
                })
            }),
        "shell_command" | "exec_command" => {
            serde_json::from_str::<ShellCommandToolCallParams>(arguments)
                .ok()
                .map(|params| {
                    serde_json::json!({
                        "command": params.command,
                        "cwd": params.workdir,
                    })
                })
                .or_else(|| exec_command_tool_input(arguments))
        }
        _ => None,
    }
}

fn exec_command_tool_input(arguments: &str) -> Option<Value> {
    let parsed = serde_json::from_str::<Value>(arguments).ok()?;
    let command = parsed.get("cmd")?.as_str()?.to_string();
    let cwd = parsed
        .get("workdir")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(serde_json::json!({
        "command": command,
        "cwd": cwd,
    }))
}

fn shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '='))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r#"'"'"'"#))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Stdio;

    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use serde_json::to_string;
    use tempfile::tempdir;

    use super::*;

    const CWD: &str = "/tmp";
    const INPUT_MESSAGE: &str = "hello";

    fn hook_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterAgent {
                event: HookEventAfterAgent {
                    thread_id: ThreadId::new(),
                    turn_id: format!("turn-{label}"),
                    input_messages: vec![INPUT_MESSAGE.to_string()],
                    last_assistant_message: Some("hi".to_string()),
                },
            },
        }
    }

    fn user_prompt_submit_payload(messages: Vec<&str>) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::UserPromptSubmit {
                event: HookEventUserPromptSubmit {
                    turn_id: "turn-user".to_string(),
                    input_messages: messages.into_iter().map(str::to_string).collect(),
                },
            },
        }
    }

    fn counting_success_hook(calls: &Arc<AtomicUsize>, name: &str) -> Hook {
        let hook_name = name.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookExecutionOutcome::success()
                })
            }),
        }
    }

    fn failing_continue_hook(calls: &Arc<AtomicUsize>, name: &str, message: &str) -> Hook {
        let hook_name = name.to_string();
        let message = message.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                let message = message.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookExecutionOutcome {
                        result: HookResult::FailedContinue(std::io::Error::other(message).into()),
                        permission_decision: None,
                        updated_input: None,
                    }
                })
            }),
        }
    }

    fn failing_abort_hook(calls: &Arc<AtomicUsize>, name: &str, message: &str) -> Hook {
        let hook_name = name.to_string();
        let message = message.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                let message = message.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookExecutionOutcome {
                        result: HookResult::FailedAbort(std::io::Error::other(message).into()),
                        permission_decision: None,
                        updated_input: None,
                    }
                })
            }),
        }
    }

    fn after_tool_use_payload(label: &str) -> HookPayload {
        HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::AfterToolUse {
                event: HookEventAfterToolUse {
                    turn_id: format!("turn-{label}"),
                    call_id: format!("call-{label}"),
                    tool_name: "apply_patch".to_string(),
                    tool_kind: HookToolKind::Custom,
                    tool_input: HookToolInput::Custom {
                        input: "*** Begin Patch".to_string(),
                    },
                    executed: true,
                    success: true,
                    duration_ms: 1,
                    mutating: true,
                    sandbox: "none".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: "ok".to_string(),
                },
            },
        }
    }

    #[test]
    fn command_from_argv_returns_none_for_empty_args() {
        assert!(command_from_argv(&[]).is_none());
        assert!(command_from_argv(&["".to_string()]).is_none());
    }

    #[tokio::test]
    async fn command_from_argv_builds_command() -> Result<()> {
        let argv = if cfg!(windows) {
            vec![
                "cmd".to_string(),
                "/C".to_string(),
                "echo hello world".to_string(),
            ]
        } else {
            vec!["echo".to_string(), "hello".to_string(), "world".to_string()]
        };
        let mut command = command_from_argv(&argv).ok_or_else(|| anyhow::anyhow!("command"))?;
        let output = command.stdout(Stdio::piped()).output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let trimmed = stdout.trim_end_matches(['\r', '\n']);
        assert_eq!(trimmed, "hello world");
        Ok(())
    }

    #[test]
    fn hooks_new_requires_program_name() {
        assert!(Hooks::new(HooksConfig::default()).after_agent.is_empty());
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec![]),
                hooks: HooksToml::default(),
                ..HooksConfig::default()
            })
            .after_agent
            .is_empty()
        );
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["".to_string()]),
                hooks: HooksToml::default(),
                ..HooksConfig::default()
            })
            .after_agent
            .is_empty()
        );
        assert_eq!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["notify-send".to_string()]),
                hooks: HooksToml::default(),
                ..HooksConfig::default()
            })
            .after_agent
            .len(),
            1
        );
    }

    #[tokio::test]
    async fn dispatch_executes_hook() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![counting_success_hook(&calls, "counting")],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("1")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "counting");
        assert!(matches!(outcomes[0].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn default_hook_is_noop_and_continues() {
        let payload = hook_payload("d");
        let outcome = Hook::default().execute(&payload).await;
        assert_eq!(outcome.hook_name, "default");
        assert!(matches!(outcome.result, HookResult::Success));
    }

    #[tokio::test]
    async fn dispatch_executes_multiple_hooks_for_same_event() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                counting_success_hook(&calls, "one"),
                counting_success_hook(&calls, "two"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("2")).await;
        assert_eq!(outcomes.len(), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            outcomes
                .iter()
                .map(|outcome| outcome.hook_name.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }

    #[tokio::test]
    async fn dispatch_continues_after_failed_continue() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                failing_continue_hook(&calls, "warn", "boom"),
                counting_success_hook(&calls, "tail"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("3")).await;
        assert_eq!(outcomes.len(), 2);
        assert!(matches!(outcomes[0].result, HookResult::FailedContinue(_)));
        assert!(matches!(outcomes[1].result, HookResult::Success));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dispatch_stops_after_failed_abort() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_agent: vec![
                failing_abort_hook(&calls, "fatal", "stop"),
                counting_success_hook(&calls, "tail"),
            ],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(hook_payload("4")).await;
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::FailedAbort(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_routes_after_tool_use_hooks() {
        let calls = Arc::new(AtomicUsize::new(0));
        let hooks = Hooks {
            after_tool_use: vec![counting_success_hook(&calls, "tool")],
            ..Hooks::default()
        };

        let outcomes = hooks.dispatch(after_tool_use_payload("tool")).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].hook_name, "tool");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn legacy_notify_invokes_command_with_json_argument() -> Result<()> {
        let dir = tempdir()?;
        let output_path = dir.path().join("notify.json");
        let script_path = dir.path().join(if cfg!(windows) {
            "capture_notify.cmd"
        } else {
            "capture_notify.sh"
        });

        if cfg!(windows) {
            fs::write(
                &script_path,
                format!(
                    "@echo off\r\nset PAYLOAD=%~1\r\n>{} echo %PAYLOAD%\r\n",
                    output_path.display()
                ),
            )?;
        } else {
            fs::write(
                &script_path,
                format!(
                    "#!/bin/sh\nprintf '%s' \"$1\" > {}\n",
                    output_path.display()
                ),
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&script_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&script_path, perms)?;
            }
        }

        let payload = hook_payload("notify");
        let hooks = Hooks::new(HooksConfig {
            legacy_notify_argv: Some(vec![script_path.display().to_string()]),
            hooks: HooksToml::default(),
            ..HooksConfig::default()
        });

        let outcomes = hooks.dispatch(payload.clone()).await;
        assert_eq!(outcomes.len(), 1);
        assert!(matches!(outcomes[0].result, HookResult::Success));

        let json = timeout(Duration::from_secs(2), async {
            loop {
                if output_path.exists() {
                    return fs::read_to_string(&output_path).expect("read captured notify output");
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("wait for notify output");

        let expected = crate::legacy_notify_json(&payload)?;
        assert_eq!(json, expected);
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_serializes_after_tool_use_payload_to_json() {
        let payload = after_tool_use_payload("json");
        let serialized = to_string(&payload).expect("serialize hook payload");
        assert!(serialized.contains("\"event_type\":\"after_tool_use\""));
        assert!(serialized.contains("\"tool_name\":\"apply_patch\""));
    }

    #[test]
    fn matcher_matches_pipe_separated_tool_names() {
        let payload = after_tool_use_payload("matcher");
        assert!(matches_hook(Some("apply_patch|other"), &payload.hook_event));
        assert!(!matches_hook(Some("read_file|other"), &payload.hook_event));
        assert!(matches_hook(Some(""), &payload.hook_event));
    }

    #[test]
    fn matcher_supports_claude_tool_aliases() {
        let shell_payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-shell".to_string(),
                    call_id: "call-shell".to_string(),
                    tool_name: "shell".to_string(),
                    tool_kind: HookToolKind::LocalShell,
                    tool_input: HookToolInput::LocalShell {
                        params: crate::types::HookToolInputLocalShell {
                            command: vec!["dmidecode".to_string()],
                            workdir: Some(CWD.to_string()),
                            timeout_ms: Some(1000),
                            sandbox_permissions: None,
                            prefix_rule: None,
                            justification: None,
                        },
                    },
                    executed: false,
                    success: false,
                    duration_ms: 0,
                    mutating: false,
                    sandbox: "none".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: String::new(),
                },
            },
        };
        assert!(matches_hook(Some("Bash"), &shell_payload.hook_event));
        assert!(matches_hook(Some("Bash|other"), &shell_payload.hook_event));
        assert!(matches_hook(
            Some("Edit|Write"),
            &after_tool_use_payload("write").hook_event
        ));
    }

    #[test]
    fn hooks_new_builds_command_hooks_for_configured_events() {
        let hooks = Hooks::new(HooksConfig {
            legacy_notify_argv: None,
            hooks: HooksToml {
                pre_tool_use: vec![HookRuleConfig {
                    matcher: Some("apply_patch".to_string()),
                    commands: vec![CommandHookConfig {
                        command: "true".to_string(),
                        timeout_sec: None,
                    }],
                }],
                ..HooksToml::default()
            },
            ..HooksConfig::default()
        });
        assert_eq!(hooks.pre_tool_use.len(), 1);
    }

    #[test]
    fn hooks_stop_does_not_register_after_agent_hooks() {
        let hooks = Hooks::new(HooksConfig {
            hooks: HooksToml {
                stop: vec![HookRuleConfig {
                    matcher: Some(String::new()),
                    commands: vec![CommandHookConfig {
                        command: "true".to_string(),
                        timeout_sec: Some(5),
                    }],
                }],
                ..HooksToml::default()
            },
            ..HooksConfig::default()
        });

        assert!(hooks.after_agent.is_empty());
    }

    #[test]
    fn hooks_new_builds_session_start_handlers_from_toml_config() {
        let hooks = Hooks::new(HooksConfig {
            hooks: HooksToml {
                session_start: vec![HookRuleConfig {
                    matcher: Some("startup|resume".to_string()),
                    commands: vec![CommandHookConfig {
                        command: "true".to_string(),
                        timeout_sec: Some(5),
                    }],
                }],
                ..HooksToml::default()
            },
            ..HooksConfig::default()
        });

        let summaries = hooks.preview_session_start(&SessionStartRequest {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            transcript_path: None,
            model: "gpt-5.4".to_string(),
            permission_mode: "default".to_string(),
            source: crate::events::session_start::SessionStartSource::Startup,
        });

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].event_name, HookEventName::SessionStart);
        assert_eq!(summaries[0].source_path, PathBuf::from(CONFIG_TOML_FILE));
    }

    #[test]
    fn hooks_new_reports_invalid_session_start_matcher_warning() {
        let hooks = Hooks::new(HooksConfig {
            hooks: HooksToml {
                session_start: vec![HookRuleConfig {
                    matcher: Some("[".to_string()),
                    commands: vec![CommandHookConfig {
                        command: "true".to_string(),
                        timeout_sec: None,
                    }],
                }],
                ..HooksToml::default()
            },
            ..HooksConfig::default()
        });

        let summaries = hooks.preview_session_start(&SessionStartRequest {
            session_id: ThreadId::new(),
            cwd: PathBuf::from(CWD),
            transcript_path: None,
            model: "gpt-5.4".to_string(),
            permission_mode: "default".to_string(),
            source: crate::events::session_start::SessionStartSource::Startup,
        });

        assert!(summaries.is_empty());
        assert_eq!(
            hooks.startup_warnings(),
            &[String::from(
                "invalid matcher \"[\" in config.toml: regex parse error:\n    [\n    ^\nerror: unclosed character class"
            )]
        );
    }

    #[test]
    fn parse_command_output_denies_block_decisions() {
        let result = parse_command_output(br#"{"decision":"block","reason":"stop"}"#);
        assert!(matches!(result.result, HookResult::FailedAbort(_)));
    }

    #[test]
    fn parse_command_output_denies_permission_output() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"blocked"}}"#,
        );
        assert!(matches!(result.result, HookResult::FailedAbort(_)));
    }

    #[test]
    fn parse_command_output_ignores_advisory_json() {
        let result =
            parse_command_output(br#"{"continue":true,"stopReason":"Consider simplifying"}"#);
        assert!(matches!(result.result, HookResult::Success));
    }

    #[test]
    fn parse_command_output_recognizes_allow_and_ask() {
        let allow = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"allow","permissionDecisionReason":"ok"}}"#,
        );
        assert!(matches!(allow.result, HookResult::Success));
        assert_eq!(
            allow.permission_decision,
            Some(HookPermissionDecision::Allow {
                reason: "ok".to_string(),
            })
        );

        let ask = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"ask","permissionDecisionReason":"check"}}"#,
        );
        assert!(matches!(ask.result, HookResult::Success));
        assert_eq!(
            ask.permission_decision,
            Some(HookPermissionDecision::Ask {
                reason: "check".to_string(),
            })
        );
    }

    #[test]
    fn parse_command_output_extracts_updated_input() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"allow","permissionDecisionReason":"RTK auto-rewrite","updatedInput":{"command":"rtk git status","cwd":"/tmp"}}}"#,
        );
        assert!(matches!(result.result, HookResult::Success));
        assert!(result.updated_input.is_some());
        let updated = result.updated_input.unwrap();
        assert_eq!(updated["command"], "rtk git status");
    }

    #[test]
    fn parse_command_output_returns_none_updated_input_when_absent() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"allow","permissionDecisionReason":"ok"}}"#,
        );
        assert!(matches!(result.result, HookResult::Success));
        assert!(result.updated_input.is_none());
    }

    #[test]
    fn parse_command_output_extracts_updated_input_without_permission() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"updatedInput":{"command":"rtk git log"}}}"#,
        );
        assert!(matches!(result.result, HookResult::Success));
        assert!(result.permission_decision.is_none());
        assert!(result.updated_input.is_some());
        assert_eq!(result.updated_input.unwrap()["command"], "rtk git log");
    }

    #[test]
    fn parse_command_output_extracts_additional_context_as_updated_input() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"Relevant memories:\n- one"}} "#,
        );
        assert!(matches!(result.result, HookResult::Success));
        assert!(result.permission_decision.is_none());
        assert_eq!(
            result.updated_input,
            Some(serde_json::json!({
                "additional_context": "Relevant memories:\n- one"
            }))
        );
    }

    #[test]
    fn command_hook_input_adds_prompt_for_user_prompt_submit() {
        let payload = user_prompt_submit_payload(vec!["first", "second"]);

        let actual = command_hook_input(&payload);

        assert_eq!(actual["hook_event_name"], "UserPromptSubmit");
        assert_eq!(actual["prompt"], "first\n\nsecond");
    }

    #[test]
    fn command_hook_input_translates_shell_function_arguments_for_claude_hooks() {
        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from("/tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-shell".to_string(),
                    call_id: "call-shell".to_string(),
                    tool_name: "shell".to_string(),
                    tool_kind: HookToolKind::Function,
                    tool_input: HookToolInput::Function {
                        arguments: r#"{"command":["dmidecode"],"workdir":"/tmp"}"#.to_string(),
                    },
                    executed: false,
                    success: false,
                    duration_ms: 0,
                    mutating: false,
                    sandbox: "danger-full-access".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: String::new(),
                },
            },
        };

        let input = command_hook_input(&payload);
        assert_eq!(input["tool_name"], Value::String("Bash".to_string()));
        assert_eq!(
            input["tool_input"]["command"],
            Value::String("dmidecode".to_string())
        );
        assert_eq!(
            input["tool_input"]["cwd"],
            Value::String("/tmp".to_string())
        );
    }

    #[test]
    fn command_hook_input_translates_shell_command_arguments_for_claude_hooks() {
        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from("/tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-shell-command".to_string(),
                    call_id: "call-shell-command".to_string(),
                    tool_name: "shell_command".to_string(),
                    tool_kind: HookToolKind::Function,
                    tool_input: HookToolInput::Function {
                        arguments: r#"{"command":"dmidecode","workdir":"/tmp"}"#.to_string(),
                    },
                    executed: false,
                    success: false,
                    duration_ms: 0,
                    mutating: false,
                    sandbox: "danger-full-access".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: String::new(),
                },
            },
        };

        let input = command_hook_input(&payload);
        assert_eq!(input["tool_name"], Value::String("Bash".to_string()));
        assert_eq!(
            input["tool_input"]["command"],
            Value::String("dmidecode".to_string())
        );
        assert_eq!(
            input["tool_input"]["cwd"],
            Value::String("/tmp".to_string())
        );
    }

    #[test]
    fn command_hook_input_translates_exec_command_arguments_for_claude_hooks() {
        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: PathBuf::from("/tmp"),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-exec-command".to_string(),
                    call_id: "call-exec-command".to_string(),
                    tool_name: "exec_command".to_string(),
                    tool_kind: HookToolKind::Function,
                    tool_input: HookToolInput::Function {
                        arguments: r#"{"cmd":"dmidecode","workdir":"/tmp"}"#.to_string(),
                    },
                    executed: false,
                    success: false,
                    duration_ms: 0,
                    mutating: false,
                    sandbox: "danger-full-access".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: String::new(),
                },
            },
        };

        let input = command_hook_input(&payload);
        assert_eq!(input["tool_name"], Value::String("Bash".to_string()));
        assert_eq!(
            input["tool_input"]["command"],
            Value::String("dmidecode".to_string())
        );
        assert_eq!(
            input["tool_input"]["cwd"],
            Value::String("/tmp".to_string())
        );
    }

    #[test]
    fn command_hook_input_translates_single_file_apply_patch_for_claude_write_hooks() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("main.rs");
        fs::write(&file_path, "fn keep() {\n    println!(\"before\");\n}\n")
            .expect("write original file");

        let payload = HookPayload {
            session_id: ThreadId::new(),
            cwd: dir.path().to_path_buf(),
            client: None,
            triggered_at: Utc
                .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .single()
                .expect("valid timestamp"),
            hook_event: HookEvent::PreToolUse {
                event: HookEventAfterToolUse {
                    turn_id: "turn-apply-patch".to_string(),
                    call_id: "call-apply-patch".to_string(),
                    tool_name: "apply_patch".to_string(),
                    tool_kind: HookToolKind::Custom,
                    tool_input: HookToolInput::Custom {
                        input: "*** Begin Patch\n*** Update File: main.rs\n@@\n-fn keep() {\n-    println!(\"before\");\n-}\n+fn keep() {\n+    println!(\"after\");\n+}\n*** End Patch\n"
                            .to_string(),
                    },
                    executed: false,
                    success: false,
                    duration_ms: 0,
                    mutating: true,
                    sandbox: "danger-full-access".to_string(),
                    sandbox_policy: "danger-full-access".to_string(),
                    access_mode: "full_access".to_string(),
                    output_preview: String::new(),
                },
            },
        };

        let input = command_hook_input(&payload);
        assert_eq!(input["tool_name"], Value::String("Write".to_string()));
        assert_eq!(
            input["tool_input"]["file_path"],
            Value::String(file_path.display().to_string())
        );
        assert_eq!(
            input["tool_input"]["content"],
            Value::String("fn keep() {\n    println!(\"after\");\n}\n".to_string())
        );
    }
}
