#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(crate) fn pull_request_history(
        &self,
        pull_request: &PullRequest,
    ) -> Result<PullRequestCommits> {
        validate_pull_request_oids(pull_request)?;
        if let Some(cached) =
            cached_pull_request_commits(&head_history_cache_key(pull_request), pull_request)
        {
            return Ok(cached);
        }
        if let Some(commits) = self.local_commit_snapshot(pull_request, false)? {
            return Ok(commits);
        }
        let temporary = TemporaryBareRepository::new()?;
        temporary.borrow_local_objects(self);
        let output = run_temp_git(
            &temporary.path,
            &[
                OsString::from("fetch"),
                OsString::from("--quiet"),
                OsString::from("--no-tags"),
                OsString::from("--filter=blob:none"),
                OsString::from(pull_request.base_repository.selector()),
                OsString::from(&pull_request.head_oid),
            ],
            128 * 1024,
            MAX_GH_ERROR_BYTES,
        )?;
        if !output.status.success() || output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to fetch complete PR head history", &output)
            );
        }
        let repository = Self {
            root: temporary.path.clone(),
            github_cli: self.github_cli.clone(),
            diff_highlighting: self.diff_highlighting,
        };
        repository
            .local_commit_snapshot(pull_request, false)?
            .context("The PR head history is incomplete after fetching")
    }
}
