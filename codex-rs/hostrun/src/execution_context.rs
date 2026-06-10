use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostrunOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostrunOutputDelta {
    pub stream: HostrunOutputStream,
    pub chunk: Vec<u8>,
}

impl HostrunOutputDelta {
    pub fn stdout(chunk: Vec<u8>) -> Self {
        Self {
            stream: HostrunOutputStream::Stdout,
            chunk,
        }
    }

    pub fn stderr(chunk: Vec<u8>) -> Self {
        Self {
            stream: HostrunOutputStream::Stderr,
            chunk,
        }
    }
}

type CancellationProbe = Arc<dyn Fn() -> bool + Send + Sync>;
type OutputSink = Arc<dyn Fn(HostrunOutputDelta) + Send + Sync>;

#[derive(Clone)]
pub struct HostrunExecutionContext {
    cancellation_probe: CancellationProbe,
    output_sink: Option<OutputSink>,
}

impl HostrunExecutionContext {
    pub fn new(is_cancelled: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            cancellation_probe: Arc::new(is_cancelled),
            output_sink: None,
        }
    }

    pub fn with_output_sink(
        mut self,
        output_sink: impl Fn(HostrunOutputDelta) + Send + Sync + 'static,
    ) -> Self {
        self.output_sink = Some(Arc::new(output_sink));
        self
    }

    pub fn emit_output(&self, delta: HostrunOutputDelta) {
        if let Some(output_sink) = &self.output_sink {
            output_sink(delta);
        }
    }

    pub fn is_cancelled(&self) -> bool {
        (self.cancellation_probe)()
    }
}

impl Default for HostrunExecutionContext {
    fn default() -> Self {
        Self::new(|| false)
    }
}

impl std::fmt::Debug for HostrunExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostrunExecutionContext")
            .field("is_cancelled", &self.is_cancelled())
            .field("has_output_sink", &self.output_sink.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use super::HostrunExecutionContext;
    use super::HostrunOutputDelta;

    #[test]
    fn default_execution_context_is_not_cancelled_and_drops_output() {
        let context = HostrunExecutionContext::default();

        context.emit_output(HostrunOutputDelta::stdout(b"hello".to_vec()));

        assert!(!context.is_cancelled());
    }

    #[test]
    fn execution_context_emits_output_and_reports_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let context = HostrunExecutionContext::new({
            let cancelled = Arc::clone(&cancelled);
            move || cancelled.load(Ordering::SeqCst)
        })
        .with_output_sink({
            let emitted = Arc::clone(&emitted);
            move |delta| emitted.lock().expect("emitted output lock").push(delta)
        });

        context.emit_output(HostrunOutputDelta::stdout(b"hello".to_vec()));
        context.emit_output(HostrunOutputDelta::stderr(b"error".to_vec()));

        assert_eq!(
            *emitted.lock().expect("emitted output lock"),
            vec![
                HostrunOutputDelta::stdout(b"hello".to_vec()),
                HostrunOutputDelta::stderr(b"error".to_vec())
            ]
        );
        assert!(!context.is_cancelled());

        cancelled.store(true, Ordering::SeqCst);

        assert!(context.is_cancelled());
    }
}
