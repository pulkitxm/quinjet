use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use super::Repository;
use super::github::{
    AnnotationSeverity, MergeGate, PullRequest, PullRequestAnnotations, PullRequestFeedback,
    run_bounded_command,
};

#[doc = " A session records what a coding process was asked to do and what it"]
#[doc = " actually did. Neither list is unbounded: a session that has run a"]
#[doc = " thousand commands is not a session anybody is still reading."]
const MAX_TASKS: usize = 64;
const MAX_CHECKPOINTS: usize = 64;
const MAX_VERIFICATIONS: usize = 64;
#[doc = " Enough of a failing command's output to see why it failed, without"]
#[doc = " turning the record into a log file."]
const MAX_RECORDED_OUTPUT: usize = 8 * 1024;
const MAX_COMMAND_STDERR: usize = 8 * 1024;
#[doc = " A session patch is read for review, not archived, so it is bounded the"]
#[doc = " same way every other diff Quinjet reads is."]
const MAX_SESSION_PATCH_BYTES: usize = 4 * 1024 * 1024;

mod model;
mod publish;
mod start;
mod verify;

pub(crate) use model::*;
pub(crate) use publish::{WorkPublishPlan, plan_work_publish, publish_work};
pub(crate) use start::{WorkStartRequest, build_work_session};
pub(crate) use verify::{run_work_verification, work_diff};
use verify::{session_worktree, worktree_git};

#[cfg(test)]
mod tests;
