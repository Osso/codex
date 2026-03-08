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

    def thread_start(
        self,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
    ) -> Thread:
        params = ThreadStartParams.model_validate(
            {
                "approvalPolicy": approval_policy,
                "baseInstructions": base_instructions,
                "config": config,
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "ephemeral": ephemeral,
                "model": model,
                "modelProvider": model_provider,
                "personality": personality,
                "sandbox": sandbox,
            }
        ).model_dump(exclude_none=True, mode="json")
        started = self._client.thread_start(params)
        return Thread(self._client, started["thread"]["id"])

    def thread(self, thread_id: str) -> Thread:
        return Thread(self._client, thread_id)

    def thread_resume(
        self,
        thread_id: str,
        *,
        approval_policy: ResumeAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: ResumePersonality | None = None,
        sandbox: ResumeSandboxMode | None = None,
    ) -> Thread:
        params = ThreadResumeParams.model_validate(
            {
                "threadId": thread_id,
                "approvalPolicy": approval_policy,
                "baseInstructions": base_instructions,
                "config": config,
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "model": model,
                "modelProvider": model_provider,
                "personality": personality,
                "sandbox": sandbox,
            }
        ).model_dump(exclude_none=True, mode="json")
        resumed = self._client.request("thread/resume", params)
        tid = (resumed.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/resume response missing thread.id")
        return Thread(self._client, tid)

    def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: str | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
    ) -> ThreadListResponse:
        params = ThreadListParams.model_validate(
            {
                "archived": archived,
                "cursor": cursor,
                "cwd": cwd,
                "limit": limit,
                "modelProviders": model_providers,
                "sortKey": sort_key,
                "sourceKinds": source_kinds,
            }
        ).model_dump(exclude_none=True, mode="json")
        result = self._client.thread_list(params)
        if not isinstance(result, dict):
            raise TypeError("thread/list response must be a dict")
        return ThreadListResponse.model_validate(result)

    def thread_read(
        self, thread_id: str, *, include_turns: bool = False
    ) -> ThreadReadResponse:
        result = self._client.thread_read(thread_id, include_turns=include_turns)
        if not isinstance(result, dict):
            raise TypeError("thread/read response must be a dict")
        return ThreadReadResponse.model_validate(result)

    def thread_fork(
        self,
        thread_id: str,
        *,
        approval_policy: ForkAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: ForkSandboxMode | None = None,
    ) -> Thread:
        params = ThreadForkParams.model_validate(
            {
                "threadId": thread_id,
                "approvalPolicy": approval_policy,
                "baseInstructions": base_instructions,
                "config": config,
                "cwd": cwd,
                "developerInstructions": developer_instructions,
                "model": model,
                "modelProvider": model_provider,
                "sandbox": sandbox,
            }
        ).model_dump(exclude_none=True, mode="json")
        forked = self._client.request("thread/fork", params)
        tid = (forked.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/fork response missing thread.id")
        return Thread(self._client, tid)

    def thread_archive(self, thread_id: str) -> None:
        self._client.thread_archive(thread_id)

    def thread_unarchive(self, thread_id: str) -> Thread:
        unarchived = self._client.thread_unarchive(thread_id)
        tid = (unarchived.get("thread") or {}).get("id")
        if not isinstance(tid, str) or not tid:
            raise ValueError("thread/unarchive response missing thread.id")
        return Thread(self._client, tid)

    def thread_set_name(self, thread_id: str, name: str) -> None:
        self._client.thread_set_name(thread_id, name)

    def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        result = self._client.request("thread/compact", {"threadId": thread_id})
        if not isinstance(result, dict):
            raise TypeError("thread/compact response must be a dict")
        return ThreadCompactStartResponse.model_validate(result)

    def turn_steer(
        self, thread_id: str, expected_turn_id: str, input: Input
    ) -> TurnSteerResponse:
        params = TurnSteerParams.model_validate(
            {
                "threadId": thread_id,
                "expectedTurnId": expected_turn_id,
                "input": _to_wire_input(input),
            }
        ).model_dump(exclude_none=True, mode="json")
        result = self._client.request("turn/steer", params)
        if not isinstance(result, dict):
            raise TypeError("turn/steer response must be a dict")
        return TurnSteerResponse.model_validate(result)

    def turn_interrupt(self, thread_id: str, turn_id: str) -> None:
        self._client.turn_interrupt(thread_id, turn_id)

    def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        result = self._client.model_list(include_hidden=include_hidden)
        if not isinstance(result, dict):
            raise TypeError("model/list response must be a dict")
        return ModelListResponse.model_validate(result)


@dataclass(slots=True)
class Thread:
    _client: AppServerClient
    id: str

    def turn(self, input: Input) -> Turn:
        turn = self._client.turn_start(self.id, _to_wire_input(input))
        return Turn(self._client, self.id, turn["turn"]["id"])


@dataclass(slots=True)
class Turn:
    _client: AppServerClient
    thread_id: str
    id: str

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
