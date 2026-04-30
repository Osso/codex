use std::time::Instant;

use super::TurnTimingState;

#[tokio::test]
async fn turn_timing_state_tracks_start_and_duration() {
    let state = TurnTimingState::default();
    assert!(state.started_at_unix_secs().await.is_none());
    assert!(state.time_to_first_token_ms().await.is_none());

    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    state.mark_turn_started(Instant::now()).await;
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let started_at = state.started_at_unix_secs().await.unwrap();
    assert!(started_at >= before && started_at <= after);

    let (completed_at, duration_ms) = state.completed_at_and_duration_ms().await;
    assert!(completed_at.is_some());
    assert!(duration_ms.is_some());
    assert!(duration_ms.unwrap() >= 0);
}
