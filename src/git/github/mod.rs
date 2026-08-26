mod checks;
mod conversation;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};
use std::{env, thread};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

pub(crate) use self::checks::{
    CheckLogLine, CheckLogSeverity, CheckRunLog, CheckStep, PullRequestCheck,
    PullRequestCheckStatus, PullRequestChecks, unix_now,
};
pub(crate) use self::conversation::{ConversationEntry, ConversationKind, PullRequestConversation};
use super::diff::{
    DiffDocument, DiffLineCounts, PullRequestDetails, parse_diff, parse_numstat,
    split_patch_by_file,
};
use super::{MAX_DIFF_BYTES, Repository, StackOperation, text, trim_ascii};

const MAX_GIT_REMOTES: usize = 32;
const MAX_REMOTE_URL_ENTRIES: usize = 64;
const MAX_REMOTE_URLS: usize = 32;
const MAX_GITHUB_REPOSITORIES: usize = 16;
const MAX_GH_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_PULL_REQUEST_TITLE_BYTES: usize = 16 * 1024;
const MAX_PULL_REQUEST_BODY_BYTES: usize = 256 * 1024;
const MAX_GH_ERROR_BYTES: usize = 256 * 1024;
const MAX_PR_PATH_BYTES: usize = 8 * 1024 * 1024;
const MAX_PR_PATHS: usize = 16_384;
const MAX_FILE_COUNT_PAGES: usize = 64;
#[doc = " A single file's patch is cached only if it is small enough that one file"]
#[doc = " cannot crowd out the rest of a pull request."]
const MAX_CACHED_PATCH_BYTES: usize = 1024 * 1024;
#[doc = " The cache now holds immutable content (finished run logs, patches for a"]
#[doc = " fixed pair of commits) rather than only small metadata blobs, so the budget"]
#[doc = " is sized for those and pruned oldest-first."]
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 2_048;
const REPOSITORY_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PULL_REQUEST_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const TEMPORARY_REPOSITORY_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_MAGIC: &[u8] = b"quinjet-gh-cache-v1\n";

const PULL_REQUEST_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){id number title body author{login} state isDraft createdAt updatedAt url baseRefName baseRefOid headRefName headRefOid headRepository{nameWithOwner} isCrossRepository additions deletions changedFiles locked viewerCanClose viewerCanReopen viewerCanUpdate viewerCanUpdateBranch viewerCanSubscribe viewerCanReact viewerDidAuthor viewerSubscription mergeStateStatus mergeable maintainerCanModify viewerCanMergeAsAdmin autoMergeRequest{mergeMethod} mergeQueueEntry{id position state} mergeQueue{id} reviewDecision}}}";
const PULL_REQUEST_VIEW_TSV_JQ: &str = r#".data.repository.pullRequest | select(. != null) | [(.id // ""), (.number|tostring), .title, (.body // ""), (.author.login // "ghost"), .state, (.isDraft|tostring), .updatedAt, .url, .baseRefName, .headRefName, (.headRepository.nameWithOwner // ""), (.isCrossRepository|tostring), (.additions|tostring), (.deletions|tostring), (.changedFiles|tostring), .baseRefOid, .headRefOid, .createdAt, (.locked|tostring), (.viewerCanClose|tostring), (.viewerCanReopen|tostring), (.viewerCanUpdate|tostring), (.viewerCanUpdateBranch|tostring), (.viewerCanSubscribe|tostring), (.viewerCanReact|tostring), (.viewerDidAuthor|tostring), (.viewerSubscription // ""), (.mergeStateStatus // "UNKNOWN"), (.mergeable // "UNKNOWN"), (.maintainerCanModify|tostring), (.viewerCanMergeAsAdmin|tostring), (.autoMergeRequest.mergeMethod // ""), (.mergeQueueEntry.id // ""), ((.mergeQueueEntry.position // 0)|tostring), (.mergeQueueEntry.state // ""), (.mergeQueue.id // ""), (.reviewDecision // "")] | @tsv"#;
const REPOSITORY_TSV_TEMPLATE: &str = "{{.nameWithOwner}}{{\"\\t\"}}{{.url}}{{\"\\n\"}}";
const PULL_REQUEST_TSV_FIELDS: usize = 38;
#[cfg(not(test))]
const RECENT_PULL_REQUESTS_CACHE_KEY: &str = "recent-pull-requests-v2";
const MAX_RECENT_PULL_REQUESTS: usize = 20;
const MAX_RECENT_CACHE_SCAN: usize = 256;
const MAX_RECENT_CACHE_ENTRY_BYTES: u64 = 384 * 1024;

static TEMPORARY_REPOSITORY_ID: AtomicU64 = AtomicU64::new(0);
static CACHE_WRITE_ID: AtomicU64 = AtomicU64::new(0);

mod api;
mod cache;
mod change_index;
mod discovery;
mod http;
mod model;
mod operation;
mod parsing;
mod prepared;
mod process;
mod pull_request;
mod review;
mod stack;
mod temporary;

pub(crate) use cache::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use change_index::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use http::*;
pub(crate) use model::*;
pub(crate) use operation::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use parsing::*;
pub(crate) use prepared::*;
pub(crate) use process::{BoundedOutput, bounded_command_error, run_bounded_command};
#[cfg(test)]
pub(crate) use review::PullRequestReviewComment;
pub(crate) use review::{
    PullRequestReviewDecision, PullRequestReviewOperation, PullRequestReviewSide,
    PullRequestReviewSnapshot, PullRequestReviewThread, PullRequestReviewThreadSubject,
};
pub(crate) use stack::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use temporary::*;

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
