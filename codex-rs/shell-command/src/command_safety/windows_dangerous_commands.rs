use super::executable_name_lookup_key;

pub(crate) fn is_dangerous_command_windows(command: &[String]) -> bool {
    let Some(cmd0) = command.first().map(String::as_str) else {
        return false;
    };

    match executable_name_lookup_key(cmd0).as_deref() {
        Some("cmd") => command
            .windows(2)
            .any(|args| matches!(args[0].as_str(), "/c" | "/C") && script_is_dangerous(&args[1])),
        Some("powershell") | Some("pwsh") => command.iter().skip(1).any(|arg| {
            let lower = arg.to_ascii_lowercase();
            dangerous_shell_word(&lower) || script_is_dangerous(&lower)
        }),
        Some(cmd) => dangerous_shell_word(cmd),
        None => false,
    }
}

pub(crate) fn is_dangerous_powershell_words(command: &[String]) -> bool {
    command.iter().any(|arg| {
        let lower = arg.to_ascii_lowercase();
        dangerous_shell_word(&lower) || script_is_dangerous(&lower)
    })
}

fn script_is_dangerous(script: &str) -> bool {
    script
        .split(|ch: char| !ch.is_alphanumeric() && ch != '-')
        .map(|word| word.to_ascii_lowercase())
        .any(|word| dangerous_shell_word(&word))
}

fn dangerous_shell_word(word: &str) -> bool {
    matches!(
        word,
        "del"
            | "erase"
            | "move"
            | "rd"
            | "ren"
            | "rename"
            | "rmdir"
            | "rm"
            | "add-content"
            | "clear-content"
            | "copy-item"
            | "move-item"
            | "new-item"
            | "out-file"
            | "remove-item"
            | "rename-item"
            | "set-content"
            | "stop-process"
    )
}
