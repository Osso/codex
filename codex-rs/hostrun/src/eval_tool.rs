use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::HostrunExecutionContext;
use crate::HostrunSessionError;
use crate::HostrunSessionStore;

pub const HOSTRUN_EVAL_TOOL_NAME: &str = "hostrun_eval";
pub const DEFAULT_HOSTRUN_SESSION_ID: &str = "default";

#[derive(Debug)]
pub enum HostrunEvalToolError {
    InvalidArguments(String),
    SessionLockPoisoned,
    Eval(HostrunSessionError),
    Encode(serde_json::Error),
}

impl fmt::Display for HostrunEvalToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(message) => write!(formatter, "{message}"),
            Self::SessionLockPoisoned => write!(formatter, "Hostrun session lock was poisoned"),
            Self::Eval(error) => write!(formatter, "{error}"),
            Self::Encode(error) => write!(formatter, "failed to encode Hostrun eval: {error}"),
        }
    }
}

impl std::error::Error for HostrunEvalToolError {}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HostrunEvalArguments {
    pub session_id: Option<String>,
    pub code: String,
}

pub fn parse_eval_arguments(arguments: &str) -> Result<HostrunEvalArguments, HostrunEvalToolError> {
    serde_json::from_str(arguments)
        .map_err(|error| HostrunEvalToolError::InvalidArguments(error.to_string()))
}

pub fn parse_eval_arguments_value(
    arguments: Value,
) -> Result<HostrunEvalArguments, HostrunEvalToolError> {
    serde_json::from_value(arguments)
        .map_err(|error| HostrunEvalToolError::InvalidArguments(error.to_string()))
}

pub fn run_eval_tool(
    sessions: &Arc<Mutex<HostrunSessionStore>>,
    input: &HostrunEvalArguments,
    context: HostrunExecutionContext,
) -> Result<Value, HostrunEvalToolError> {
    let mut sessions = sessions
        .lock()
        .map_err(|_| HostrunEvalToolError::SessionLockPoisoned)?;
    let result = sessions
        .eval_with_context(
            input
                .session_id
                .as_deref()
                .unwrap_or(DEFAULT_HOSTRUN_SESSION_ID),
            &input.code,
            context,
        )
        .map_err(HostrunEvalToolError::Eval)?;

    serde_json::to_value(result).map_err(HostrunEvalToolError::Encode)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::DEFAULT_HOSTRUN_SESSION_ID;
    use super::HostrunEvalArguments;
    use super::parse_eval_arguments;
    use super::parse_eval_arguments_value;

    #[test]
    fn parse_eval_arguments_accepts_code_and_optional_session() {
        let parsed = parse_eval_arguments(r#"{"session_id":"session-1","code":"1 + 1"}"#)
            .expect("arguments parse");

        assert_eq!(
            parsed,
            HostrunEvalArguments {
                session_id: Some("session-1".to_string()),
                code: "1 + 1".to_string(),
            }
        );
    }

    #[test]
    fn parse_eval_arguments_rejects_missing_code() {
        let error = parse_eval_arguments_value(json!({
            "session_id": DEFAULT_HOSTRUN_SESSION_ID,
        }))
        .expect_err("missing code should fail");

        assert!(error.to_string().contains("missing field `code`"));
    }
}
