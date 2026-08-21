use std::ffi::OsString;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{
    BoundedOutput, CacheLife, PullRequest, Repository, bounded_command_error, parse_tsv_record,
};

const MAX_CHECK_LOG_BYTES: usize = 8 * 1024 * 1024;
/// Check state is the one thing here that genuinely changes minute to minute,
/// so it is the one thing kept on a clock rather than on an identity.
const CHECK_LIST_CACHE_TTL: Duration = Duration::from_secs(30);
/// A ceiling on how much a single pull request will warm in the background.
const MAX_PREFETCHED_CHECK_LOGS: usize = 32;
const MAX_CHECK_LOG_LINES: usize = 200_000;
const CHECK_TSV_FIELDS: usize = 8;
const STEP_TSV_FIELDS: usize = 6;

const CHECK_TSV_JQ: &str = r#".[] | [.name, .workflow, .state, .bucket, (.description // ""), (.link // ""), (.startedAt // ""), (.completedAt // "")] | @tsv"#;
const JOB_STEPS_TSV_JQ: &str = r#".steps[]? | [((.number // 0)|tostring), (.name // ""), (.status // ""), (.conclusion // ""), (.started_at // ""), (.completed_at // "")] | @tsv"#;

mod model;
mod parsing;
mod repository;

pub(crate) use model::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use parsing::*;

#[cfg(test)]
mod tests;
