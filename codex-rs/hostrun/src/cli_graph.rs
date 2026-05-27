use serde_json::Value;
use serde_json::json;

use crate::cli_execution::CliCommandStatus;
use crate::cli_execution::CliProcessOutput;

pub(crate) fn insert_command_graph(
    result: &mut serde_json::Map<String, Value>,
    output: &CliProcessOutput,
) {
    if output.upstream.is_empty() {
        return;
    }
    let mut commands = output
        .upstream
        .iter()
        .map(command_status_value)
        .collect::<Vec<_>>();
    commands.push(command_status_value(&output.command));
    result.insert("commands".to_string(), Value::Array(commands));
}

fn command_status_value(command: &CliCommandStatus) -> Value {
    json!({
        "program": command.program,
        "args": command.args,
        "exitCode": command.exit_code,
        "success": command.success
    })
}
