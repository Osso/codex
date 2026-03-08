from codex_app_server import ThreadStartParams, Codex, TextInput

with Codex() as codex:
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(ThreadStartParams(model="gpt-5"))
    result = thread.turn(TextInput("Say hello in one sentence.")).run()
    print("Status:", result.status)
    print("Text:", result.text)
