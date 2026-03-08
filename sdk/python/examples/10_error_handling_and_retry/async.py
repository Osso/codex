import asyncio
import random

from codex_app_server.async_client import AsyncAppServerClient
from codex_app_server.errors import (
    InvalidParamsError,
    JsonRpcError,
    MethodNotFoundError,
    ServerBusyError,
    is_retryable_error,
)


async def retry_on_overload_async(
    op,
    *,
    max_attempts: int = 3,
    initial_delay_s: float = 0.25,
    max_delay_s: float = 2.0,
    jitter_ratio: float = 0.2,
):
    if max_attempts < 1:
        raise ValueError("max_attempts must be >= 1")

    delay = initial_delay_s
    attempt = 0
    while True:
        attempt += 1
        try:
            return await op()
        except Exception as exc:  # noqa: BLE001
            if attempt >= max_attempts or not is_retryable_error(exc):
                raise
            jitter = delay * jitter_ratio
            sleep_for = min(max_delay_s, delay) + random.uniform(-jitter, jitter)
            if sleep_for > 0:
                await asyncio.sleep(sleep_for)
            delay = min(max_delay_s, delay * 2)


async def main() -> None:
    async with AsyncAppServerClient() as client:
        await client.initialize()

        started = await client.thread_start(model="gpt-5")
        thread_id = started["thread"]["id"]

        turn = await retry_on_overload_async(
            lambda: client.turn_text(
                thread_id, "Summarize retry best practices in 3 bullets."
            ),
            max_attempts=3,
            initial_delay_s=0.25,
            max_delay_s=2.0,
        )
        turn_id = turn["turn"]["id"]
        text = await _collect_text_until_completed(client, turn_id)
        print("Text:", text)

        try:
            # Async client has no direct `request`; use sync transport via helper.
            await client._call_sync(
                client._sync.request, "demo/missingMethod", {}
            )  # noqa: SLF001
        except MethodNotFoundError as exc:
            print("Method not found:", exc.message)
        except InvalidParamsError as exc:
            print("Invalid params:", exc.message)
        except ServerBusyError as exc:
            print("Server overloaded after retries:", exc.message)
        except JsonRpcError as exc:
            print(f"JSON-RPC error {exc.code}: {exc.message}")


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
