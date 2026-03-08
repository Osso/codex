# Codex App Server SDK — API Reference

Public surface of `codex_app_server` for app-server v2. See docs/getting-started.md for a walkthrough.

## Package Entry

```python
from codex_app_server import (
    Codex,
    Thread, Turn, TurnResult,
    InitializeResult,
    Input, InputItem,
    TextInput, ImageInput, LocalImageInput, SkillInput, MentionInput,
    ThreadItem,  # event item model
)
```

- Version: `codex_app_server.__version__`
- Requires Python >= 3.10

## Codex

```python
Codex(config: AppServerConfig | None = None)
```

- `metadata -> InitializeResult` — server name/version captured at startup.
- `close() -> None` — closes transport; also on context manager exit.

Thread/session methods:

```python
thread_start(*, approval_policy=None, base_instructions=None, config=None,
             cwd=None, developer_instructions=None, ephemeral=None,
             model=None, model_provider=None, personality=None,
             sandbox=None) -> Thread

thread(thread_id: str) -> Thread

thread_resume(thread_id: str, *, approval_policy=None, base_instructions=None,
              config=None, cwd=None, developer_instructions=None,
              model=None, model_provider=None, sandbox=None) -> Thread

thread_read(thread_id: str) -> ThreadReadResponse
thread_list(*, limit: int | None = None, cursor: str | None = None,
            include_archived: bool | None = None) -> ThreadListResponse
thread_fork(thread_id: str, *, approval_policy=None, base_instructions=None,
            config=None, cwd=None, developer_instructions=None,
            model=None, model_provider=None, sandbox=None) -> Thread
thread_archive(thread_id: str) -> None
thread_unarchive(thread_id: str) -> Thread
thread_set_name(thread_id: str, name: str) -> None
thread_compact(thread_id: str) -> ThreadCompactStartResponse

turn_steer(thread_id: str, expected_turn_id: str, input: Input) -> TurnSteerResponse
turn_interrupt(thread_id: str, turn_id: str) -> None

models(*, include_hidden: bool = False) -> ModelListResponse
```

Context manager:

```python
with Codex() as codex:
    ...
```

## Thread

```python
@dataclass
class Thread:
    id: str

    def turn(self, input: Input) -> Turn: ...
```

## Turn

```python
@dataclass
class Turn:
    thread_id: str
    id: str

    def stream(self) -> Iterable[Notification]: ...
    def run(self) -> TurnResult: ...
```

`run()` returns:

```python
@dataclass
class TurnResult:
    thread_id: str
    turn_id: str
    status: str
    error: Any | None
    text: str
    items: list[ThreadItem]
    usage: ThreadTokenUsageUpdatedNotification | None = None
```

## Inputs

```python
@dataclass class TextInput: text: str
@dataclass class ImageInput: url: str
@dataclass class LocalImageInput: path: str
@dataclass class SkillInput: name: str; path: str
@dataclass class MentionInput: name: str; path: str

InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem
```

Example:

```python
with Codex() as codex:
    thread = codex.thread_start(model="gpt-5")
    result = thread.turn(TextInput("Say hello in one sentence.")).run()
    print(result.text)
```

## Conversation Helpers

Fluent helpers for thread-scoped calls:

```python
from codex_app_server.conversation import ThreadSession, AsyncThreadSession
```

- `ThreadSession(client, thread_id)` — sync helper; `ask`, `stream`, `turn_text`, etc.
- `AsyncThreadSession(client, thread_id)` — async counterpart.

## Retry Helper

```python
from codex_app_server.retry import retry_on_overload
```

- Retries on transient overload (`ServerBusyError`) with exponential backoff + jitter.

## Errors

Common exceptions in `codex_app_server.errors`:

- `AppServerError` (base)
- `JsonRpcError`, `AppServerRpcError`, and specific subclasses like `InvalidParamsError`, `ServerBusyError`, `RetryLimitExceededError`
- `is_retryable_error(exc) -> bool`

