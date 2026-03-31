use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use regex::Regex;

use super::ConfiguredHandler;
use super::config::HookHandlerConfig;
use super::config::HooksFile;
use super::config::MatcherGroup;
use crate::types::HookRuleConfig;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub warnings: Vec<String>,
}

pub(crate) fn discover_handlers(config_layer_stack: Option<&ConfigLayerStack>) -> DiscoveryResult {
    let Some(config_layer_stack) = config_layer_stack else {
        return DiscoveryResult {
            handlers: Vec::new(),
            warnings: Vec::new(),
        };
    };

    let mut handlers = Vec::new();
    let mut warnings = Vec::new();
    let mut display_order = 0_i64;

    for layer in
        config_layer_stack.get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, false)
    {
        if let Some(folder) = layer.config_folder() {
            discover_layer(
                folder.as_path(),
                &mut handlers,
                &mut warnings,
                &mut display_order,
            );
        }
    }

    DiscoveryResult { handlers, warnings }
}

fn discover_layer(
    folder: &Path,
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
) {
    let Some(source_path) = hooks_config_path(folder) else {
        return;
    };
    let Some(parsed) = load_hooks_file(source_path.as_path(), warnings) else {
        return;
    };

    append_discovered_groups(
        handlers,
        warnings,
        display_order,
        source_path.as_path(),
        codex_protocol::protocol::HookEventName::SessionStart,
        parsed.hooks.session_start,
    );
    append_discovered_groups(
        handlers,
        warnings,
        display_order,
        source_path.as_path(),
        codex_protocol::protocol::HookEventName::Stop,
        parsed.hooks.stop,
    );
}

fn hooks_config_path(folder: &Path) -> Option<PathBuf> {
    let source_path = folder.join("hooks.json");
    source_path.as_path().is_file().then_some(source_path)
}

fn load_hooks_file(source_path: &Path, warnings: &mut Vec<String>) -> Option<HooksFile> {
    let contents = match fs::read_to_string(source_path) {
        Ok(contents) => contents,
        Err(err) => {
            warnings.push(format!(
                "failed to read hooks config {}: {err}",
                source_path.display()
            ));
            return None;
        }
    };

    match serde_json::from_str(&contents) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            warnings.push(format!(
                "failed to parse hooks config {}: {err}",
                source_path.display()
            ));
            None
        }
    }
}

fn append_discovered_groups(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    groups: Vec<MatcherGroup>,
) {
    for group in groups {
        let MatcherGroup { matcher, hooks } = group;
        append_group_handlers(
            handlers,
            warnings,
            display_order,
            source_path,
            event_name,
            group_matcher(event_name, matcher.as_deref()),
            hooks,
        );
    }
}

fn group_matcher(
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
) -> Option<&str> {
    match event_name {
        codex_protocol::protocol::HookEventName::SessionStart => matcher,
        codex_protocol::protocol::HookEventName::Stop => None,
    }
}

pub(crate) fn discover_toml_session_start_handlers(
    source_path: &Path,
    rules: &[HookRuleConfig],
) -> DiscoveryResult {
    discover_toml_handlers(
        source_path,
        rules,
        codex_protocol::protocol::HookEventName::SessionStart,
    )
}

pub(crate) fn discover_toml_stop_handlers(
    source_path: &Path,
    rules: &[HookRuleConfig],
) -> DiscoveryResult {
    discover_toml_handlers(
        source_path,
        rules,
        codex_protocol::protocol::HookEventName::Stop,
    )
}

fn discover_toml_handlers(
    source_path: &Path,
    rules: &[HookRuleConfig],
    event_name: codex_protocol::protocol::HookEventName,
) -> DiscoveryResult {
    let mut handlers = Vec::new();
    let mut warnings = Vec::new();
    let mut display_order = 0_i64;

    for rule in rules {
        if let Some(matcher) = rule.matcher.as_deref()
            && let Err(err) = Regex::new(matcher)
        {
            warnings.push(format!(
                "invalid matcher {matcher:?} in {}: {err}",
                source_path.display()
            ));
            continue;
        }

        for command in &rule.commands {
            if command.command.trim().is_empty() {
                warnings.push(format!(
                    "skipping empty hook command in {}",
                    source_path.display()
                ));
                continue;
            }
            handlers.push(ConfiguredHandler {
                event_name,
                matcher: rule.matcher.clone(),
                command: command.command.clone(),
                timeout_sec: command.timeout_sec.unwrap_or(600).max(1),
                status_message: command_label(&command.command),
                source_path: source_path.to_path_buf(),
                display_order,
            });
            display_order += 1;
        }
    }

    DiscoveryResult { handlers, warnings }
}

fn append_group_handlers(
    handlers: &mut Vec<ConfiguredHandler>,
    warnings: &mut Vec<String>,
    display_order: &mut i64,
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    group_handlers: Vec<HookHandlerConfig>,
) {
    if let Some(matcher) = matcher
        && let Err(err) = Regex::new(matcher)
    {
        warnings.push(format!(
            "invalid matcher {matcher:?} in {}: {err}",
            source_path.display()
        ));
        return;
    }

    for handler in group_handlers {
        if let Some(configured) = configured_handler(
            source_path,
            event_name,
            matcher,
            *display_order,
            handler,
            warnings,
        ) {
            handlers.push(configured);
            *display_order += 1;
        }
    }
}

fn configured_handler(
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    display_order: i64,
    handler: HookHandlerConfig,
    warnings: &mut Vec<String>,
) -> Option<ConfiguredHandler> {
    match handler {
        HookHandlerConfig::Command {
            command,
            timeout_sec,
            r#async,
            status_message,
        } => build_command_handler(
            source_path,
            event_name,
            matcher,
            display_order,
            command,
            timeout_sec,
            r#async,
            status_message,
            warnings,
        ),
        HookHandlerConfig::Prompt {} => {
            push_unsupported_hook_warning(source_path, warnings, "prompt");
            None
        }
        HookHandlerConfig::Agent {} => {
            push_unsupported_hook_warning(source_path, warnings, "agent");
            None
        }
    }
}

fn build_command_handler(
    source_path: &Path,
    event_name: codex_protocol::protocol::HookEventName,
    matcher: Option<&str>,
    display_order: i64,
    command: String,
    timeout_sec: Option<u64>,
    is_async: bool,
    status_message: Option<String>,
    warnings: &mut Vec<String>,
) -> Option<ConfiguredHandler> {
    if is_async {
        warnings.push(format!(
            "skipping async hook in {}: async hooks are not supported yet",
            source_path.display()
        ));
        return None;
    }
    if command.trim().is_empty() {
        warnings.push(format!(
            "skipping empty hook command in {}",
            source_path.display()
        ));
        return None;
    }

    let status_message = status_message.or_else(|| command_label(&command));
    Some(ConfiguredHandler {
        event_name,
        matcher: matcher.map(ToOwned::to_owned),
        command,
        timeout_sec: timeout_sec.unwrap_or(600).max(1),
        status_message,
        source_path: source_path.to_path_buf(),
        display_order,
    })
}

fn push_unsupported_hook_warning(
    source_path: &Path,
    warnings: &mut Vec<String>,
    handler_type: &str,
) {
    warnings.push(format!(
        "skipping {handler_type} hook in {}: {handler_type} hooks are not supported yet",
        source_path.display()
    ));
}

fn command_label(command: &str) -> Option<String> {
    let program = shlex::split(command)?
        .into_iter()
        .next()
        .filter(|segment| !segment.is_empty())?;
    PathBuf::from(program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::command_label;
    use super::discover_toml_stop_handlers;
    use crate::types::CommandHookConfig;
    use crate::types::HookRuleConfig;

    #[test]
    fn discover_toml_stop_handlers_uses_command_basename_as_status_message() {
        let discovered = discover_toml_stop_handlers(
            Path::new("/tmp/config.toml"),
            &[HookRuleConfig {
                matcher: None,
                commands: vec![CommandHookConfig {
                    command: "/home/osso/.cargo/bin/claude-plan-hook --fast".to_string(),
                    timeout_sec: None,
                }],
            }],
        );

        assert_eq!(discovered.handlers.len(), 1);
        assert_eq!(
            discovered.handlers[0].status_message.as_deref(),
            Some("claude-plan-hook")
        );
    }

    #[test]
    fn command_label_extracts_program_basename() {
        assert_eq!(
            command_label("/home/osso/.claude/hooks/simplify-nudge.sh"),
            Some("simplify-nudge.sh".to_string())
        );
    }
}
