from codex_app_server.client import AppServerClient

with AppServerClient() as client:
    metadata = client.initialize()
    server = metadata.get("serverInfo", {})
    print("Server:", server.get("name"), server.get("version"))

    started = client.thread_start(ThreadStartParams(model="gpt-5"))
    thread_id = started["thread"]["id"]

    turn = client.turn_text(thread_id, "Say hello in one sentence.")
    turn_id = turn["turn"]["id"]

    chunks: list[str] = []
    while True:
        event = client.next_notification()
        if event.method == "item/agentMessage/delta":
            chunks.append((event.params or {}).get("delta", ""))
        if (
            event.method == "turn/completed"
            and (event.params or {}).get("turn", {}).get("id") == turn_id
        ):
            break

    print("Thread:", thread_id)
    print("Turn:", turn_id)
    print("Text:", "".join(chunks).strip())
