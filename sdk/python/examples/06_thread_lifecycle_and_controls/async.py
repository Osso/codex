import asyncio
from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()
        started = await client.thread_start(ThreadStartParams(model="gpt-5"))
        thread_id = started["thread"]["id"]

        # Stable lifecycle calls without requiring model execution.
        await client.thread_list(limit=20)
        _ = await client.thread_read(thread_id, include_turns=False)

        print("Lifecycle OK:", thread_id)


if __name__ == "__main__":
    asyncio.run(main())
