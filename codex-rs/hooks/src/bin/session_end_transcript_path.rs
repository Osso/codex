use std::io::Read;

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    if let Some(path) = codex_hooks::session_end_transcript_path_from_json(&input) {
        println!("{path}");
    }
}
