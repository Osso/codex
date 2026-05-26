use super::*;
use codex_protocol::AgentPath;
use pretty_assertions::assert_eq;
use std::collections::HashSet;

fn agent_path(path: &str) -> AgentPath {
    AgentPath::try_from(path).expect("valid agent path")
}

fn agent_metadata(thread_id: ThreadId) -> AgentMetadata {
    AgentMetadata {
        agent_id: Some(thread_id),
        ..Default::default()
    }
}

fn agent_metadata_with_nickname(thread_id: ThreadId, agent_nickname: &str) -> AgentMetadata {
    AgentMetadata {
        agent_id: Some(thread_id),
        agent_nickname: Some(agent_nickname.to_string()),
        ..Default::default()
    }
}

#[test]
fn format_agent_nickname_adds_ordinal_suffixes() {
    assert_eq!(format_agent_nickname("Plato", /*suffix_index*/ 0), "Plato");
    assert_eq!(
        format_agent_nickname("Plato", /*suffix_index*/ 1),
        "Plato the 2nd"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*suffix_index*/ 2),
        "Plato the 3rd"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*suffix_index*/ 10),
        "Plato the 11th"
    );
    assert_eq!(
        format_agent_nickname("Plato", /*suffix_index*/ 20),
        "Plato the 21st"
    );
}

#[test]
fn session_depth_defaults_to_zero_for_root_sources() {
    assert_eq!(session_depth(&SessionSource::Cli), 0);
}

#[test]
fn thread_spawn_depth_increments_and_enforces_limit() {
    let session_source = SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id: ThreadId::new(),
        depth: 1,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    });
    let child_depth = next_thread_spawn_depth(&session_source);
    assert_eq!(child_depth, 2);
    assert!(exceeds_thread_spawn_depth_limit(
        child_depth,
        /*max_depth*/ 1
    ));
}

#[test]
fn non_thread_spawn_subagents_default_to_depth_zero() {
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);
    assert_eq!(session_depth(&session_source), 0);
    assert_eq!(next_thread_spawn_depth(&session_source), 1);
    assert!(!exceeds_thread_spawn_depth_limit(
        /*depth*/ 1, /*max_depth*/ 1
    ));
}

#[test]
fn commit_holds_slot_until_release() {
    let registry = Arc::new(AgentRegistry::default());
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot available");
    let thread_id = ThreadId::new();
    registry.register_spawned_thread(agent_metadata(thread_id));

    let err = match registry.ensure_spawn_limit(Some(1)) {
        Ok(_) => panic!("limit should be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(thread_id);
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot released after thread removal");
}

#[test]
fn release_ignores_unknown_thread_id() {
    let registry = Arc::new(AgentRegistry::default());
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot available");
    let thread_id = ThreadId::new();
    registry.register_spawned_thread(agent_metadata(thread_id));

    registry.release_spawned_thread(ThreadId::new());

    let err = match registry.ensure_spawn_limit(Some(1)) {
        Ok(_) => panic!("limit should still be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(thread_id);
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot released after real thread removal");
}

#[test]
fn release_is_idempotent_for_registered_threads() {
    let registry = Arc::new(AgentRegistry::default());
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot available");
    let first_id = ThreadId::new();
    registry.register_spawned_thread(agent_metadata(first_id));

    registry.release_spawned_thread(first_id);

    registry.ensure_spawn_limit(Some(1)).expect("slot reused");
    let second_id = ThreadId::new();
    registry.register_spawned_thread(agent_metadata(second_id));

    registry.release_spawned_thread(first_id);

    let err = match registry.ensure_spawn_limit(Some(1)) {
        Ok(_) => panic!("limit should still be enforced"),
        Err(err) => err,
    };
    let CodexErr::AgentLimitReached { max_threads } = err else {
        panic!("expected CodexErr::AgentLimitReached");
    };
    assert_eq!(max_threads, 1);

    registry.release_spawned_thread(second_id);
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot released after second thread removal");
}

#[test]
fn release_threads_missing_from_drops_stale_counted_agents() {
    let registry = Arc::new(AgentRegistry::default());
    let stale_thread_id = ThreadId::new();
    registry
        .ensure_spawn_limit(Some(1))
        .expect("slot available");
    registry.register_spawned_thread(agent_metadata(stale_thread_id));

    registry.release_threads_missing_from(&HashSet::new());

    registry
        .ensure_spawn_limit(Some(1))
        .expect("stale counted agent should be released");
}

#[test]
fn unregistered_nickname_does_not_mark_name_used() {
    let registry = Arc::new(AgentRegistry::default());
    let agent_nickname = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("reserve agent name");
    assert_eq!(agent_nickname, "alpha");

    let agent_nickname = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("unregistered name should remain available");
    assert_eq!(agent_nickname, "alpha");
}

#[test]
fn active_duplicate_nickname_gets_next_suffix() {
    let registry = Arc::new(AgentRegistry::default());
    let first_name = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    assert_eq!(first_name, "alpha");
    registry.register_spawned_thread(agent_metadata_with_nickname(first_id, &first_name));

    let second_name = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("active duplicate should get a suffix");
    assert_eq!(second_name, "alpha the 2nd");
}

#[test]
fn released_nickname_is_available_again() {
    let registry = Arc::new(AgentRegistry::default());

    let first_name = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    assert_eq!(first_name, "alpha");
    registry.register_spawned_thread(agent_metadata_with_nickname(first_id, &first_name));

    registry.release_spawned_thread(first_id);

    let second_name = registry
        .reserve_agent_nickname(&["alpha"], /*preferred*/ None)
        .expect("released name should be available");
    assert_eq!(second_name, "alpha");
}

#[test]
fn released_suffix_nickname_is_available_again_while_base_is_active() {
    let registry = Arc::new(AgentRegistry::default());

    let first_name = registry
        .reserve_agent_nickname(&["Plato"], /*preferred*/ None)
        .expect("reserve first agent name");
    let first_id = ThreadId::new();
    assert_eq!(first_name, "Plato");
    registry.register_spawned_thread(agent_metadata_with_nickname(first_id, &first_name));

    let second_name = registry
        .reserve_agent_nickname(&["Plato"], /*preferred*/ None)
        .expect("reserve second agent name");
    let second_id = ThreadId::new();
    assert_eq!(second_name, "Plato the 2nd");
    registry.register_spawned_thread(agent_metadata_with_nickname(second_id, &second_name));
    registry.release_spawned_thread(second_id);

    let third_name = registry
        .reserve_agent_nickname(&["Plato"], /*preferred*/ None)
        .expect("reserve third agent name");
    assert_eq!(third_name, "Plato the 2nd");
}

#[test]
fn register_root_thread_indexes_root_path() {
    let registry = Arc::new(AgentRegistry::default());
    let root_thread_id = ThreadId::new();

    registry.register_root_thread(root_thread_id);

    assert_eq!(
        registry.agent_id_for_path(&AgentPath::root()),
        Some(root_thread_id)
    );
}

#[test]
fn reserved_agent_path_is_released_when_spawn_fails() {
    let registry = Arc::new(AgentRegistry::default());
    let path = agent_path("/root/researcher");
    registry
        .reserve_agent_path(&path)
        .expect("reserve first path");
    registry.release_reserved_agent_path(&path);

    registry
        .reserve_agent_path(&path)
        .expect("released reservation should free the path");
}

#[test]
fn committed_agent_path_is_indexed_until_release() {
    let registry = Arc::new(AgentRegistry::default());
    let thread_id = ThreadId::new();
    let path = agent_path("/root/researcher");
    registry.reserve_agent_path(&path).expect("reserve path");
    registry.register_spawned_thread(AgentMetadata {
        agent_id: Some(thread_id),
        agent_path: Some(path.clone()),
        ..Default::default()
    });

    assert_eq!(registry.agent_id_for_path(&path), Some(thread_id));

    registry.release_spawned_thread(thread_id);
    assert_eq!(registry.agent_id_for_path(&path), None);
}
