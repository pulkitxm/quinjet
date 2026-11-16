use anyhow::Result;
use serde::Serialize;
use serde_json::{Map, Value};

use super::client::Tool;

pub(crate) const CONTEXT_TEXT_LIMIT: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ContextKey {
    RepoPath,
    Branch,
    StagedDiff,
    WorkingDiff,
    Diff,
    RecentCommits,
    PullRequest,
    CheckLog,
}

impl ContextKey {
    pub(crate) const ALL: [Self; 8] = [
        Self::RepoPath,
        Self::Branch,
        Self::StagedDiff,
        Self::WorkingDiff,
        Self::Diff,
        Self::RecentCommits,
        Self::PullRequest,
        Self::CheckLog,
    ];

    pub(crate) const fn canonical(self) -> &'static str {
        match self {
            Self::RepoPath => "repo_path",
            Self::Branch => "branch",
            Self::StagedDiff => "staged_diff",
            Self::WorkingDiff => "working_diff",
            Self::Diff => "diff",
            Self::RecentCommits => "recent_commits",
            Self::PullRequest => "pull_request",
            Self::CheckLog => "check_log",
        }
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::RepoPath => "the repository root on this machine",
            Self::Branch => "the checked-out branch",
            Self::StagedDiff => "the staged patch",
            Self::WorkingDiff => "the unstaged patch",
            Self::Diff => "the staged patch, or the unstaged one when nothing is staged",
            Self::RecentCommits => "the last 20 commit subjects on HEAD",
            Self::PullRequest => "the selected pull-request number",
            Self::CheckLog => "the selected or first failing check's log",
        }
    }
}

pub(crate) fn context_key(name: &str) -> Option<ContextKey> {
    let normalized: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| character.to_ascii_lowercase())
        .collect();
    match normalized.as_str() {
        "repopath" | "repositorypath" | "repositoryroot" | "reporoot" | "workdir" => {
            Some(ContextKey::RepoPath)
        }
        "branch" | "branchname" | "currentbranch" => Some(ContextKey::Branch),
        "stageddiff" | "stagedpatch" | "stagedchanges" => Some(ContextKey::StagedDiff),
        "workingdiff" | "unstageddiff" | "worktreediff" | "workingtreediff" => {
            Some(ContextKey::WorkingDiff)
        }
        "diff" | "patch" => Some(ContextKey::Diff),
        "recentcommits" | "commithistory" | "recenthistory" => Some(ContextKey::RecentCommits),
        "pullrequest" | "pullrequestnumber" | "prnumber" => Some(ContextKey::PullRequest),
        "checklog" | "cilog" | "failurelog" | "steplog" | "joblog" => Some(ContextKey::CheckLog),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Filled {
    pub filled: Vec<String>,
    pub empty: Vec<String>,
}

pub(crate) fn fill(
    tool: &Tool,
    arguments: &mut Map<String, Value>,
    provide: Option<&mut dyn FnMut(ContextKey) -> Result<Option<String>>>,
) -> Result<Filled> {
    let mut report = Filled::default();
    let mut provide = provide;
    for param in &tool.params {
        if arguments.contains_key(&param.name) {
            continue;
        }
        let value = match (context_key(&param.name), provide.as_mut()) {
            (Some(key), Some(provide)) => provide(key)?.map(|text| typed(key, text)),
            _ => None,
        };
        if let Some(value) = value {
            drop(arguments.insert(param.name.clone(), value));
            report.filled.push(param.name.clone());
        } else {
            drop(arguments.insert(param.name.clone(), Value::String(String::new())));
            report.empty.push(param.name.clone());
        }
    }
    Ok(report)
}

fn typed(key: ContextKey, text: String) -> Value {
    if key == ContextKey::PullRequest
        && let Ok(number) = text.parse::<u64>()
    {
        return Value::from(number);
    }
    Value::String(text)
}

pub(crate) fn bounded(mut text: String) -> String {
    if text.len() <= CONTEXT_TEXT_LIMIT {
        return text;
    }
    let mut end = CONTEXT_TEXT_LIMIT;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.truncate(end);
    text.push_str("\n[truncated by Quinjet]\n");
    text
}

pub(crate) fn parse_assignment(text: &str) -> Result<(String, Value)> {
    let (name, value) = text
        .split_once('=')
        .filter(|(name, _)| !name.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("`{text}` is not a name=value argument"))?;
    let value = serde_json::from_str::<Value>(value)
        .ok()
        .filter(|parsed| !parsed.is_string())
        .unwrap_or_else(|| Value::String(value.to_owned()));
    Ok((name.trim().to_owned(), value))
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::super::client::ToolParam;
    use super::*;

    fn tool(params: &[(&str, bool)]) -> Tool {
        Tool {
            name: "probe".to_owned(),
            title: None,
            description: None,
            params: params
                .iter()
                .map(|(name, required)| ToolParam {
                    name: (*name).to_owned(),
                    description: None,
                    required: *required,
                })
                .collect(),
        }
    }

    #[test]
    fn parameter_names_match_context_in_any_case_style() {
        assert_eq!(context_key("staged_diff"), Some(ContextKey::StagedDiff));
        assert_eq!(context_key("stagedDiff"), Some(ContextKey::StagedDiff));
        assert_eq!(context_key("check-log"), Some(ContextKey::CheckLog));
        assert_eq!(context_key("pullRequest"), Some(ContextKey::PullRequest));
        assert_eq!(context_key("repo"), None);
        assert_eq!(context_key("number"), None);
        for key in ContextKey::ALL {
            assert_eq!(context_key(key.canonical()), Some(key));
        }
    }

    #[test]
    fn filling_keeps_explicit_values_and_reports_gaps() {
        let tool = tool(&[
            ("staged_diff", true),
            ("pull_request", true),
            ("branch", false),
            ("style", true),
            ("tone", false),
        ]);
        let mut arguments = Map::new();
        drop(arguments.insert("branch".to_owned(), Value::from("explicit")));
        let mut asked = Vec::new();
        let report = fill(
            &tool,
            &mut arguments,
            Some(&mut |key| {
                asked.push(key);
                Ok(match key {
                    ContextKey::StagedDiff => Some("diff --git a b".to_owned()),
                    ContextKey::PullRequest => Some("42".to_owned()),
                    _ => None,
                })
            }),
        )
        .unwrap();
        assert_eq!(asked, [ContextKey::StagedDiff, ContextKey::PullRequest]);
        assert_eq!(report.filled, ["staged_diff", "pull_request"]);
        assert_eq!(report.empty, ["style", "tone"]);
        assert_eq!(arguments.get("pull_request"), Some(&Value::from(42)));
        assert_eq!(arguments.get("branch"), Some(&Value::from("explicit")));
        assert_eq!(arguments.get("tone"), Some(&Value::from("")));
    }

    #[test]
    fn without_context_every_parameter_is_still_sent() {
        let tool = tool(&[("staged_diff", true), ("tone", false)]);
        let mut arguments = Map::new();
        let report = fill(&tool, &mut arguments, None).unwrap();
        assert_eq!(report.filled, Vec::<String>::new());
        assert_eq!(report.empty, ["staged_diff", "tone"]);
        assert_eq!(arguments.len(), 2);
    }

    #[test]
    fn assignments_keep_text_and_decode_json_scalars() {
        assert_eq!(
            parse_assignment("limit=5").unwrap(),
            ("limit".to_owned(), Value::from(5))
        );
        assert_eq!(
            parse_assignment("flag=true").unwrap(),
            ("flag".to_owned(), Value::from(true))
        );
        assert_eq!(
            parse_assignment("message=a=b c").unwrap(),
            ("message".to_owned(), Value::from("a=b c"))
        );
        parse_assignment("=x").unwrap_err();
        parse_assignment("novalue").unwrap_err();
    }

    #[test]
    fn long_context_is_truncated_on_a_character_boundary() {
        let text = "é".repeat(CONTEXT_TEXT_LIMIT);
        let bounded = bounded(text);
        assert!(bounded.ends_with("[truncated by Quinjet]\n"));
        assert!(bounded.len() < CONTEXT_TEXT_LIMIT + 64);
    }
}
