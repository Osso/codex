from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")
    turn = thread.turn(TextInput("Write a short haiku about compilers."))

    for event in turn.stream():
        print(event.method, event.params)
