import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()

        created = await client.thread_start(model="gpt-5")
        thread_id = created["thread"]["id"]

        first = await client.turn_text(thread_id, "Tell me one fact about Saturn.")
        await _wait_completed(client, first["turn"]["id"])
        print("Created thread:", thread_id)

        resumed = await client.thread_resume(thread_id)
        resumed_id = resumed["thread"]["id"]
        second = await client.turn_text(resumed_id, "Continue with one more fact.")
        text = await _collect_text_until_completed(client, second["turn"]["id"])
        print(text)


async def _wait_completed(client: AsyncAppServerClient, turn_id: str) -> None:
    while True:
        event = await client.next_notification()
        if (
            event.method == "turn/completed"
            and (event.params or {}).get("turn", {}).get("id") == turn_id
        ):
            return


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
