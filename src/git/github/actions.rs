use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{
    CacheLife, GitHubRepository, PullRequest, PullRequestCheck, Repository, bounded_command_error,
    parse_tsv_record,
};

#[doc = " Which runs exist for a commit, and what they produced, changes while CI"]
#[doc = " is running, so both listings stay on the same short clock the check"]
#[doc = " listing uses."]
const RUN_CACHE_TTL: Duration = Duration::from_secs(30);
#[doc = " A commit with more workflow runs than this is not a pull request anyone"]
#[doc = " is reading; the cap is reported rather than silently applied."]
const MAX_WORKFLOW_RUNS: usize = 100;
const MAX_ARTIFACTS: usize = 200;
#[doc = " An artifact archive goes straight to disk, but a name GitHub reports is"]
#[doc = " written by whoever wrote the workflow, so it is never used as a path."]
const MAX_ARTIFACT_NAME_BYTES: usize = 255;
const RUN_TSV_FIELDS: usize = 7;
const ARTIFACT_TSV_FIELDS: usize = 8;
const PENDING_TSV_FIELDS: usize = 5;
const DEPLOYMENT_TSV_FIELDS: usize = 6;

const RUN_TSV_JQ: &str = r#".workflow_runs[] | [(.id|tostring), (.name // ""), (.status // ""), (.conclusion // ""), (.html_url // ""), ((.run_attempt // 1)|tostring), (.event // "")] | @tsv"#;
const ARTIFACT_TSV_JQ: &str = r#".artifacts[] | [(.id|tostring), (.name // ""), ((.size_in_bytes // 0)|tostring), ((.expired // false)|tostring), (.expires_at // ""), (.created_at // ""), ((.workflow_run.id // 0)|tostring), (.archive_download_url // "")] | @tsv"#;
const PENDING_TSV_JQ: &str = r#".[] | [((.environment.id // 0)|tostring), (.environment.name // ""), ((.wait_timer // 0)|tostring), ((.current_user_can_approve // false)|tostring), ([.reviewers[]? | (.reviewer.login // .reviewer.name // "")] | join(", "))] | @tsv"#;
const DEPLOYMENT_TSV_JQ: &str = r#".[] | [(.id|tostring), (.environment // ""), (.description // ""), (.created_at // ""), (.url // ""), ((.transient_environment // false)|tostring)] | @tsv"#;

mod artifacts;
mod deployments;
mod model;
mod operation;
mod runs;

pub(crate) use model::*;
pub(crate) use operation::WorkflowOperation;

#[cfg(test)]
mod tests;
