"""Exercise the agent methods against scripted model and gateway fakes."""

from __future__ import annotations

from types import SimpleNamespace
from typing import Any, Self
from unittest import mock

import pytest
from quinjet_agent import agent, gateway, model


def text_block(text: str) -> SimpleNamespace:
    """Build a text content block."""
    return SimpleNamespace(type="text", text=text)


def tool_block(identifier: str, name: str, arguments: dict[str, Any]) -> SimpleNamespace:
    """Build a tool_use content block."""
    return SimpleNamespace(type="tool_use", id=identifier, name=name, input=arguments)


def reply(stop_reason: str, *content: SimpleNamespace, **extra: Any) -> SimpleNamespace:
    """Build a Messages API response."""
    return SimpleNamespace(stop_reason=stop_reason, content=list(content), **extra)


class FakeClient:
    """Answer each request with the next scripted response."""

    def __init__(self, *responses: SimpleNamespace) -> None:
        """Queue the responses and record every request."""
        self.responses = list(responses)
        self.requests: list[dict[str, Any]] = []
        self.beta = SimpleNamespace(messages=SimpleNamespace(create=self.create))

    async def create(self, **request: Any) -> SimpleNamespace:
        """Record the request and return the next response."""
        self.requests.append({**request, "messages": list(request.get("messages", []))})
        return self.responses.pop(0)


class FakeGateway:
    """A Quinjet gateway with one read tool."""

    def __init__(self) -> None:
        """Start with no calls."""
        self.url = "http://quinjet.test/mcp"
        self.calls: list[tuple[str, dict[str, Any]]] = []

    async def __aenter__(self) -> Self:
        """Open nothing."""
        return self

    async def __aexit__(self, *exception: object) -> None:
        """Close nothing."""

    def tools(self) -> list[dict[str, Any]]:
        """List the one tool."""
        return [{"name": "prChecks", "description": "List checks", "input_schema": {}}]

    async def call(self, name: str, arguments: dict[str, Any]) -> tuple[str, bool]:
        """Record the call and answer with a failing check."""
        self.calls.append((name, arguments))
        return '[{"name": "Test", "status": "failed"}]', False


@pytest.fixture(autouse=True)
def default_settings(monkeypatch: pytest.MonkeyPatch) -> None:
    """Clear model overrides from the environment."""
    monkeypatch.setenv("QUINJET_AGENT_MODEL", "")
    monkeypatch.setenv("QUINJET_AGENT_EFFORT", "")


def use(monkeypatch: pytest.MonkeyPatch, client: FakeClient) -> None:
    """Route model calls to the fake client."""
    monkeypatch.setattr(model, "client_factory", lambda: client)


def test_commit_messages_use_the_default_model_with_fallbacks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A commit message comes from one low-effort request."""
    client = FakeClient(reply("end_turn", text_block("fix: keep the cursor in view\n")))
    use(monkeypatch, client)
    message = agent.Agent.draft_commit_message("+line\n", "feat: earlier\n", "main")
    assert message == "fix: keep the cursor in view"
    request = client.requests[0]
    assert request["model"] == "claude-opus-5-5"
    assert request["output_config"] == {"effort": "low"}
    assert request["betas"] == [model.FALLBACK_BETA]
    assert request["fallbacks"] == "default"
    assert "<patch>\n+line\n\n</patch>" in request["messages"][0]["content"]
    assert "feat: earlier" in request["messages"][0]["content"]


def test_environment_overrides_model_and_effort(monkeypatch: pytest.MonkeyPatch) -> None:
    """QUINJET_AGENT_MODEL and QUINJET_AGENT_EFFORT win over the defaults."""
    client = FakeClient(reply("end_turn", text_block("No findings.")))
    use(monkeypatch, client)
    overrides = {"QUINJET_AGENT_MODEL": "claude-sonnet-5-5", "QUINJET_AGENT_EFFORT": "max"}
    with mock.patch.dict("os.environ", overrides):
        assert agent.Agent.review_diff("+x", "main") == "No findings."
    assert client.requests[0]["model"] == "claude-sonnet-5-5"
    assert client.requests[0]["output_config"] == {"effort": "max"}


def test_empty_input_is_refused_before_any_request(monkeypatch: pytest.MonkeyPatch) -> None:
    """An empty staged patch is a caller error, not a model call."""
    client = FakeClient()
    use(monkeypatch, client)
    with pytest.raises(ValueError, match="staged patch is empty"):
        agent.Agent.draft_commit_message("  \n", "", "main")
    assert client.requests == []


def test_declined_and_empty_answers_raise(monkeypatch: pytest.MonkeyPatch) -> None:
    """A refusal or a reply without text surfaces as an error."""
    details = SimpleNamespace(explanation="policy")
    use(monkeypatch, FakeClient(reply("refusal", stop_details=details)))
    with pytest.raises(model.AgentError, match="declined: policy"):
        agent.Agent.explain_check_failure("error: boom", 7)
    use(monkeypatch, FakeClient(reply("end_turn")))
    with pytest.raises(model.AgentError, match="no text"):
        agent.Agent.explain_check_failure("error: boom", 7)


def test_check_failures_carry_the_pull_request_and_log(monkeypatch: pytest.MonkeyPatch) -> None:
    """The log reaches the model inside log tags."""
    client = FakeClient(reply("end_turn", text_block("Failing step: Test")))
    use(monkeypatch, client)
    assert agent.Agent.explain_check_failure("error: boom", 7) == "Failing step: Test"
    content = client.requests[0]["messages"][0]["content"]
    assert content.startswith("Pull request #7")
    assert "<log>\nerror: boom\n</log>" in content


def test_investigations_loop_through_quinjet_tools(monkeypatch: pytest.MonkeyPatch) -> None:
    """Tool calls go to the gateway and their results go back to the model."""
    client = FakeClient(
        reply("tool_use", text_block("Reading checks"), tool_block("t1", "prChecks", {})),
        reply("pause_turn", text_block("still working")),
        reply("end_turn", text_block("Verdict: needs work")),
    )
    use(monkeypatch, client)
    fake = FakeGateway()
    with mock.patch.object(gateway, "open_gateway", return_value=fake):
        assert agent.Agent.investigate_pull_request(12) == "Verdict: needs work"
    assert fake.calls == [("prChecks", {})]
    assert client.requests[0]["tools"] == fake.tools()
    assert client.requests[0]["output_config"] == {"effort": "high"}
    results = client.requests[1]["messages"][-1]["content"]
    assert results[0]["tool_use_id"] == "t1"
    assert results[0]["is_error"] is False
    assert "failed" in results[0]["content"]
    assert [entry["role"] for entry in client.requests[2]["messages"]] == [
        "user",
        "assistant",
        "user",
        "assistant",
    ]


def test_investigations_stop_after_the_turn_limit(monkeypatch: pytest.MonkeyPatch) -> None:
    """A model that never stops calling tools ends with an error."""
    looping = [
        reply("tool_use", tool_block(f"t{turn}", "prChecks", {}))
        for turn in range(model.MAX_TOOL_TURNS)
    ]
    use(monkeypatch, FakeClient(*looping))
    with (
        mock.patch.object(gateway, "open_gateway", return_value=FakeGateway()),
        pytest.raises(model.AgentError, match="did not finish"),
    ):
        agent.Agent.investigate_pull_request(12)


def test_every_declared_parameter_is_sent() -> None:
    """Missing parameters become empty strings so Graftcode binds them by name."""
    schema = {"properties": {"number": {}, "commit": {}, "repo": {}}}
    assert gateway.complete_arguments(schema, {"number": "4"}) == {
        "number": "4",
        "commit": "",
        "repo": "",
    }


def test_results_prefer_structured_strings() -> None:
    """Graftcode puts the method's return value under structuredContent.value."""
    structured = SimpleNamespace(structured_content={"value": "[]"}, content=[])
    assert gateway.result_text(structured) == "[]"
    plain = SimpleNamespace(structured_content=None, content=[text_block("a"), text_block("b")])
    assert gateway.result_text(plain) == "a\nb"
    number = SimpleNamespace(structured_content={"value": 3}, content=[])
    assert gateway.result_text(number) == "3"


def test_tools_keep_their_schema_and_gain_a_type() -> None:
    """MCP tools convert to Messages API tool definitions."""
    tool = SimpleNamespace(name="prLogs", description=None, input_schema={"properties": {}})
    assert gateway.anthropic_tool(tool) == {
        "name": "prLogs",
        "description": "prLogs",
        "input_schema": {"properties": {}, "type": "object"},
    }


def test_long_tool_results_are_clipped() -> None:
    """One huge diff cannot fill the context window."""
    clipped = model.clip("x" * 10, limit=4)
    assert clipped == "xxxx\n[truncated 6 characters]"


def test_the_gateway_address_comes_from_the_environment() -> None:
    """QUINJET_GRAFT_URL points investigations at another gateway."""
    with mock.patch.dict("os.environ", {"QUINJET_GRAFT_URL": "http://box:7381/mcp"}):
        assert gateway.gateway_url() == "http://box:7381/mcp"
    with mock.patch.dict("os.environ", {"QUINJET_GRAFT_URL": ""}):
        assert gateway.gateway_url() == gateway.DEFAULT_URL
