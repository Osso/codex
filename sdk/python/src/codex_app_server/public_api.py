from __future__ import annotations

from dataclasses import dataclass
from typing import Iterator

from .client import AppServerClient, AppServerConfig
from .public_types import (
    AskForApproval,
    ForkAskForApproval,
    ForkSandboxMode,
    Personality,
    ResumeAskForApproval,
    ResumePersonality,
    ResumeSandboxMode,
    SandboxMode,
    ThreadForkParams,
    ThreadListParams,
    ThreadSortKey,
    ThreadSourceKind,
    ThreadStartParams,
    ThreadResumeParams,
    TurnStartParams,
    TurnSteerParams,
    TurnStatus,
)
from .generated.v2_types import (
    ModelListResponse,
    ThreadCompactStartResponse,
    ThreadItem,
    ThreadListResponse,
    ThreadReadResponse,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotificationPayload,
    TurnSteerResponse,
)
from .models import JsonObject, Notification


def _event_params_dict(params: object | None) -> JsonObject:
    if params is None:
        return {}
    if isinstance(params, dict):
        return params
    if hasattr(params, "model_dump"):
        return params.model_dump(exclude_none=True)
    return {}


@dataclass(slots=True)
class TurnResult:
    thread_id: str
    turn_id: str
    status: TurnStatus | str
    error: object | None
    text: str
    items: list[ThreadItem]
    usage: ThreadTokenUsageUpdatedNotification | None = None


@dataclass(slots=True)
class TextInput:
    text: str


@dataclass(slots=True)
class ImageInput:
    url: str


@dataclass(slots=True)
class LocalImageInput:
    path: str


@dataclass(slots=True)
class SkillInput:
    name: str
    path: str


@dataclass(slots=True)
class MentionInput:
    name: str
    path: str


InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem


@dataclass(slots=True)
class InitializeResult:
    server_name: str | None = None
    server_version: str | None = None


def _to_wire_item(item: InputItem) -> JsonObject:
    if isinstance(item, TextInput):
        return {"type": "text", "text": item.text}
    if isinstance(item, ImageInput):
        return {"type": "image", "url": item.url}
    if isinstance(item, LocalImageInput):
        return {"type": "localImage", "path": item.path}
    if isinstance(item, SkillInput):
        return {"type": "skill", "name": item.name, "path": item.path}
    if isinstance(item, MentionInput):
        return {"type": "mention", "name": item.name, "path": item.path}
    raise TypeError(f"unsupported input item: {type(item)!r}")


def _to_wire_input(input: Input) -> list[JsonObject]:
    if isinstance(input, list):
        return [_to_wire_item(i) for i in input]
    return [_to_wire_item(input)]


class Codex:
    """Minimal public SDK surface for app-server v2.

    Constructor is eager: it starts and initializes the app-server immediately.
    Errors are raised directly from constructor for Pythonic fail-fast behavior.
    """

    def __init__(self, config: AppServerConfig | None = None) -> None:
        self._client = AppServerClient(config=config)
        self._client.start()
        self._init = self._parse_initialize(self._client.initialize())

    def __enter__(self) -> "Codex":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    @staticmethod
    def _parse_initialize(payload: JsonObject) -> InitializeResult:
        if not isinstance(payload, dict):
            raise TypeError("initialize response must be a dict")
        server = payload.get("serverInfo")
        if isinstance(server, dict):
            return InitializeResult(
                server_name=server.get("name"),
                server_version=server.get("version"),
            )
        # Some app-server builds may omit `serverInfo` in initialize payloads.
        # Keep constructor fail-fast for transport/protocol errors, but allow
        # metadata to be unknown instead of crashing on missing optional fields.
        return InitializeResult()

    @property
    def metadata(self) -> InitializeResult:
        """Startup metadata captured during construction."""
        return self._init

    def close(self) -> None:
        self._client.close()

    def thread_start(self, params: ThreadStartParams) -> Thread:
        started = self._client.thread_start(params)
        return Thread(self._client, started["thread"]["id"])

    def thread(self, thread_id: str) -> Thread:
        return Thread(self._client, thread_id)

    def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        result = self._client.model_list(include_hidden=include_hidden)
        if not isinstance(result, dict):
            raise TypeError("model/list response must be a dict")
        return ModelListResponse.model_validate(result)


@dataclass(slots=True)
class Thread:
    _client: AppServerClient
    id: str

    def turn(
        self,
        input: Input,
        *,
        params: TurnStartParams | None = None,
    ) -> Turn:
        turn = self._client.turn_start(self.id, _to_wire_input(input), params=params)
        return Turn(self._client, self.id, turn["turn"]["id"])

    def resume(self, params: ThreadResumeParams) -> Thread:
        resumed = self._client.thread_resume(self.id, params)
        tid = (resumed.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/resume response missing thread.id")
        return Thread(self._client, tid)

    def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        result = self._client.thread_read(self.id, include_turns=include_turns)
        if not isinstance(result, dict):
            raise TypeError("thread/read response must be a dict")
        return ThreadReadResponse.model_validate(result)

    def fork(self, params: ThreadForkParams) -> Thread:
        payload = params.model_dump(exclude_none=True, mode="json") if hasattr(params, "model_dump") else params
        payload["threadId"] = self.id
        forked = self._client.request("thread/fork", payload)
        tid = (forked.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/fork response missing thread.id")
        return Thread(self._client, tid)

    def archive(self) -> None:
        self._client.thread_archive(self.id)

    def unarchive(self) -> Thread:
        unarchived = self._client.thread_unarchive(self.id)
        tid = (unarchived.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/unarchive response missing thread.id")
        return Thread(self._client, tid)

    def set_name(self, name: str) -> None:
        self._client.thread_set_name(self.id, name)

    def compact(self) -> ThreadCompactStartResponse:
        result = self._client.request("thread/compact", {"threadId": self.id})
        if not isinstance(result, dict):
            raise TypeError("thread/compact response must be a dict")
        return ThreadCompactStartResponse.model_validate(result)

@dataclass(slots=True)
class Turn:
    _client: AppServerClient
    thread_id: str
    id: str

    def steer(self, input: Input) -> TurnSteerResponse:
        params = TurnSteerParams.model_validate(
            {
                "threadId": self.thread_id,
                "expectedTurnId": self.id,
                "input": _to_wire_input(input),
            }
        ).model_dump(exclude_none=True, mode="json")
        result = self._client.request("turn/steer", params)
        if not isinstance(result, dict):
            raise TypeError("turn/steer response must be a dict")
        return TurnSteerResponse.model_validate(result)

    def interrupt(self) -> None:
        self._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> Iterator[Notification]:
        """Yield all notifications for this turn until turn/completed."""
        while True:
            event = self._client.next_notification()
            yield event
            if (
                event.method == "turn/completed"
                and _event_params_dict(event.params).get("turn", {}).get("id") == self.id
            ):
                break

    def run(self) -> TurnResult:
        """Consume turn events and return typed `TurnResult` (completed + usage + text)."""
        completed_payload: JsonObject | None = None
        usage: ThreadTokenUsageUpdatedNotification | None = None
        chunks: list[str] = []

        for event in self.stream():
            if event.method == "item/agentMessage/delta":
                chunks.append(_event_params_dict(event.params).get("delta", ""))
            elif event.method == "thread/tokenUsageUpdated":
                params = _event_params_dict(event.params)
                if params.get("turnId") == self.id:
                    usage = ThreadTokenUsageUpdatedNotification.model_validate(params)
            elif (
                event.method == "turn/completed"
                and _event_params_dict(event.params).get("turn", {}).get("id") == self.id
            ):
                completed_payload = _event_params_dict(event.params)

        if completed_payload is None:
            raise RuntimeError("turn completed event not received")

        completed = TurnCompletedNotificationPayload.model_validate(completed_payload)
        status = completed.turn.status
        return TurnResult(
            thread_id=completed.threadId,
            turn_id=completed.turn.id,
            status=status,
            error=completed.turn.error,
            text="".join(chunks),
            items=list(completed.turn.items or []),
            usage=usage,
        )
