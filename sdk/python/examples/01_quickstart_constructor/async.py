import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        metadata = await client.initialize()
        server = metadata.get("serverInfo", {})
        print("Server:", server.get("name"), server.get("version"))

        started = await client.thread_start(model="gpt-5")
        thread_id = started["thread"]["id"]
        turn = await client.turn_text(thread_id, "Say hello in one sentence.")
        turn_id = turn["turn"]["id"]

        text = await _collect_text_until_completed(client, turn_id)
        print("Status: completed")
        print("Text:", text)


async def _collect_text_until_completed(
    client: AsyncAppServerClient, turn_id: str
) -> str:
    chunks: list[str] = []
    while True:
        event = await client.next_notification()
        if event.method == "item/agentMessage/delta":
            chunks.append((event.params or {}).get("delta", ""))
        if (
            event.method == "turn/completed"
            and (event.params or {}).get("turn", {}).get("id") == turn_id
        ):
            return "".join(chunks).strip()


if __name__ == "__main__":
    asyncio.run(main())
