use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    CacheLife, GitHubRepository, PullRequest, PullRequestCommit, PullRequestCommits,
    PullRequestDiffIndex, PullRequestFileStatus, PullRequestReviewSnapshot,
    PullRequestReviewThread, Repository, is_commit_oid, text, trim_ascii,
};
use crate::state::ReviewProgressRecord;

#[doc = " Comparing more than a handful of distinct viewed commits would spend one"]
#[doc = " read each. Past this, a file keeps its mark with the commit unresolved"]
#[doc = " rather than Quinjet guessing."]
const MAX_COMPARED_VIEWED_COMMITS: usize = 8;
const MAX_COMPARE_PATH_BYTES: usize = 8 * 1024 * 1024;

mod delta;
mod model;
mod query;

pub(crate) use model::*;

#[cfg(test)]
mod tests;

#[doc = " How a caller says which commit the delta is measured from."]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewSinceRequest {
    #[doc = " The reviewer's own last visit, falling back to their last review."]
    LastReview,
    #[doc = " A commit the caller named, resolved against the pull request's own"]
    #[doc = " commit list so a typo cannot silently widen the delta."]
    Commit(String),
}

impl Repository {
    #[doc = " Resolve the commit a review delta is measured from, and say where the"]
    #[doc = " answer came from so the reading can print it."]
    pub(crate) fn resolve_review_since(
        &self,
        pull_request: &PullRequest,
        request: &ReviewSinceRequest,
        record: &ReviewProgressRecord,
        commits: &PullRequestCommits,
    ) -> Result<ReviewSince> {
        match request {
            ReviewSinceRequest::Commit(wanted) => Ok(ReviewSince {
                oid: resolve_commit(wanted, commits, pull_request)?,
                source: ReviewSinceSource::Explicit,
                detail: String::new(),
            }),
            ReviewSinceRequest::LastReview => {
                if !record.visited_oid.is_empty() && record.visited_oid != pull_request.head_oid {
                    return Ok(ReviewSince {
                        oid: record.visited_oid.clone(),
                        source: ReviewSinceSource::Visit,
                        detail: record.visited_at.clone(),
                    });
                }
                let mark = self.pull_request_viewer_review(pull_request)?;
                if is_commit_oid(&mark.commit_oid) {
                    return Ok(ReviewSince {
                        oid: mark.commit_oid,
                        source: ReviewSinceSource::Review,
                        detail: mark.state,
                    });
                }
                Ok(ReviewSince {
                    oid: commits.base_oid.clone(),
                    source: ReviewSinceSource::MergeBase,
                    detail: String::new(),
                })
            }
        }
    }

    #[doc = " Assemble one progress reading. The fetches happen here; the reduction"]
    #[doc = " is a pure function so two callers cannot disagree about what is left."]
    pub(crate) fn pull_request_review_progress(
        &self,
        pull_request: &PullRequest,
        index: &PullRequestDiffIndex,
        request: &ReviewSinceRequest,
    ) -> Result<ReviewProgress> {
        let record = crate::state::load_review_progress(
            &pull_request.base_repository.url,
            pull_request.number,
        );
        let commits = self.pull_request_commits(pull_request)?;
        let review = self.pull_request_review(pull_request)?;
        let mut warnings = Vec::new();
        let since = match self.resolve_review_since(pull_request, request, &record, &commits) {
            Ok(since) => since,
            Err(error) => {
                warnings.push(format!("{error:#}"));
                ReviewSince {
                    oid: commits.base_oid.clone(),
                    source: ReviewSinceSource::MergeBase,
                    detail: String::new(),
                }
            }
        };
        let repository = &pull_request.base_repository;
        let changed_since =
            self.changed_paths_between(repository, &since.oid, &pull_request.head_oid);
        if changed_since.is_none() && !since.oid.is_empty() {
            warnings.push(format!(
                "unable to compare {} with the head commit; the delta is unknown",
                short_oid(&since.oid)
            ));
        }
        let (changed_since_viewed, viewed_warnings) =
            self.compare_viewed_commits(pull_request, &record);
        warnings.extend(viewed_warnings);
        Ok(delta::build_progress(delta::ReviewProgressInputs {
            repository: &repository.name_with_owner,
            number: pull_request.number,
            head_oid: &pull_request.head_oid,
            since,
            record: &record,
            index,
            review: &review,
            commits: &commits,
            changed_since: changed_since.as_ref(),
            changed_since_viewed: &changed_since_viewed,
            warnings,
        }))
    }

    #[doc = " Compare each distinct commit a file was read at against the head, so a"]
    #[doc = " mark can be kept when nothing moved and reopened when something did."]
    fn compare_viewed_commits(
        &self,
        pull_request: &PullRequest,
        record: &ReviewProgressRecord,
    ) -> (Vec<(String, BTreeSet<PathBuf>)>, Vec<String>) {
        let mut wanted: Vec<String> = Vec::new();
        for file in &record.viewed {
            if file.head_oid != pull_request.head_oid
                && is_commit_oid(&file.head_oid)
                && !wanted.contains(&file.head_oid)
            {
                wanted.push(file.head_oid.clone());
            }
        }
        let mut warnings = Vec::new();
        if wanted.len() > MAX_COMPARED_VIEWED_COMMITS {
            warnings.push(format!(
                "files were read at {} different commits; only the {MAX_COMPARED_VIEWED_COMMITS} newest are compared",
                wanted.len()
            ));
            wanted.truncate(MAX_COMPARED_VIEWED_COMMITS);
        }
        let compared = wanted
            .into_iter()
            .filter_map(|oid| {
                let changed = self.changed_paths_between(
                    &pull_request.base_repository,
                    &oid,
                    &pull_request.head_oid,
                )?;
                Some((oid, changed))
            })
            .collect();
        (compared, warnings)
    }
}

#[doc = " Accept a full object name or a unique abbreviation, but only of a commit"]
#[doc = " this pull request actually contains, so `--since` can never widen the"]
#[doc = " delta to something unrelated."]
fn resolve_commit(
    wanted: &str,
    commits: &PullRequestCommits,
    pull_request: &PullRequest,
) -> Result<String> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        bail!("a review delta needs a commit to measure from");
    }
    if wanted.eq_ignore_ascii_case(&commits.base_oid)
        || wanted.eq_ignore_ascii_case(&pull_request.base_oid)
    {
        return Ok(commits.base_oid.clone());
    }
    let matches: Vec<&PullRequestCommit> = commits
        .commits
        .iter()
        .filter(|commit| {
            commit.oid.eq_ignore_ascii_case(wanted)
                || commit
                    .oid
                    .to_ascii_lowercase()
                    .starts_with(&wanted.to_ascii_lowercase())
        })
        .collect();
    match matches.as_slice() {
        [only] => Ok(only.oid.clone()),
        [] => bail!(
            "`{wanted}` does not name a commit in pull request #{}",
            pull_request.number
        ),
        _ => bail!("`{wanted}` matches more than one commit in this pull request"),
    }
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
}
