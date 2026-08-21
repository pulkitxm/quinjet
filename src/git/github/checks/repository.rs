#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    /// `gh pr checks` exits non-zero when any run failed, so a useful response
    /// has to be recognized by its content rather than by the exit status. That
    /// is why this reads `gh` directly instead of going through the cached
    /// helper, and caches the accepted body itself.
    pub(crate) fn pull_request_checks(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<PullRequestChecks> {
        let key = format!(
            "checks-v1\n{}\n{}\n{}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.number,
            pull_request.head_oid
        );
        if !refresh
            && let Some(cached) =
                super::super::cache_read(&key, CacheLife::Ttl(CHECK_LIST_CACHE_TTL))
        {
            return Ok(PullRequestChecks {
                checks: parse_pull_request_checks(&cached)?,
                from_cache: true,
            });
        }
        let output = self.run_gh(pull_request_checks_args(pull_request))?;
        let accepted_status = output.status.success()
            || matches!(output.status.code(), Some(1 | 8)) && !output.stdout.is_empty();
        if output.stdout_truncated {
            bail!("pull-request checks exceeded the metadata limit");
        }
        if !accepted_status {
            let error = String::from_utf8_lossy(&output.stderr);
            if error.to_ascii_lowercase().contains("no checks") {
                return Ok(PullRequestChecks::default());
            }
            bail!(
                "{}",
                bounded_command_error("unable to load pull-request checks", &output)
            );
        }
        let checks = parse_pull_request_checks(&output.stdout)?;
        super::super::cache_write(&key, &output.stdout);
        Ok(PullRequestChecks {
            checks,
            from_cache: false,
        })
    }

    /// Read a check run's steps and its raw log, then attach every log line to
    /// the step whose run window contains it. Runner output is timestamped in
    /// UTC and the steps API reports the same clock, so the ranges map exactly
    /// without guessing at group headings.
    ///
    /// The log endpoint serves whatever a running job has written so far, so
    /// repeating this call while a job runs is what makes the view tail it. Only
    /// the first seconds of a job answer 404, before the blob exists at all.
    pub(crate) fn pull_request_check_log(
        &self,
        pull_request: &PullRequest,
        check: &PullRequestCheck,
    ) -> Result<CheckRunLog> {
        let Some(job) = check.job_id() else {
            return Ok(CheckRunLog::unavailable(format!(
                "{} does not publish logs through GitHub Actions",
                check.name
            )));
        };
        let repository = &pull_request.base_repository.name_with_owner;
        let life = if check.status.is_running() {
            CacheLife::Ttl(Duration::ZERO)
        } else {
            CacheLife::Immutable
        };
        let mut steps = self.check_run_steps(repository, job, life)?;
        let (raw, truncated) = self.check_run_raw_log(repository, job, life)?;
        if raw.is_empty() && steps.is_empty() {
            return Ok(CheckRunLog::unavailable(
                "GitHub has not published anything for this check yet".to_owned(),
            ));
        }
        let (lines, line_limit_reached) = parse_check_log(&raw);
        let loose_lines = assign_lines_to_steps(&mut steps, lines);
        Ok(CheckRunLog {
            steps,
            loose_lines,
            truncated: truncated || line_limit_reached,
            unavailable: None,
            log_pending: raw.is_empty(),
        })
    }

    /// Read every finished run into the cache so that selecting any of them is
    /// answered from disk. Runs still in progress are skipped: their output is
    /// not cacheable, and re-reading it here would spend requests the live tail
    /// is about to spend anyway.
    pub(crate) fn prefetch_check_run_logs(
        &self,
        pull_request: &PullRequest,
        checks: &[PullRequestCheck],
        wanted: &dyn Fn() -> bool,
    ) -> usize {
        checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .take(MAX_PREFETCHED_CHECK_LOGS)
            .take_while(|_| wanted())
            .filter(|check| self.pull_request_check_log(pull_request, check).is_ok())
            .count()
    }

    fn check_run_steps(
        &self,
        repository: &str,
        job: u64,
        life: CacheLife,
    ) -> Result<Vec<CheckStep>> {
        let response = self.checked_cached_gh(
            &format!("check-steps-v1\n{repository}\n{job}\n{life:?}"),
            life,
            false,
            [
                OsString::from("api"),
                OsString::from(format!("repos/{repository}/actions/jobs/{job}")),
                OsString::from("--jq"),
                OsString::from(JOB_STEPS_TSV_JQ),
            ],
            "unable to read the check run steps",
        );
        match response {
            Err(_) => Ok(Vec::new()),
            Ok(response) => parse_check_steps(&response.data),
        }
    }

    fn check_run_raw_log(
        &self,
        repository: &str,
        job: u64,
        life: CacheLife,
    ) -> Result<(Vec<u8>, bool)> {
        let key = format!("check-log-v1\n{repository}\n{job}");
        if life == CacheLife::Immutable
            && let Some(cached) = super::super::cache_read_bounded(&key, life, MAX_CHECK_LOG_BYTES)
        {
            return Ok((cached, false));
        }
        let endpoint = format!("repos/{repository}/actions/jobs/{job}/logs");
        let output = self.run_gh_log([
            OsString::from("api"),
            OsString::from("--allow-escape-sequences"),
            OsString::from(&endpoint),
        ])?;
        let output = if output.status.success() || !rejects_unknown_flag(&output) {
            output
        } else {
            self.run_gh_log([OsString::from("api"), OsString::from(&endpoint)])?
        };
        if !output.status.success() && !output.stdout_truncated {
            if log_not_published(&output) {
                return Ok((Vec::new(), false));
            }
            bail!(
                "{}",
                bounded_command_error("unable to read the check run log", &output)
            );
        }
        if life == CacheLife::Immutable && !output.stdout_truncated && !output.stdout.is_empty() {
            super::super::cache_write_bounded(&key, &output.stdout, MAX_CHECK_LOG_BYTES);
        }
        Ok((output.stdout, output.stdout_truncated))
    }

    fn run_gh_log<I>(&self, args: I) -> Result<BoundedOutput>
    where
        I: IntoIterator<Item = OsString>,
    {
        self.run_gh_bounded(args, MAX_CHECK_LOG_BYTES)
    }
}
