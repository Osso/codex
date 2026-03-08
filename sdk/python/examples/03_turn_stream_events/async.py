import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()
        started = await client.thread_start(ThreadStartParams(model="gpt-5"))
        thread_id = started["thread"]["id"]

        turn = await client.turn_text(thread_id, "Write a short haiku about compilers.")
        turn_id = turn["turn"]["id"]

        while True:
            event = await client.next_notification()
            print(event.method, event.params)
            if (
                event.method == "turn/completed"
                and (event.params or {}).get("turn", {}).get("id") == turn_id
            ):
                break


if __name__ == "__main__":
    asyncio.run(main())
