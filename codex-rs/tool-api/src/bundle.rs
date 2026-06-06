use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;

use crate::FunctionToolSpec;
use crate::ToolCall;
use crate::ToolError;

/// Future returned by one contributed function-tool invocation.
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, ToolError>> + Send + 'a>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCallOutputDelta {
    pub stream: ToolCallOutputStream,
    pub chunk: Vec<u8>,
}

impl ToolCallOutputDelta {
    pub fn stdout(chunk: Vec<u8>) -> Self {
        Self {
            stream: ToolCallOutputStream::Stdout,
            chunk,
        }
    }

    pub fn stderr(chunk: Vec<u8>) -> Self {
        Self {
            stream: ToolCallOutputStream::Stderr,
            chunk,
        }
    }
}

type CancellationProbe = Arc<dyn Fn() -> bool + Send + Sync>;
type OutputSink = Arc<dyn Fn(ToolCallOutputDelta) + Send + Sync>;

#[derive(Clone)]
pub struct ToolExecutionContext {
    cancellation_probe: CancellationProbe,
    output_sink: Option<OutputSink>,
}

impl ToolExecutionContext {
    pub fn new(is_cancelled: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            cancellation_probe: Arc::new(is_cancelled),
            output_sink: None,
        }
    }

    pub fn with_output_sink(
        mut self,
        output_sink: impl Fn(ToolCallOutputDelta) + Send + Sync + 'static,
    ) -> Self {
        self.output_sink = Some(Arc::new(output_sink));
        self
    }

    pub fn emit_output(&self, delta: ToolCallOutputDelta) {
        if let Some(output_sink) = &self.output_sink {
            output_sink(delta);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        (self.cancellation_probe)()
    }
}

impl Default for ToolExecutionContext {
    fn default() -> Self {
        Self::new(|| false)
    }
}

impl std::fmt::Debug for ToolExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolExecutionContext")
            .field("is_cancelled", &self.is_cancelled())
            .field("has_output_sink", &self.output_sink.is_some())
            .finish()
    }
}

/// Model-visible definition plus executable implementation for one contributed
/// function tool.
#[derive(Clone)]
pub struct ToolBundle {
    spec: FunctionToolSpec,
    executor: Arc<dyn ToolExecutor>,
}

impl ToolBundle {
    /// Creates one contributed function-tool bundle.
    pub fn new(spec: FunctionToolSpec, executor: Arc<dyn ToolExecutor>) -> Self {
        Self { spec, executor }
    }

    /// Returns the contributed function-tool spec.
    pub fn spec(&self) -> &FunctionToolSpec {
        &self.spec
    }

    /// Returns the contributed function-tool name.
    pub fn tool_name(&self) -> &str {
        self.spec.name.as_str()
    }

    /// Returns the executable implementation.
    pub fn executor(&self) -> Arc<dyn ToolExecutor> {
        Arc::clone(&self.executor)
    }
}

/// Executable behavior for one contributed function tool.
///
/// Implementations receive the model-supplied call id and JSON arguments and
/// return the JSON value that should be exposed to the model.
pub trait ToolExecutor: Send + Sync {
    fn execute<'a>(&'a self, call: ToolCall) -> ToolFuture<'a>;

    fn execute_with_context<'a>(
        &'a self,
        call: ToolCall,
        _context: ToolExecutionContext,
    ) -> ToolFuture<'a> {
        self.execute(call)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::ToolBundle;
    use super::ToolCallOutputDelta;
    use super::ToolExecutionContext;
    use super::ToolExecutor;
    use super::ToolFuture;
    use crate::FunctionToolSpec;
    use crate::ToolCall;

    struct StubExecutor;

    impl ToolExecutor for StubExecutor {
        fn execute<'a>(&'a self, _call: ToolCall) -> ToolFuture<'a> {
            Box::pin(async { Ok(json!({ "ok": true })) })
        }
    }

    #[test]
    fn bundle_derives_name_from_function_spec() {
        let bundle = ToolBundle::new(
            FunctionToolSpec {
                name: "echo".to_string(),
                description: "Echo arguments.".to_string(),
                strict: false,
                parameters: json!({ "type": "object" }),
            },
            Arc::new(StubExecutor),
        );

        assert_eq!(bundle.tool_name(), "echo");
    }

    #[test]
    fn execution_context_emits_output_and_reports_cancellation() {
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let emitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let context = ToolExecutionContext::new({
            let cancelled = std::sync::Arc::clone(&cancelled);
            move || cancelled.load(std::sync::atomic::Ordering::SeqCst)
        })
        .with_output_sink({
            let emitted = std::sync::Arc::clone(&emitted);
            move |delta| emitted.lock().expect("emitted output lock").push(delta)
        });

        context.emit_output(ToolCallOutputDelta::stdout(b"hello".to_vec()));
        assert_eq!(
            *emitted.lock().expect("emitted output lock"),
            vec![ToolCallOutputDelta::stdout(b"hello".to_vec())]
        );

        assert!(!context.is_cancelled());
        cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(context.is_cancelled());
    }
}
