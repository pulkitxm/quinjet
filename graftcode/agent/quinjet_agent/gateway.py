"""Quinjet's grafted read commands, reached over its Graftcode gateway's MCP endpoint."""

from __future__ import annotations

import json
import os
from contextlib import AsyncExitStack
from typing import Any, Self

from mcp import ClientSession
from mcp.client.streamable_http import streamable_http_client

DEFAULT_URL = "http://localhost:7381/mcp"
READ_TOOLS = frozenset(
    {
        "status",
        "diff",
        "log",
        "show",
        "search",
        "branchList",
        "branchCompare",
        "prView",
        "prFiles",
        "prCommits",
        "prDiff",
        "prConversation",
        "prChecks",
        "prLogs",
        "prReviewsShow",
        "stackView",
    },
)


def gateway_url() -> str:
    """Return the Quinjet gateway address from QUINJET_GRAFT_URL."""
    return os.environ.get("QUINJET_GRAFT_URL") or DEFAULT_URL


def anthropic_tool(tool: Any) -> dict[str, Any]:
    """Describe one MCP tool in the shape the Messages API expects."""
    schema = dict(tool.input_schema or {"type": "object", "properties": {}})
    schema.setdefault("type", "object")
    return {
        "name": tool.name,
        "description": tool.description or tool.name,
        "input_schema": schema,
    }


def complete_arguments(schema: dict[str, Any], arguments: dict[str, Any]) -> dict[str, Any]:
    """Send every declared parameter, because Graftcode binds arguments by position."""
    completed = dict.fromkeys(schema.get("properties", {}), "")
    completed.update(arguments)
    return completed


def result_text(result: Any) -> str:
    """Flatten an MCP tool result into text."""
    structured = getattr(result, "structured_content", None) or {}
    value = structured.get("value")
    if isinstance(value, str):
        return value
    parts = [getattr(item, "text", "") for item in getattr(result, "content", None) or []]
    text = "\n".join(part for part in parts if part)
    if value is not None and not text:
        return json.dumps(value)
    return text


class QuinjetGateway:
    """An MCP session restricted to Quinjet's read-only tools."""

    def __init__(self, url: str) -> None:
        """Remember the gateway address; the session opens on entry."""
        self.url = url
        self.session: ClientSession | None = None
        self.schemas: dict[str, dict[str, Any]] = {}
        self.listed: list[dict[str, Any]] = []
        self._stack = AsyncExitStack()

    async def __aenter__(self) -> Self:
        """Open the MCP session and list the read tools it offers."""
        read, write = await self._stack.enter_async_context(streamable_http_client(self.url))
        self.session = await self._stack.enter_async_context(ClientSession(read, write))
        await self.session.initialize()
        listing = await self.session.list_tools()
        for tool in listing.tools:
            if tool.name in READ_TOOLS:
                described = anthropic_tool(tool)
                self.listed.append(described)
                self.schemas[tool.name] = described["input_schema"]
        return self

    async def __aexit__(self, *exception: object) -> None:
        """Close the MCP session."""
        await self._stack.aclose()

    def tools(self) -> list[dict[str, Any]]:
        """Return the read tools as Messages API tool definitions."""
        return list(self.listed)

    async def call(self, name: str, arguments: dict[str, Any]) -> tuple[str, bool]:
        """Run one read tool and return its text and whether it failed."""
        if self.session is None or name not in self.schemas:
            return f"{name} is not one of Quinjet's read tools", True
        result = await self.session.call_tool(
            name,
            complete_arguments(self.schemas[name], arguments),
        )
        return result_text(result), bool(getattr(result, "is_error", False))


def open_gateway(url: str) -> QuinjetGateway:
    """Create the gateway session used by investigations."""
    return QuinjetGateway(url)
