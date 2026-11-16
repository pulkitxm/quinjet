"""The class a Graftcode gateway hosts: every static method becomes a tool."""

from __future__ import annotations

import asyncio

from quinjet_agent import gateway, model, prompts


def _require(value: str, what: str) -> str:
    if not value or not value.strip():
        message = f"{what} is empty"
        raise ValueError(message)
    return value


async def _investigate(pull_request: int) -> str:
    async with gateway.open_gateway(gateway.gateway_url()) as quinjet:
        tools = quinjet.tools()
        if not tools:
            message = f"the Quinjet gateway at {quinjet.url} offers no read tools"
            raise model.AgentError(message)
        return await model.run_agent(
            model.Settings.from_environment("high"),
            prompts.INVESTIGATION,
            f"Triage pull request #{pull_request}.",
            tools,
            quinjet.call,
        )


class Agent:
    """Claude-backed help for the repository Quinjet has open."""

    @staticmethod
    def draft_commit_message(staged_diff: str, recent_commits: str, branch: str) -> str:
        """Draft a commit message for the staged change.

        Args:
            staged_diff: The staged patch, as printed by git diff --cached.
            recent_commits: Recent commit subjects on the branch, one per line.
            branch: The checked-out branch name.

        """
        prompt = (
            f"Branch: {branch}\n\nRecent commit subjects:\n{recent_commits}\n\n"
            f"Staged patch:\n<patch>\n{_require(staged_diff, 'The staged patch')}\n</patch>"
        )
        return asyncio.run(
            model.complete(model.Settings.from_environment("low"), prompts.COMMIT_MESSAGE, prompt),
        )

    @staticmethod
    def review_diff(diff: str, branch: str) -> str:
        """Review a patch for bugs and risky changes before it is committed.

        Args:
            diff: The patch to review; the staged one, or the unstaged one when nothing is staged.
            branch: The checked-out branch name.

        """
        prompt = f"Branch: {branch}\n\n<patch>\n{_require(diff, 'The patch')}\n</patch>"
        return asyncio.run(
            model.complete(model.Settings.from_environment("high"), prompts.REVIEW, prompt),
        )

    @staticmethod
    def explain_check_failure(check_log: str, pull_request: int) -> str:
        """Explain why a pull request's continuous integration check failed.

        Args:
            check_log: The failing check's log, with its steps.
            pull_request: The pull request number the check ran for.

        """
        prompt = (
            f"Pull request #{pull_request}\n\n"
            f"<log>\n{model.clip(_require(check_log, 'The check log'))}\n</log>"
        )
        return asyncio.run(
            model.complete(
                model.Settings.from_environment("medium"),
                prompts.CHECK_FAILURE,
                prompt,
            ),
        )

    @staticmethod
    def investigate_pull_request(pull_request: int) -> str:
        """Triage a pull request by reading its checks, logs, diff and conversation through Quinjet.

        Args:
            pull_request: The pull request number to investigate.

        """
        return asyncio.run(_investigate(int(pull_request)))
