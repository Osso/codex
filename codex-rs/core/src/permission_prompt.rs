use crate::config::Config;
use crate::config::edit::ConfigEdit;
use crate::config::edit::ConfigEditsBuilder;
use anyhow::Result;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerStackOrdering;
use codex_git_utils::get_git_repo_root;
use codex_mcp::permission_prompt::PermissionDestination;
use codex_mcp::permission_prompt::PermissionPromptDecision;
use codex_mcp::permission_prompt::PermissionPromptRequest;
use codex_mcp::permission_prompt::PermissionRuleBehavior;
use codex_mcp::permission_prompt::PermissionUpdate;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::protocol::ReviewDecision;
use codex_shell_command::parse_command::shlex_join;
use serde_json::Value;
use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use tokio::sync::Mutex;
use toml::Value as TomlValue;
use toml_edit::value;
use tracing::warn;

const BASH_PERMISSION_TOOL_NAME: &str = "Bash";
const CONFIG_LOCAL_TOML_FILE: &str = "config.local.toml";
const FILE_PATH_PERMISSION_TOOL_NAMES: [&str; 5] =
    ["Edit", "Write", "MultiEdit", "NotebookEdit", "Read"];

fn split_qualified_tool_name(name: &str) -> Option<(String, String)> {
    let remainder = name.strip_prefix("mcp__")?;
    let (server_name, tool_name) = remainder.split_once("__")?;
    if server_name.is_empty() || tool_name.is_empty() {
        return None;
    }
    Some((server_name.to_string(), tool_name.to_string()))
}
const PERMISSION_PROMPT_RULES_TABLE_KEY: &str = "permission_prompt_rules";
const PERMISSION_PROMPT_RULE_KEY_PREFIX: &str = "content:";

fn build_permission_prompt_input(command: &[String], cwd: &Path, reason: Option<&str>) -> Value {
    let mut input = serde_json::json!({
        "command": shlex_join(command),
        "argv": command,
        "cwd": cwd.to_string_lossy().to_string(),
    });
    if let Some(reason) = reason
        && let Some(input) = input.as_object_mut()
    {
        input.insert("reason".to_string(), Value::String(reason.to_string()));
    }
    input
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PermissionPromptSessionRule {
    tool_name: String,
    rule_content: String,
}

#[derive(Debug, Clone)]
struct PermissionPromptPersistenceContext {
    codex_home: PathBuf,
    repo_dot_codex_folder: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub(crate) struct PermissionPromptSessionRuleCache {
    always_allow: HashSet<PermissionPromptSessionRule>,
    persistence_context: Option<PermissionPromptPersistenceContext>,
}

impl PermissionPromptSessionRuleCache {
    pub(crate) fn from_config(config: &Config) -> Self {
        let persistence_context = PermissionPromptPersistenceContext {
            codex_home: config.codex_home.to_path_buf(),
            repo_dot_codex_folder: get_git_repo_root(&config.cwd).map(|root| root.join(".codex")),
        };
        let mut always_allow = HashSet::new();
        for layer in config.config_layer_stack.get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ false,
        ) {
            apply_persistent_allow_rules_from_toml_value(&layer.config, &mut always_allow);
        }
        if let Some(repo_dot_codex_folder) = persistence_context.repo_dot_codex_folder.as_ref() {
            let local_settings_path = repo_dot_codex_folder.join(CONFIG_LOCAL_TOML_FILE);
            let local_settings_contents = match std::fs::read_to_string(&local_settings_path) {
                Ok(contents) => Some(contents),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                Err(err) => {
                    warn!(
                        "failed to read persisted permission prompt rules at {}: {err:#}",
                        local_settings_path.display()
                    );
                    None
                }
            };

            if let Some(local_settings_contents) = local_settings_contents {
                match toml::from_str::<TomlValue>(&local_settings_contents) {
                    Ok(local_config) => {
                        apply_persistent_allow_rules_from_toml_value(
                            &local_config,
                            &mut always_allow,
                        );
                    }
                    Err(err) => {
                        warn!(
                            "failed to parse persisted permission prompt rules at {}: {err:#}",
                            local_settings_path.display()
                        );
                    }
                }
            }
        }

        Self {
            always_allow,
            persistence_context: Some(persistence_context),
        }
    }

    fn is_allowed(&self, rule: &PermissionPromptSessionRule) -> bool {
        self.always_allow.contains(rule)
    }

    fn remember_allow_rule(&mut self, rule: PermissionPromptSessionRule) {
        self.always_allow.insert(rule);
    }
}

fn persistent_rule_content_key(rule_content: &str) -> String {
    format!("{PERMISSION_PROMPT_RULE_KEY_PREFIX}{rule_content}")
}

fn rule_content_from_persistent_rule_key(key: &str) -> Option<String> {
    key.strip_prefix(PERMISSION_PROMPT_RULE_KEY_PREFIX)
        .map(ToOwned::to_owned)
}

fn apply_persistent_allow_rules_from_toml_value(
    config_toml_value: &TomlValue,
    always_allow: &mut HashSet<PermissionPromptSessionRule>,
) {
    let Some(tool_table) = config_toml_value
        .as_table()
        .and_then(|table| table.get(PERMISSION_PROMPT_RULES_TABLE_KEY))
        .and_then(TomlValue::as_table)
    else {
        return;
    };

    for (tool_name, rules_value) in tool_table {
        let tool_name = tool_name.trim();
        if tool_name.is_empty() {
            continue;
        }
        let Some(rules_table) = rules_value.as_table() else {
            continue;
        };

        for (persisted_key, enabled_value) in rules_table {
            let Some(enabled) = enabled_value.as_bool() else {
                continue;
            };
            let Some(rule_content) = rule_content_from_persistent_rule_key(persisted_key) else {
                continue;
            };
            let rule = PermissionPromptSessionRule {
                tool_name: tool_name.to_string(),
                rule_content,
            };
            if enabled {
                always_allow.insert(rule);
            } else {
                always_allow.remove(&rule);
            }
        }
    }
}

fn permission_prompt_session_rule_for_invocation(
    tool_name: &str,
    input: &Value,
) -> Option<PermissionPromptSessionRule> {
    if tool_name.trim().is_empty() {
        return None;
    }

    let rule_content = if tool_name == BASH_PERMISSION_TOOL_NAME {
        input.get("command")?.as_str()?.to_string()
    } else if FILE_PATH_PERMISSION_TOOL_NAMES.contains(&tool_name) {
        input.get("file_path")?.as_str()?.to_string()
    } else {
        String::new()
    };

    Some(PermissionPromptSessionRule {
        tool_name: tool_name.to_string(),
        rule_content,
    })
}

fn permission_prompt_session_rule_from_updated_permission(
    rule: &Value,
) -> Option<PermissionPromptSessionRule> {
    let tool_name = rule
        .get("toolName")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if tool_name.is_empty() {
        return None;
    }

    let rule_content = match rule.get("ruleContent") {
        None | Some(Value::Null) => String::new(),
        Some(rule_content) => rule_content.as_str()?.to_string(),
    };

    Some(PermissionPromptSessionRule {
        tool_name,
        rule_content,
    })
}

fn permission_prompt_allow_rule_updates(
    decision: &PermissionPromptDecision,
) -> Vec<(PermissionDestination, PermissionPromptSessionRule)> {
    let updated_permissions = match decision {
        PermissionPromptDecision::Allow {
            updated_permissions,
            ..
        } => updated_permissions,
        PermissionPromptDecision::Deny {
            updated_permissions,
            ..
        } => updated_permissions,
    };
    let Some(updated_permissions) = updated_permissions.as_deref() else {
        return Vec::new();
    };

    let mut updates = Vec::new();
    for updated_permission in updated_permissions {
        let PermissionUpdate::AddRules {
            destination,
            behavior,
            rules,
        } = updated_permission;
        if *behavior != PermissionRuleBehavior::Allow {
            continue;
        }

        for rule in rules {
            let Some(rule) = permission_prompt_session_rule_from_updated_permission(rule) else {
                continue;
            };
            updates.push((*destination, rule));
        }
    }

    updates
}

fn repo_dot_codex_folder_for_destination(
    invocation_cwd: &Path,
    persistence_context: &PermissionPromptPersistenceContext,
) -> Option<PathBuf> {
    get_git_repo_root(invocation_cwd)
        .map(|repo_root| repo_root.join(".codex"))
        .or_else(|| persistence_context.repo_dot_codex_folder.clone())
}

async fn persist_allow_rule_for_destination(
    destination: PermissionDestination,
    rule: &PermissionPromptSessionRule,
    invocation_cwd: &Path,
    persistence_context: &PermissionPromptPersistenceContext,
) -> Result<()> {
    let (config_folder, config_toml_file) = match destination {
        PermissionDestination::Session => return Ok(()),
        PermissionDestination::UserSettings => {
            (persistence_context.codex_home.clone(), CONFIG_TOML_FILE)
        }
        PermissionDestination::ProjectSettings | PermissionDestination::LocalSettings => {
            let Some(config_folder) =
                repo_dot_codex_folder_for_destination(invocation_cwd, persistence_context)
            else {
                anyhow::bail!(
                    "cannot persist permission prompt rule for `{destination:?}` without a repository root"
                );
            };
            let config_toml_file = if destination == PermissionDestination::ProjectSettings {
                CONFIG_TOML_FILE
            } else {
                CONFIG_LOCAL_TOML_FILE
            };
            (config_folder, config_toml_file)
        }
    };

    tokio::fs::create_dir_all(&config_folder).await?;
    ConfigEditsBuilder::new(&config_folder)
        .with_config_toml_file(config_toml_file)
        .with_edits([ConfigEdit::SetPath {
            segments: vec![
                PERMISSION_PROMPT_RULES_TABLE_KEY.to_string(),
                rule.tool_name.clone(),
                persistent_rule_content_key(&rule.rule_content),
            ],
            value: value(true),
        }])
        .apply()
        .await
}

async fn apply_permission_prompt_rule_updates(
    decision: &PermissionPromptDecision,
    invocation_cwd: &Path,
    session_rule_cache: &Mutex<PermissionPromptSessionRuleCache>,
) {
    let updates = permission_prompt_allow_rule_updates(decision);
    if updates.is_empty() {
        return;
    }

    let persistence_context = {
        let cache = session_rule_cache.lock().await;
        cache.persistence_context.clone()
    };

    for (destination, rule) in updates {
        if destination == PermissionDestination::Session {
            let mut cache = session_rule_cache.lock().await;
            cache.remember_allow_rule(rule);
            continue;
        }

        let Some(persistence_context) = persistence_context.as_ref() else {
            warn!(
                "cannot persist permission prompt rule for destination `{destination:?}` because persistence context is unavailable"
            );
            continue;
        };

        if let Err(err) = persist_allow_rule_for_destination(
            destination,
            &rule,
            invocation_cwd,
            persistence_context,
        )
        .await
        {
            warn!(
                "failed to persist permission prompt rule for destination `{destination:?}`: {err:#}"
            );
        }

        let mut cache = session_rule_cache.lock().await;
        cache.remember_allow_rule(rule);
    }
}

/// Query the configured MCP permission-prompt tool for an exec approval decision.
///
/// Returns `Some(ReviewDecision)` when the tool produced a valid allow/deny
/// decision. Returns `None` when the caller should fall back to the interactive
/// approval prompt (tool not configured, missing, timed out, or returned
/// malformed output).
pub(crate) async fn maybe_decide_command_approval_with_permission_prompt_tool<F, Fut>(
    configured_tool: Option<&str>,
    call_id: &str,
    command: &[String],
    cwd: &Path,
    reason: Option<&str>,
    session_rule_cache: &Mutex<PermissionPromptSessionRuleCache>,
    call_tool: F,
) -> Option<ReviewDecision>
where
    F: FnOnce(String, String, Value) -> Fut,
    Fut: Future<Output = Result<CallToolResult>>,
{
    let configured_tool = configured_tool?;
    let Some((server_name, tool_name)) = split_qualified_tool_name(configured_tool) else {
        warn!(
            "permission_prompt_tool value `{configured_tool}` is invalid; expected `mcp__server__tool`"
        );
        return None;
    };

    let input = build_permission_prompt_input(command, cwd, reason);
    let invocation_rule =
        permission_prompt_session_rule_for_invocation(BASH_PERMISSION_TOOL_NAME, &input);
    if let Some(invocation_rule) = invocation_rule {
        let cache = session_rule_cache.lock().await;
        if cache.is_allowed(&invocation_rule) {
            return Some(ReviewDecision::Approved);
        }
    }

    let request = PermissionPromptRequest {
        tool_name: BASH_PERMISSION_TOOL_NAME.to_string(),
        input,
        tool_use_id: Some(call_id.to_string()),
    };
    let arguments = match request.to_tool_arguments() {
        Ok(arguments) => arguments,
        Err(err) => {
            warn!("failed to serialize permission prompt request for `{configured_tool}`: {err:#}");
            return None;
        }
    };

    let result = match call_tool(server_name, tool_name, arguments).await {
        Ok(result) => result,
        Err(err) => {
            warn!(
                "permission prompt tool `{configured_tool}` failed; falling back to interactive approval: {err:#}"
            );
            return None;
        }
    };

    let decision = match PermissionPromptDecision::from_call_tool_result(&result) {
        Ok(decision) => decision,
        Err(err) => {
            warn!(
                "permission prompt tool `{configured_tool}` returned an invalid decision; falling back to interactive approval: {err:#}"
            );
            return None;
        }
    };

    apply_permission_prompt_rule_updates(&decision, cwd, session_rule_cache).await;

    match decision {
        PermissionPromptDecision::Allow { .. } => Some(ReviewDecision::Approved),
        PermissionPromptDecision::Deny { message, .. } => {
            warn!("permission prompt tool denied command approval: {message}");
            Some(ReviewDecision::Denied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::tempdir;
    use toml::Value as TomlValue;

    fn empty_cache() -> Mutex<PermissionPromptSessionRuleCache> {
        Mutex::new(PermissionPromptSessionRuleCache::default())
    }

    fn cache_with_persistence_context(
        codex_home: &Path,
        repo_dot_codex_folder: Option<PathBuf>,
    ) -> Mutex<PermissionPromptSessionRuleCache> {
        Mutex::new(PermissionPromptSessionRuleCache {
            always_allow: HashSet::new(),
            persistence_context: Some(PermissionPromptPersistenceContext {
                codex_home: codex_home.to_path_buf(),
                repo_dot_codex_folder,
            }),
        })
    }

    #[tokio::test]
    async fn no_configured_tool_returns_none() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();
        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            None,
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { unreachable!("tool should not be called") },
        )
        .await;
        assert_eq!(decision, None);
    }

    #[tokio::test]
    async fn invalid_configured_tool_returns_none() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();
        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { unreachable!("tool should not be called") },
        )
        .await;
        assert_eq!(decision, None);
    }

    #[tokio::test]
    async fn allow_decision_returns_approved() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ];
        let expected_command = shlex_join(&command);
        let expected_argv = command.clone();
        let expected_cwd = cwd.path().to_string_lossy().to_string();
        let cache = empty_cache();
        let call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: None,
        }
        .to_call_tool_result()
        .expect("serialize decision");

        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            Some("retry without sandbox"),
            &cache,
            move |server, tool, arguments| async move {
                assert_eq!(server, "approval_server");
                assert_eq!(tool, "approval_prompt");
                assert_eq!(arguments["tool_name"], serde_json::json!("Bash"));
                assert_eq!(arguments["tool_use_id"], serde_json::json!("call_1"));
                assert_eq!(
                    arguments["input"]["command"],
                    serde_json::json!(expected_command)
                );
                assert_eq!(arguments["input"]["argv"], serde_json::json!(expected_argv));
                assert_eq!(arguments["input"]["cwd"], serde_json::json!(expected_cwd));
                assert_eq!(
                    arguments["input"]["reason"],
                    serde_json::json!("retry without sandbox")
                );
                Ok(call_result)
            },
        )
        .await;

        assert_eq!(decision, Some(ReviewDecision::Approved));
    }

    #[tokio::test]
    async fn allow_decision_accepts_updated_input_contract() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();
        let call_result = PermissionPromptDecision::Allow {
            updated_input: Some(serde_json::json!({
                "sandbox_permissions": "require_escalated",
            })),
            updated_permissions: None,
        }
        .to_call_tool_result()
        .expect("serialize decision");

        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { Ok(call_result) },
        )
        .await;

        assert_eq!(decision, Some(ReviewDecision::Approved));
    }

    #[tokio::test]
    async fn deny_decision_returns_denied() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();
        let call_result = PermissionPromptDecision::Deny {
            message: "blocked by policy".to_string(),
            updated_permissions: None,
        }
        .to_call_tool_result()
        .expect("serialize decision");

        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { Ok(call_result) },
        )
        .await;

        assert_eq!(decision, Some(ReviewDecision::Denied));
    }

    #[tokio::test]
    async fn call_tool_errors_fall_back_to_interactive_prompt() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();

        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { Err(anyhow!("timed out")) },
        )
        .await;

        assert_eq!(decision, None);
    }

    #[tokio::test]
    async fn malformed_tool_response_falls_back_to_interactive_prompt() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec!["echo".to_string(), "hello".to_string()];
        let cache = empty_cache();
        let malformed_result = CallToolResult {
            content: vec![serde_json::json!({
                "type": "text",
                "text": "not json",
            })],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        };

        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { Ok(malformed_result) },
        )
        .await;

        assert_eq!(decision, None);
    }

    #[tokio::test]
    async fn session_allow_rule_suppresses_followup_prompt() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ];
        let cache = empty_cache();
        let invocation_count = Arc::new(AtomicUsize::new(0));
        let rule_content = shlex_join(&command);
        let call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: Some(vec![PermissionUpdate::AddRules {
                destination: PermissionDestination::Session,
                behavior: PermissionRuleBehavior::Allow,
                rules: vec![serde_json::json!({
                    "toolName": BASH_PERMISSION_TOOL_NAME,
                    "ruleContent": rule_content,
                })],
            }]),
        }
        .to_call_tool_result()
        .expect("serialize decision");
        let first_count = Arc::clone(&invocation_count);
        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                first_count.fetch_add(1, Ordering::SeqCst);
                Ok(call_result)
            },
        )
        .await;
        assert_eq!(decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 1);

        let second_count = Arc::clone(&invocation_count);
        let cached_decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_2",
            &command,
            cwd.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                second_count.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("cached rule should skip tool invocation"))
            },
        )
        .await;
        assert_eq!(cached_decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn non_session_rule_does_not_suppress_followup_prompt() {
        let cwd = tempdir().expect("create tempdir");
        let command = vec![
            "bash".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ];
        let cache = empty_cache();
        let invocation_count = Arc::new(AtomicUsize::new(0));
        let rule_content = shlex_join(&command);
        let call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: Some(vec![PermissionUpdate::AddRules {
                destination: PermissionDestination::UserSettings,
                behavior: PermissionRuleBehavior::Allow,
                rules: vec![serde_json::json!({
                    "toolName": BASH_PERMISSION_TOOL_NAME,
                    "ruleContent": rule_content,
                })],
            }]),
        }
        .to_call_tool_result()
        .expect("serialize decision");
        let first_count = Arc::clone(&invocation_count);
        let first_decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            cwd.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                first_count.fetch_add(1, Ordering::SeqCst);
                Ok(call_result)
            },
        )
        .await;
        assert_eq!(first_decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 1);

        let second_call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: None,
        }
        .to_call_tool_result()
        .expect("serialize decision");
        let second_count = Arc::clone(&invocation_count);
        let second_decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_2",
            &command,
            cwd.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                second_count.fetch_add(1, Ordering::SeqCst);
                Ok(second_call_result)
            },
        )
        .await;
        assert_eq!(second_decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn user_settings_rule_persists_and_suppresses_followup_prompt() {
        let codex_home = tempdir().expect("create codex home");
        let repo = tempdir().expect("create repo");
        std::fs::write(repo.path().join(".git"), "gitdir: nowhere").expect("seed git marker");
        let cache =
            cache_with_persistence_context(codex_home.path(), Some(repo.path().join(".codex")));
        let command = vec!["echo".to_string(), "hello".to_string()];
        let rule_content = shlex_join(&command);
        let invocation_count = Arc::new(AtomicUsize::new(0));
        let call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: Some(vec![PermissionUpdate::AddRules {
                destination: PermissionDestination::UserSettings,
                behavior: PermissionRuleBehavior::Allow,
                rules: vec![serde_json::json!({
                    "toolName": BASH_PERMISSION_TOOL_NAME,
                    "ruleContent": rule_content,
                })],
            }]),
        }
        .to_call_tool_result()
        .expect("serialize decision");
        let first_count = Arc::clone(&invocation_count);
        let first_decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            repo.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                first_count.fetch_add(1, Ordering::SeqCst);
                Ok(call_result)
            },
        )
        .await;
        assert_eq!(first_decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 1);

        let persisted = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))
            .expect("read user config");
        let parsed: TomlValue = toml::from_str(&persisted).expect("parse user config");
        let enabled = parsed
            .as_table()
            .and_then(|table| table.get(PERMISSION_PROMPT_RULES_TABLE_KEY))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(BASH_PERMISSION_TOOL_NAME))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(&persistent_rule_content_key("echo hello")))
            .and_then(TomlValue::as_bool);
        assert_eq!(enabled, Some(true));

        let second_count = Arc::clone(&invocation_count);
        let cached_decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_2",
            &command,
            repo.path(),
            None,
            &cache,
            move |_server, _tool, _arguments| async move {
                second_count.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("cached rule should skip tool invocation"))
            },
        )
        .await;
        assert_eq!(cached_decision, Some(ReviewDecision::Approved));
        assert_eq!(invocation_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn project_and_local_settings_rules_persist_to_repo_codex_files() {
        let codex_home = tempdir().expect("create codex home");
        let repo = tempdir().expect("create repo");
        std::fs::write(repo.path().join(".git"), "gitdir: nowhere").expect("seed git marker");
        let repo_codex = repo.path().join(".codex");
        std::fs::create_dir_all(&repo_codex).expect("create repo .codex");
        std::fs::write(repo_codex.join(CONFIG_TOML_FILE), "# keep me\n")
            .expect("seed project config");
        let cache = cache_with_persistence_context(codex_home.path(), Some(repo_codex.clone()));
        let command = vec!["echo".to_string(), "hello".to_string()];
        let rule_content = shlex_join(&command);
        let call_result = PermissionPromptDecision::Allow {
            updated_input: None,
            updated_permissions: Some(vec![
                PermissionUpdate::AddRules {
                    destination: PermissionDestination::ProjectSettings,
                    behavior: PermissionRuleBehavior::Allow,
                    rules: vec![serde_json::json!({
                        "toolName": BASH_PERMISSION_TOOL_NAME,
                        "ruleContent": rule_content,
                    })],
                },
                PermissionUpdate::AddRules {
                    destination: PermissionDestination::LocalSettings,
                    behavior: PermissionRuleBehavior::Allow,
                    rules: vec![serde_json::json!({
                        "toolName": BASH_PERMISSION_TOOL_NAME,
                        "ruleContent": "echo hello",
                    })],
                },
            ]),
        }
        .to_call_tool_result()
        .expect("serialize decision");
        let decision = maybe_decide_command_approval_with_permission_prompt_tool(
            Some("mcp__approval_server__approval_prompt"),
            "call_1",
            &command,
            repo.path(),
            None,
            &cache,
            |_server, _tool, _arguments| async move { Ok(call_result) },
        )
        .await;
        assert_eq!(decision, Some(ReviewDecision::Approved));

        let project_contents = std::fs::read_to_string(repo_codex.join(CONFIG_TOML_FILE))
            .expect("read project config");
        assert!(project_contents.contains("# keep me"));
        let project_toml: TomlValue =
            toml::from_str(&project_contents).expect("parse project config");
        let project_enabled = project_toml
            .as_table()
            .and_then(|table| table.get(PERMISSION_PROMPT_RULES_TABLE_KEY))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(BASH_PERMISSION_TOOL_NAME))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(&persistent_rule_content_key("echo hello")))
            .and_then(TomlValue::as_bool);
        assert_eq!(project_enabled, Some(true));

        let local_contents = std::fs::read_to_string(repo_codex.join(CONFIG_LOCAL_TOML_FILE))
            .expect("read local settings");
        let local_toml: TomlValue = toml::from_str(&local_contents).expect("parse local settings");
        let local_enabled = local_toml
            .as_table()
            .and_then(|table| table.get(PERMISSION_PROMPT_RULES_TABLE_KEY))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(BASH_PERMISSION_TOOL_NAME))
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get(&persistent_rule_content_key("echo hello")))
            .and_then(TomlValue::as_bool);
        assert_eq!(local_enabled, Some(true));
    }
}
