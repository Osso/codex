import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()
        started = await client.thread_start(model="gpt-5")
        thread_id = started["thread"]["id"]

        turn = await client.turn_text(thread_id, "Give 3 bullets about SIMD.")
        turn_id = turn["turn"]["id"]
        completed = await _wait_completed(client, turn_id)

        print("thread_id:", thread_id)
        print("turn_id:", turn_id)
        print("status:", completed.get("turn", {}).get("status"))
        print("error:", completed.get("turn", {}).get("error"))
        print("text:", _extract_text(completed))
        print("items:", completed.get("items", []))
        print("usage:", completed.get("turn", {}).get("usage"))


def _extract_text(completed: dict) -> str:
    parts: list[str] = []
    for item in completed.get("items", []):
        if item.get("type") != "agent_message":
            continue
        for c in item.get("content", []):
            if c.get("type") == "output_text":
                parts.append(c.get("text", ""))
    return "\n".join(p for p in parts if p).strip()


async def _wait_completed(client: AsyncAppServerClient, turn_id: str) -> dict:
    while True:
        event = await client.next_notification()
        if (
            event.method == "turn/completed"
            and (event.params or {}).get("turn", {}).get("id") == turn_id
        ):
            return event.params or {}


if __name__ == "__main__":
    asyncio.run(main())
