use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use super::model::{self, Settings};
use crate::git::Repository;
use crate::graft::context::{CONTEXT_TEXT_LIMIT, bounded};
use crate::graft::schema::{describe, input_schema};

const RECENT_COMMITS: usize = 20;
const STYLE: &str = "Write plain text. Never use the em dash character.";

const COMMIT_MESSAGE: &str = "You write Git commit messages for staged changes. \
Use a Conventional Commits subject (feat, fix, docs, test, refactor, perf, ci, chore) in the imperative mood, \
at most 72 characters, describing what changed and why rather than listing files. \
Add a short body after a blank line only when the reason is not obvious from the subject. \
Follow the conventions visible in the recent commit subjects you are given. \
Reply with the commit message and nothing else.";

const REVIEW: &str = "You review a patch before it is committed. \
Report only real problems: bugs, edge cases that break, security issues, data loss, missing tests for changed behavior, \
and changes that contradict each other. \
List findings most severe first as `path:line - problem - why it matters`. \
If there is nothing worth fixing, reply exactly `No findings.`";

const CHECK_FAILURE: &str = "You explain why a continuous integration step failed. \
Read the log and answer with four short sections: Failing step, Error, Root cause, Fix. \
Quote the decisive log lines exactly. When the log does not show the cause, say what to read next.";

const INVESTIGATION: &str = "You triage a GitHub pull request for its author, using Quinjet's read-only Git and GitHub tools \
on the repository that holds it. Start with prView and prChecks. For a failing check, read prLogs for it. \
Use prDiff, prFiles, prConversation and show only when they help explain a failure or a review concern. \
Finish with: Verdict (ready, blocked, or needs work), Evidence, Root cause, Next steps. \
Cite check names, files and commits you actually read.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiTask {
    CommitMessage,
    Review,
    ExplainCheck { pull_request: u64, log: String },
    Investigate { pull_request: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AiAnswer {
    pub task: String,
    pub text: String,
    pub model: String,
}

impl AiTask {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::CommitMessage => "Draft Commit Message".to_owned(),
            Self::Review => "Review Changes".to_owned(),
            Self::ExplainCheck { pull_request, .. } => {
                format!("Explain Failing Check on #{pull_request}")
            }
            Self::Investigate { pull_request } => format!("Investigate #{pull_request}"),
        }
    }

    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::CommitMessage => "commit-message",
            Self::Review => "review",
            Self::ExplainCheck { .. } => "explain-check",
            Self::Investigate { .. } => "investigate",
        }
    }

    const fn effort(&self) -> &'static str {
        match self {
            Self::CommitMessage => "low",
            Self::ExplainCheck { .. } => "medium",
            Self::Review | Self::Investigate { .. } => "high",
        }
    }
}

pub(crate) fn run_task(repository: &Repository, task: &AiTask) -> Result<AiAnswer> {
    let settings = Settings::resolve(task.effort())?;
    let text = match task {
        AiTask::CommitMessage => {
            let staged = repository.patch_text(true, CONTEXT_TEXT_LIMIT)?;
            if staged.trim().is_empty() {
                bail!("nothing is staged; stage the changes to describe first");
            }
            let prompt = format!(
                "Branch: {}\n\nRecent commit subjects:\n{}\nStaged patch:\n<patch>\n{}\n</patch>",
                branch(repository),
                recent_subjects(repository),
                bounded(staged)
            );
            model::complete(&settings, &system(COMMIT_MESSAGE), &prompt)?
        }
        AiTask::Review => {
            let staged = repository.patch_text(true, CONTEXT_TEXT_LIMIT)?;
            let patch = if staged.trim().is_empty() {
                repository.patch_text(false, CONTEXT_TEXT_LIMIT)?
            } else {
                staged
            };
            if patch.trim().is_empty() {
                bail!("there are no changes to review");
            }
            let prompt = format!(
                "Branch: {}\n\n<patch>\n{}\n</patch>",
                branch(repository),
                bounded(patch)
            );
            model::complete(&settings, &system(REVIEW), &prompt)?
        }
        AiTask::ExplainCheck { pull_request, log } => {
            if log.trim().is_empty() {
                bail!("the check log is empty");
            }
            let prompt = format!(
                "Pull request #{pull_request}\n\n<log>\n{}\n</log>",
                model::clip(log, CONTEXT_TEXT_LIMIT)
            );
            model::complete(&settings, &system(CHECK_FAILURE), &prompt)?
        }
        AiTask::Investigate { pull_request } => investigate(&settings, repository, *pull_request)?,
    };
    Ok(AiAnswer {
        task: task.name().to_owned(),
        text,
        model: settings.model,
    })
}

fn system(prompt: &str) -> String {
    format!("{prompt} {STYLE}")
}

fn branch(repository: &Repository) -> String {
    repository
        .status()
        .map(|status| status.branch.head)
        .ok()
        .filter(|head| !head.is_empty())
        .unwrap_or_else(|| "(detached)".to_owned())
}

fn recent_subjects(repository: &Repository) -> String {
    let mut text = String::new();
    for commit in repository
        .history("HEAD", 0, RECENT_COMMITS)
        .unwrap_or_default()
    {
        text.push_str(&commit.subject);
        text.push('\n');
    }
    text
}

pub(crate) fn investigation_tools() -> Vec<Value> {
    crate::cli::graft_receiver(false)
        .investigation_methods()
        .map(|method| {
            json!({
                "name": method.name,
                "description": describe(method),
                "input_schema": input_schema(method, false),
            })
        })
        .collect()
}

fn investigate(settings: &Settings, repository: &Repository, pull_request: u64) -> Result<String> {
    let receiver = crate::cli::graft_receiver(false);
    let tools = investigation_tools();
    // nosemgrep: rust.lang.security.current-exe.current-exe
    let quinjet = std::env::current_exe().context("could not find the running Quinjet")?;
    let root = repository.root().to_path_buf();
    model::run_agent(
        settings,
        &system(INVESTIGATION),
        &format!(
            "Triage pull request #{pull_request}. Every tool parameter must be sent; pass \"\" for optional ones you do not need."
        ),
        &tools,
        &mut |name, input| {
            let Some(method) = receiver
                .investigation_methods()
                .find(|method| method.name == name)
            else {
                return (format!("Quinjet has no read tool called {name}"), true);
            };
            let values = input.as_object().cloned().unwrap_or_default();
            run_quinjet(&quinjet, &root, &method.argv(&values))
        },
    )
}

fn run_quinjet(quinjet: &Path, root: &Path, argv: &[String]) -> (String, bool) {
    let output = std::process::Command::new(quinjet)
        .arg("-C")
        .arg(root)
        .args(argv)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => {
            (String::from_utf8_lossy(&output.stdout).into_owned(), false)
        }
        Ok(output) => (
            format!(
                "quinjet {} failed: {}",
                argv.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            true,
        ),
        Err(error) => (format!("could not run quinjet: {error}"), true),
    }
}
