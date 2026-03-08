import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()
        started = await client.thread_start(ThreadStartParams(model="gpt-5"))
        thread_id = started["thread"]["id"]

        turn = await client.turn_start(
            thread_id,
            [
                {"type": "text", "text": "What is in this image? Give 3 bullets."},
                {
                    "type": "image",
                    "url": "https://upload.wikimedia.org/wikipedia/commons/3/3a/Cat03.jpg",
                },
            ],
        )
        turn_id = turn["turn"]["id"]
        status, text = await _collect_until_completed(client, turn_id)

        print("Status:", status)
        print(text)


async def _collect_until_completed(
    client: AsyncAppServerClient, turn_id: str
) -> tuple[str, str]:
    chunks: list[str] = []
    status = "unknown"
    while True:
        event = await client.next_notification()
        if event.method == "item/agentMessage/delta":
            chunks.append((event.params or {}).get("delta", ""))
        if (
            event.method == "turn/completed"
            and (event.params or {}).get("turn", {}).get("id") == turn_id
        ):
            status = (event.params or {}).get("turn", {}).get("status", "unknown")
            return status, "".join(chunks).strip()


if __name__ == "__main__":
    asyncio.run(main())
