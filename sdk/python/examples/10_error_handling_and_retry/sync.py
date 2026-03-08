from codex_app_server.client import AppServerClient
from codex_app_server.errors import (
    InvalidParamsError,
    JsonRpcError,
    MethodNotFoundError,
    ServerBusyError,
)
from codex_app_server.retry import retry_on_overload

with AppServerClient() as client:
    client.initialize()

    started = client.thread_start(model="gpt-5")
    thread_id = started["thread"]["id"]

    # Example 1: retry a turn on transient server overload.
    result = retry_on_overload(
        lambda: client.run_text_turn(
            thread_id, "Summarize retry best practices in 3 bullets."
        ),
        max_attempts=3,
        initial_delay_s=0.25,
        max_delay_s=2.0,
    )
    print("Text:", result[0])

    # Example 2: targeted exception handling for common RPC failures.
    try:
        # Deliberately call a missing method to demonstrate typed error handling.
        client.request("demo/missingMethod", {})
    except MethodNotFoundError as exc:
        print("Method not found:", exc.message)
    except InvalidParamsError as exc:
        print("Invalid params:", exc.message)
    except ServerBusyError as exc:
        print("Server overloaded after retries:", exc.message)
    except JsonRpcError as exc:
        print(f"JSON-RPC error {exc.code}: {exc.message}")
