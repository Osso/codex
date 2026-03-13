use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct CommandHookConfig {
    pub command: String,
    #[serde(default)]
    pub timeout_sec: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct HookRuleConfig {
    #[serde(default)]
    pub matcher: Option<String>,
    #[serde(default)]
    pub commands: Vec<CommandHookConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct HooksToml {
    #[serde(default)]
    pub pre_tool_use: Vec<HookRuleConfig>,
    #[serde(default)]
    pub post_tool_use: Vec<HookRuleConfig>,
    #[serde(default)]
    pub user_prompt_submit: Vec<HookRuleConfig>,
    #[serde(default)]
    pub stop: Vec<HookRuleConfig>,
    #[serde(default)]
    pub session_end: Vec<HookRuleConfig>,
    #[serde(default)]
    pub subagent_start: Vec<HookRuleConfig>,
    #[serde(default)]
    pub subagent_stop: Vec<HookRuleConfig>,
}
