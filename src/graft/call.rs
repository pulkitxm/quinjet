use std::time::Duration;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};

use super::client::{McpSession, Tool, ToolReply};
use super::context::{self, ContextKey};
use super::registry::Gateway;
use crate::cli::{EXIT_NOT_FOUND, EXIT_UNAVAILABLE, Failure};
use crate::git::Repository;

pub(crate) const LIST_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) const CALL_TIMEOUT: Duration = Duration::from_secs(600);
const RECENT_COMMITS: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct GraftCall {
    pub gateway: Gateway,
    pub tool: String,
    pub arguments: Map<String, Value>,
    pub fill_context: bool,
    pub pull_request: Option<u64>,
    pub check_log: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraftReply {
    pub gateway: String,
    pub tool: String,
    pub filled: Vec<String>,
    pub empty: Vec<String>,
    pub text: String,
    pub is_error: bool,
    pub structured: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewayTools {
    pub gateway: String,
    pub url: String,
    #[serde(skip)]
    pub token_env: Option<String>,
    pub tools: Vec<Tool>,
    pub error: Option<String>,
}

impl GatewayTools {
    pub(crate) fn source(&self) -> Gateway {
        Gateway {
            name: self.gateway.clone(),
            url: self.url.clone(),
            token_env: self.token_env.clone(),
        }
    }
}

pub(crate) fn list_tools(gateways: &[Gateway]) -> Vec<GatewayTools> {
    gateways
        .iter()
        .map(|gateway| {
            let listed = McpSession::connect(&gateway.url, gateway.token(), LIST_TIMEOUT)
                .and_then(|mut session| session.list_tools());
            let (tools, error) = match listed {
                Ok(tools) => (tools, None),
                Err(error) => (Vec::new(), Some(format!("{error:#}"))),
            };
            GatewayTools {
                gateway: gateway.name.clone(),
                url: gateway.url.clone(),
                token_env: gateway.token_env.clone(),
                tools,
                error,
            }
        })
        .collect()
}

pub(crate) fn call(repository: &Repository, request: GraftCall) -> Result<GraftReply> {
    let mut session = McpSession::connect(
        &request.gateway.url,
        request.gateway.token(),
        request.timeout,
    )
    .map_err(|error| unavailable(&request.gateway, &error))?;
    let tools = session
        .list_tools()
        .map_err(|error| unavailable(&request.gateway, &error))?;
    let tool = find_tool(&tools, &request.gateway.name, &request.tool)?;
    let mut arguments = request.arguments.clone();
    let report = if request.fill_context {
        context::fill(
            tool,
            &mut arguments,
            Some(&mut |key| provide(repository, &request, key)),
        )?
    } else {
        context::fill(tool, &mut arguments, None)?
    };
    let name = tool.name.clone();
    let ToolReply {
        text,
        is_error,
        structured,
    } = session
        .call_tool(&name, arguments)
        .map_err(|error| unavailable(&request.gateway, &error))?;
    Ok(GraftReply {
        gateway: request.gateway.name,
        tool: name,
        filled: report.filled,
        empty: report.empty,
        text,
        is_error,
        structured,
    })
}

pub(crate) fn find_tool<'a>(tools: &'a [Tool], gateway: &str, wanted: &str) -> Result<&'a Tool> {
    if let Some(tool) = tools.iter().find(|tool| tool.name == wanted) {
        return Ok(tool);
    }
    let folded: Vec<&Tool> = tools
        .iter()
        .filter(|tool| fold(&tool.name) == fold(wanted))
        .collect();
    if let [only] = folded.as_slice() {
        return Ok(only);
    }
    Err(Failure::new(
        EXIT_NOT_FOUND,
        format!("the `{gateway}` gateway has no tool called `{wanted}`"),
    )
    .hint(if tools.is_empty() {
        "the gateway lists no tools".to_owned()
    } else {
        format!(
            "its tools are: {}",
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
    .into())
}

fn fold(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn unavailable(gateway: &Gateway, error: &anyhow::Error) -> anyhow::Error {
    Failure::new(
        EXIT_UNAVAILABLE,
        format!("the `{}` gateway failed: {error:#}", gateway.name),
    )
    .hint(format!(
        "check that a gateway is running at {}",
        gateway.url
    ))
    .into()
}

fn provide(
    repository: &Repository,
    request: &GraftCall,
    key: ContextKey,
) -> Result<Option<String>> {
    let text = match key {
        ContextKey::RepoPath => Some(repository.root().display().to_string()),
        ContextKey::Branch => {
            let status = repository.status()?;
            Some(status.branch.head).filter(|head| !head.is_empty())
        }
        ContextKey::StagedDiff => Some(repository.patch_text(true, context::CONTEXT_TEXT_LIMIT)?),
        ContextKey::WorkingDiff => Some(repository.patch_text(false, context::CONTEXT_TEXT_LIMIT)?),
        ContextKey::Diff => {
            let staged = repository.patch_text(true, context::CONTEXT_TEXT_LIMIT)?;
            if staged.trim().is_empty() {
                Some(repository.patch_text(false, context::CONTEXT_TEXT_LIMIT)?)
            } else {
                Some(staged)
            }
        }
        ContextKey::RecentCommits => {
            let commits = repository
                .history("HEAD", 0, RECENT_COMMITS)
                .unwrap_or_default();
            let mut text = String::new();
            for commit in commits {
                text.push_str(&commit.subject);
                text.push('\n');
            }
            Some(text)
        }
        ContextKey::PullRequest => request.pull_request.map(|number| number.to_string()),
        ContextKey::CheckLog => request.check_log.clone(),
    };
    Ok(text.map(context::bounded))
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::super::client::ToolParam;
    use super::*;

    fn tool(name: &str) -> Tool {
        Tool {
            name: name.to_owned(),
            title: None,
            description: None,
            params: vec![ToolParam {
                name: "staged_diff".to_owned(),
                description: None,
                required: true,
            }],
        }
    }

    #[test]
    fn tools_resolve_exactly_then_across_case_styles() {
        let tools = [tool("draft_commit_message"), tool("prChecks")];
        find_tool(&tools, "agent", "draft_commit_message").unwrap();
        assert_eq!(
            find_tool(&tools, "agent", "draftCommitMessage")
                .map(|tool| tool.name.as_str())
                .ok(),
            Some("draft_commit_message")
        );
        let error = find_tool(&tools, "agent", "explain").err();
        let failure = error
            .as_ref()
            .and_then(|error| error.downcast_ref::<Failure>());
        assert_eq!(failure.map(|failure| failure.code), Some(EXIT_NOT_FOUND));
        assert_eq!(
            failure.and_then(|failure| failure.hint.as_deref()),
            Some("its tools are: draft_commit_message, prChecks")
        );
    }
}
