use std::collections::HashMap;
use std::process::Child;

use serde_json::Map;
use serde_json::Value;
use serde_json::json;

#[derive(Default)]
pub(crate) struct ProcessRegistry {
    next_id: u64,
    processes: HashMap<String, ManagedProcess>,
}

impl ProcessRegistry {
    pub(crate) fn insert(
        &mut self,
        program: &str,
        argv: &[String],
        child: Child,
        payload: Map<String, Value>,
    ) -> String {
        self.next_id += 1;
        let id = format!("process-{}", self.next_id);
        self.processes.insert(
            id.clone(),
            ManagedProcess {
                program: program.to_string(),
                argv: argv.to_vec(),
                child,
                payload,
            },
        );
        id
    }

    pub(crate) fn take(&mut self, id: &str) -> Option<ManagedProcess> {
        self.processes.remove(id)
    }

    pub(crate) fn kill_all(&mut self) {
        for process in self.processes.values_mut() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
        self.processes.clear();
    }
}

pub(crate) struct ManagedProcess {
    pub(crate) program: String,
    pub(crate) argv: Vec<String>,
    pub(crate) child: Child,
    pub(crate) payload: Map<String, Value>,
}

pub(crate) fn started_value(id: &str, pid: u32, program: &str, argv: &[String]) -> Value {
    json!({
        "id": id,
        "pid": pid,
        "program": program,
        "args": argv,
        "stdout": {
            "process": id,
            "stream": "stdout"
        },
        "stderr": {
            "process": id,
            "stream": "stderr"
        }
    })
}
