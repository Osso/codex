use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use tokio::sync::Mutex;

#[derive(Debug, Default)]
pub(crate) struct TurnTimingState {
    state: Mutex<TurnTimingStateInner>,
}

#[derive(Debug, Default)]
struct TurnTimingStateInner {
    started_at: Option<Instant>,
    started_at_unix_secs: Option<i64>,
}

impl TurnTimingState {
    pub(crate) async fn mark_turn_started(&self, started_at: Instant) {
        let mut state = self.state.lock().await;
        state.started_at = Some(started_at);
        state.started_at_unix_secs = Some(now_unix_timestamp_secs());
    }

    pub(crate) async fn started_at_unix_secs(&self) -> Option<i64> {
        self.state.lock().await.started_at_unix_secs
    }

    pub(crate) async fn completed_at_and_duration_ms(&self) -> (Option<i64>, Option<i64>) {
        let state = self.state.lock().await;
        let completed_at = Some(now_unix_timestamp_secs());
        let duration_ms = state
            .started_at
            .map(|started_at| i64::try_from(started_at.elapsed().as_millis()).unwrap_or(i64::MAX));
        (completed_at, duration_ms)
    }

    pub(crate) async fn time_to_first_token_ms(&self) -> Option<i64> {
        None
    }
}

fn now_unix_timestamp_secs() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
}

pub(crate) fn now_unix_timestamp_ms() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

impl TurnTimingStateInner {}

#[cfg(test)]
#[path = "turn_timing_tests.rs"]
mod tests;
