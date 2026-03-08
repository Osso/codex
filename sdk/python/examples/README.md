# Python SDK Examples

Each example folder contains both runnable versions:

- `sync.py` (synchronous client surface)
- `async.py` (async client surface)

## Run format

From `sdk/python`:

```bash
python examples/<example-folder>/sync.py
python examples/<example-folder>/async.py
```

## Recommended first run

```bash
python examples/01_quickstart_constructor/sync.py
python examples/01_quickstart_constructor/async.py
```

## Index

- `01_quickstart_constructor/`
  - first run / sanity check
- `02_turn_run/`
  - inspect full turn output fields
- `03_turn_stream_events/`
  - stream and print raw notifications
- `04_models_and_metadata/`
  - read server metadata and model list
- `05_existing_thread/`
  - resume a real existing thread (created in-script)
- `06_thread_lifecycle_and_controls/`
  - thread lifecycle + control calls
- `07_image_and_text/`
  - remote image URL + text multimodal turn
- `08_local_image_and_text/`
  - local image + text multimodal turn (auto-downloads sample image)
- `09_async_parity/`
  - same flow in sync and async styles
- `10_error_handling_and_retry/`
  - typed JSON-RPC errors + overload retry pattern
- `11_cli_mini_app/`
  - interactive chat loop
