use std::io;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;

use crate::types::CommandHookConfig;
use crate::types::Hook;
use crate::types::HookEvent;
use crate::types::HookPayload;
use crate::types::HookResponse;
use crate::types::HookResult;
use crate::types::HookRuleConfig;
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
use crate::types::HookToolInput;
#[cfg(test)]
use crate::types::HookToolKind;

#[derive(Default, Clone)]
pub struct HooksConfig {
    pub legacy_notify_argv: Option<Vec<String>>,
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
}

impl Default for Hooks {
    fn default() -> Self {
        Self::new(HooksConfig::default())
    }
}

impl Hooks {
    pub fn new(config: HooksConfig) -> Self {
        let after_agent = config
            .legacy_notify_argv
            .filter(|argv| !argv.is_empty() && !argv[0].is_empty())
            .map(crate::notify_hook)
            .into_iter()
            .chain(build_command_hooks("stop", &config.hooks.stop))
            .collect();
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
        }
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
                    return HookResult::Success;
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
        .any(|part| part == subject)
}

async fn run_command_hook(
    command_text: &str,
    timeout_sec: Option<u64>,
    payload: &HookPayload,
) -> HookResult {
    let mut command = shell_command(command_text);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("CODEX_HOOK_EVENT", payload.hook_event.name());
    if let Some(tool_name) = payload.hook_event.tool_name() {
        command.env("CLAUDE_TOOL_NAME", tool_name);
    }

    let payload_json = match serde_json::to_vec(payload) {
        Ok(payload_json) => payload_json,
        Err(err) => return HookResult::FailedContinue(err.into()),
    };

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => return HookResult::FailedContinue(err.into()),
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
                Ok(Err(err)) => return HookResult::FailedContinue(err.into()),
                Err(_) => {
                    return HookResult::FailedContinue(
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("hook timed out after {seconds}s"),
                        )
                        .into(),
                    );
                }
            }
        }
        None => match child.wait_with_output().await {
            Ok(output) => output,
            Err(err) => return HookResult::FailedContinue(err.into()),
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
        return HookResult::FailedContinue(io::Error::other(message).into());
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

fn parse_command_output(stdout: &[u8]) -> HookResult {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return HookResult::Success;
    }
    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        return HookResult::Success;
    };

    if json.get("decision").and_then(Value::as_str) == Some("block") {
        let reason = json
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("hook blocked operation");
        return HookResult::FailedAbort(io::Error::other(reason.to_string()).into());
    }

    let maybe_deny = json
        .get("hookSpecificOutput")
        .and_then(Value::as_object)
        .and_then(|obj| {
            (obj.get("permissionDecision").and_then(Value::as_str) == Some("deny")).then(|| {
                obj.get("permissionDecisionReason")
                    .and_then(Value::as_str)
                    .unwrap_or("hook denied operation")
                    .to_string()
            })
        });
    if let Some(reason) = maybe_deny {
        return HookResult::FailedAbort(io::Error::other(reason).into());
    }

    HookResult::Success
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

    fn counting_success_hook(calls: &Arc<AtomicUsize>, name: &str) -> Hook {
        let hook_name = name.to_string();
        let calls = Arc::clone(calls);
        Hook {
            name: hook_name,
            func: Arc::new(move |_| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    HookResult::Success
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
                    HookResult::FailedContinue(std::io::Error::other(message).into())
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
                    HookResult::FailedAbort(std::io::Error::other(message).into())
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
            })
            .after_agent
            .is_empty()
        );
        assert!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["".to_string()]),
                hooks: HooksToml::default(),
            })
            .after_agent
            .is_empty()
        );
        assert_eq!(
            Hooks::new(HooksConfig {
                legacy_notify_argv: Some(vec!["notify-send".to_string()]),
                hooks: HooksToml::default(),
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
        });
        assert_eq!(hooks.pre_tool_use.len(), 1);
    }

    #[test]
    fn parse_command_output_denies_block_decisions() {
        let result = parse_command_output(br#"{"decision":"block","reason":"stop"}"#);
        assert!(matches!(result, HookResult::FailedAbort(_)));
    }

    #[test]
    fn parse_command_output_denies_permission_output() {
        let result = parse_command_output(
            br#"{"hookSpecificOutput":{"permissionDecision":"deny","permissionDecisionReason":"blocked"}}"#,
        );
        assert!(matches!(result, HookResult::FailedAbort(_)));
    }

    #[test]
    fn parse_command_output_ignores_advisory_json() {
        let result =
            parse_command_output(br#"{"continue":true,"stopReason":"Consider simplifying"}"#);
        assert!(matches!(result, HookResult::Success));
    }
}
