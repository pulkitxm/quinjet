"""Claude calls shared by every agent method."""

from __future__ import annotations

import os
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

from anthropic import AsyncAnthropic

DEFAULT_MODEL = "claude-opus-5-5"
FALLBACK_BETA = "server-side-fallback-2026-07-01"
MAX_TOKENS = 16000
MAX_TOOL_TURNS = 16
TOOL_RESULT_LIMIT = 60_000

ToolCall = Callable[[str, dict[str, Any]], Awaitable[tuple[str, bool]]]


class AgentError(RuntimeError):
    """The model could not produce an answer."""


@dataclass(frozen=True)
class Settings:
    """Model and effort for one request."""

    model: str
    effort: str

    @classmethod
    def from_environment(cls, effort: str) -> Settings:
        """Read overrides from QUINJET_AGENT_MODEL and QUINJET_AGENT_EFFORT."""
        return cls(
            model=os.environ.get("QUINJET_AGENT_MODEL") or DEFAULT_MODEL,
            effort=os.environ.get("QUINJET_AGENT_EFFORT") or effort,
        )


def client_factory() -> Any:
    """Build the API client from the ambient Anthropic credentials."""
    return AsyncAnthropic()


def clip(text: str, limit: int = TOOL_RESULT_LIMIT) -> str:
    """Bound a tool result so one large answer cannot fill the context."""
    if len(text) <= limit:
        return text
    return f"{text[:limit]}\n[truncated {len(text) - limit} characters]"


def final_text(response: Any) -> str:
    """Return the reply text, refusing to pass on a declined or empty answer."""
    if response.stop_reason == "refusal":
        details = getattr(response, "stop_details", None)
        reason = getattr(details, "explanation", None) or "the request was declined"
        message = f"The model declined: {reason}"
        raise AgentError(message)
    text = "\n".join(block.text for block in response.content if block.type == "text").strip()
    if not text:
        message = f"The model returned no text (stop reason {response.stop_reason})"
        raise AgentError(message)
    return text


async def _create(client: Any, settings: Settings, **request: Any) -> Any:
    return await client.beta.messages.create(
        model=settings.model,
        max_tokens=MAX_TOKENS,
        output_config={"effort": settings.effort},
        betas=[FALLBACK_BETA],
        fallbacks="default",
        **request,
    )


async def complete(settings: Settings, system: str, prompt: str) -> str:
    """Answer one prompt without tools."""
    client = client_factory()
    response = await _create(
        client,
        settings,
        system=system,
        messages=[{"role": "user", "content": prompt}],
    )
    return final_text(response)


async def run_agent(
    settings: Settings,
    system: str,
    prompt: str,
    tools: list[dict[str, Any]],
    call: ToolCall,
) -> str:
    """Let the model call tools until it answers or runs out of turns."""
    client = client_factory()
    messages: list[dict[str, Any]] = [{"role": "user", "content": prompt}]
    for _ in range(MAX_TOOL_TURNS):
        response = await _create(client, settings, system=system, tools=tools, messages=messages)
        if response.stop_reason == "pause_turn":
            messages.append({"role": "assistant", "content": response.content})
            continue
        if response.stop_reason != "tool_use":
            return final_text(response)
        messages.append({"role": "assistant", "content": response.content})
        results = []
        for block in response.content:
            if block.type != "tool_use":
                continue
            text, is_error = await call(block.name, dict(block.input))
            results.append(
                {
                    "type": "tool_result",
                    "tool_use_id": block.id,
                    "content": clip(text),
                    "is_error": is_error,
                },
            )
        messages.append({"role": "user", "content": results})
    message = f"The investigation did not finish within {MAX_TOOL_TURNS} tool turns"
    raise AgentError(message)
