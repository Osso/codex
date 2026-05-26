use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::Mutex;

/// This structure is used to add some limits on the multi-agent capabilities for Codex. In
/// the current implementation, it limits:
/// * Total number of sub-agents (i.e. threads) per user session
///
/// This structure is shared by all agents in the same user session (because the `AgentControl`
/// is).
#[derive(Default)]
pub(crate) struct AgentRegistry {
    registered_agents: Mutex<HashMap<String, AgentMetadata>>,
}

fn counted_agent_count(registered_agents: &HashMap<String, AgentMetadata>) -> usize {
    registered_agents
        .values()
        .filter(|metadata| {
            metadata.agent_id.is_some()
                && !metadata.agent_path.as_ref().is_some_and(AgentPath::is_root)
        })
        .count()
}

fn used_agent_nicknames(registered_agents: &HashMap<String, AgentMetadata>) -> HashSet<String> {
    registered_agents
        .values()
        .filter_map(|metadata| metadata.agent_nickname.clone())
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AgentMetadata {
    pub(crate) agent_id: Option<ThreadId>,
    pub(crate) agent_path: Option<AgentPath>,
    pub(crate) agent_nickname: Option<String>,
    pub(crate) agent_role: Option<String>,
    pub(crate) last_task_message: Option<String>,
}

fn format_agent_nickname(name: &str, suffix_index: usize) -> String {
    match suffix_index {
        0 => name.to_string(),
        suffix_index => {
            let value = suffix_index + 1;
            let suffix = match value % 100 {
                11..=13 => "th",
                _ => match value % 10 {
                    1 => "st", // codespell:ignore
                    2 => "nd", // codespell:ignore
                    3 => "rd", // codespell:ignore
                    _ => "th", // codespell:ignore
                },
            };
            format!("{name} the {value}{suffix}")
        }
    }
}

fn session_depth(session_source: &SessionSource) -> i32 {
    match session_source {
        SessionSource::SubAgent(SubAgentSource::ThreadSpawn { depth, .. }) => *depth,
        SessionSource::SubAgent(_) => 0,
        _ => 0,
    }
}

pub(crate) fn next_thread_spawn_depth(session_source: &SessionSource) -> i32 {
    session_depth(session_source).saturating_add(1)
}

pub(crate) fn exceeds_thread_spawn_depth_limit(depth: i32, max_depth: i32) -> bool {
    depth > max_depth
}

impl AgentRegistry {
    pub(crate) fn ensure_spawn_limit(self: &Arc<Self>, max_threads: Option<usize>) -> Result<()> {
        if let Some(max_threads) = max_threads {
            let registered_agents = self
                .registered_agents
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if counted_agent_count(&registered_agents) >= max_threads {
                return Err(CodexErr::AgentLimitReached { max_threads });
            }
        }
        Ok(())
    }

    pub(crate) fn release_spawned_thread(&self, thread_id: ThreadId) {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed_key = registered_agents
            .iter()
            .find_map(|(key, metadata)| (metadata.agent_id == Some(thread_id)).then_some(key))
            .cloned();
        if let Some(key) = removed_key {
            registered_agents.remove(key.as_str());
        }
    }

    pub(crate) fn register_root_thread(&self, thread_id: ThreadId) {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registered_agents
            .entry(AgentPath::ROOT.to_string())
            .or_insert_with(|| AgentMetadata {
                agent_id: Some(thread_id),
                agent_path: Some(AgentPath::root()),
                ..Default::default()
            });
    }

    pub(crate) fn agent_id_for_path(&self, agent_path: &AgentPath) -> Option<ThreadId> {
        self.registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(agent_path.as_str())
            .and_then(|metadata| metadata.agent_id)
    }

    pub(crate) fn agent_metadata_for_thread(&self, thread_id: ThreadId) -> Option<AgentMetadata> {
        self.registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .find(|metadata| metadata.agent_id == Some(thread_id))
            .cloned()
    }

    pub(crate) fn registered_non_root_agents(&self) -> Vec<AgentMetadata> {
        self.registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .filter(|metadata| {
                metadata.agent_id.is_some()
                    && !metadata.agent_path.as_ref().is_some_and(AgentPath::is_root)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn release_threads_missing_from(&self, live_thread_ids: &HashSet<ThreadId>) {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registered_agents.retain(|_, metadata| {
            let Some(thread_id) = metadata.agent_id else {
                return true;
            };
            let is_counted_agent = !metadata.agent_path.as_ref().is_some_and(AgentPath::is_root);
            !is_counted_agent || live_thread_ids.contains(&thread_id)
        });
    }

    pub(crate) fn update_last_task_message(&self, thread_id: ThreadId, last_task_message: String) {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(metadata) = registered_agents
            .values_mut()
            .find(|metadata| metadata.agent_id == Some(thread_id))
        {
            metadata.last_task_message = Some(last_task_message);
        }
    }

    pub(crate) fn register_spawned_thread(&self, agent_metadata: AgentMetadata) {
        let Some(thread_id) = agent_metadata.agent_id else {
            return;
        };
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = agent_metadata
            .agent_path
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("thread:{thread_id}"));
        registered_agents.insert(key, agent_metadata);
    }

    pub(crate) fn reserve_agent_nickname(
        &self,
        names: &[&str],
        preferred: Option<&str>,
    ) -> Option<String> {
        let registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(preferred) = preferred {
            return Some(preferred.to_string());
        }
        if names.is_empty() {
            return None;
        }

        let mut suffix_index = 0;
        loop {
            let used_agent_nicknames = used_agent_nicknames(&registered_agents);
            let available_names: Vec<String> = names
                .iter()
                .map(|name| format_agent_nickname(name, suffix_index))
                .filter(|name| !used_agent_nicknames.contains(name))
                .collect();
            if let Some(name) = available_names.choose(&mut rand::rng()) {
                return Some(name.clone());
            }
            suffix_index += 1;
        }
    }

    pub(crate) fn reserve_agent_path(&self, agent_path: &AgentPath) -> Result<()> {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match registered_agents.entry(agent_path.to_string()) {
            Entry::Occupied(_) => Err(CodexErr::UnsupportedOperation(format!(
                "agent path `{agent_path}` already exists"
            ))),
            Entry::Vacant(entry) => {
                entry.insert(AgentMetadata {
                    agent_path: Some(agent_path.clone()),
                    ..Default::default()
                });
                Ok(())
            }
        }
    }

    pub(crate) fn release_reserved_agent_path(&self, agent_path: &AgentPath) {
        let mut registered_agents = self
            .registered_agents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registered_agents
            .get(agent_path.as_str())
            .is_some_and(|metadata| metadata.agent_id.is_none())
        {
            registered_agents.remove(agent_path.as_str());
        }
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
