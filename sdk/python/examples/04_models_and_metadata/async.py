import asyncio

from codex_app_server.async_client import AsyncAppServerClient


async def main() -> None:
    async with AsyncAppServerClient() as client:
        metadata = await client.initialize()
        print("metadata:", metadata)

        models = await client.model_list(include_hidden=True)
        data = models.get("data", [])
        print("models.count:", len(data))
        if data:
            print("first model id:", data[0].get("id"))


if __name__ == "__main__":
    asyncio.run(main())
