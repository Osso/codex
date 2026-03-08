from codex_app_server import Codex

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")

    # Stable lifecycle calls without requiring model execution.
    _ = codex.thread_list(limit=20)
    _ = codex.thread_read(thread.id, include_turns=False)

    print("Lifecycle OK:", thread.id)
