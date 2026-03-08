import asyncio
from base64 import b64decode
from pathlib import Path

from codex_app_server.async_client import AsyncAppServerClient

HERE = Path(__file__).parent
IMAGE_PATH = HERE / "sample.png"

if not IMAGE_PATH.exists():
    IMAGE_PATH.write_bytes(
        b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z4xQAAAAASUVORK5CYII="
        )
    )


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()
        started = await client.thread_start(model="gpt-5")
        thread_id = started["thread"]["id"]

        turn = await client.turn_start(
            thread_id,
            [
                {
                    "type": "text",
                    "text": "Read this local image and summarize what you see in 2 bullets.",
                },
                {"type": "localImage", "path": str(IMAGE_PATH.resolve())},
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
