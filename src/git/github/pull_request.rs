#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(crate) fn pull_request_lookup(
        &self,
        known_repositories: &[GitHubRepository],
        selected_repository: Option<&GitHubRepository>,
        number: u64,
        refresh: bool,
    ) -> Result<PullRequestSnapshot> {
        if number == 0 {
            bail!("Pull-request numbers must be positive integers");
        }
        let (repositories, mut warnings) = if known_repositories.is_empty() || refresh {
            self.github_repositories(refresh)?
        } else {
            (known_repositories.to_vec(), Vec::new())
        };
        let Some(repository) = select_repository(&repositories, selected_repository).cloned()
        else {
            bail!("No configured fetch or push remote resolves to a GitHub repository");
        };
        if selected_repository.is_some_and(|selected| {
            !repository
                .url
                .trim_end_matches('/')
                .eq_ignore_ascii_case(selected.url.trim_end_matches('/'))
        }) {
            warnings.push(format!(
                "The selected GitHub repository is no longer configured; using {}",
                repository.display_name()
            ));
        }
        let response = self.pull_request_metadata(&repository, &repositories, number, refresh)?;
        if response.1 == CacheDisposition::Stale {
            warnings.push(format!(
                "GitHub is unavailable; showing stale cached metadata for #{number}"
            ));
        }
        Ok(PullRequestSnapshot {
            repositories,
            selected_repository: Some(repository),
            pull_request: response.0,
            warnings,
            exact_number: Some(number),
            from_cache: response.1 != CacheDisposition::Network,
        })
    }

    pub(crate) fn prepare_pull_request_diff<F>(
        &self,
        pull_request: &PullRequest,
        progress: F,
    ) -> Result<PreparedPullRequest>
    where
        F: FnMut(PullRequestProgress),
    {
        self.prepare_pull_request_comparison(pull_request, false, progress)
    }

    pub(crate) fn prepare_pull_request_stack_diff<F>(
        &self,
        stack: &PullRequestStack,
        from: usize,
        to: usize,
        progress: F,
    ) -> Result<PreparedPullRequest>
    where
        F: FnMut(PullRequestProgress),
    {
        let pull_request = stack.comparison(from, to)?;
        self.prepare_pull_request_comparison(&pull_request, true, progress)
    }

    fn prepare_pull_request_comparison<F>(
        &self,
        pull_request: &PullRequest,
        exact_base: bool,
        mut progress: F,
    ) -> Result<PreparedPullRequest>
    where
        F: FnMut(PullRequestProgress),
    {
        let (repository, merge_base, head, api_counts) =
            if self.has_commit(&pull_request.base_oid) && self.has_commit(&pull_request.head_oid) {
                progress(PullRequestProgress::FindingMergeBase);
                let base = if exact_base {
                    pull_request.base_oid.clone()
                } else {
                    self.merge_base(&pull_request.base_oid, &pull_request.head_oid)?
                };
                (
                    PreparedRepository::Opened(self.root().to_path_buf()),
                    base,
                    pull_request.head_oid.clone(),
                    None,
                )
            } else {
                progress(PullRequestProgress::PreparingRepository);
                let merge_base_hint = if exact_base {
                    Some(pull_request.base_oid.clone())
                } else {
                    self.merge_base_from_api(pull_request)
                };
                let api_counts = if exact_base {
                    None
                } else {
                    self.pull_request_file_counts_from_api(pull_request)
                };
                let temporary = TemporaryBareRepository::new()?;
                temporary.borrow_local_objects(self);
                let (merge_base, head) = fetch_pull_request(
                    &temporary.path,
                    pull_request,
                    merge_base_hint.as_deref(),
                    &mut progress,
                )?;
                (
                    PreparedRepository::Temporary(temporary),
                    merge_base,
                    head,
                    api_counts,
                )
            };
        progress(PullRequestProgress::EnumeratingFiles);
        let (files, truncated) =
            changed_files_in_repository(repository.path(), &merge_base, &head, api_counts)?;
        let total_files = if truncated {
            pull_request.changed_files.max(files.len())
        } else {
            files.len()
        };
        let mut pull_request = pull_request.clone();
        if exact_base {
            pull_request.changed_files = total_files;
            pull_request.additions = files
                .iter()
                .filter_map(|file| file.counts)
                .map(|counts| counts.additions)
                .sum();
            pull_request.deletions = files
                .iter()
                .filter_map(|file| file.counts)
                .map(|counts| counts.deletions)
                .sum();
        }
        Ok(PreparedPullRequest {
            repository,
            pull_request,
            merge_base,
            head,
            index: PullRequestDiffIndex {
                files,
                total_files,
                truncated,
            },
        })
    }

    pub(super) fn pull_request_metadata(
        &self,
        repository: &GitHubRepository,
        repositories: &[GitHubRepository],
        number: u64,
        refresh: bool,
    ) -> Result<(PullRequest, CacheDisposition)> {
        let response = self.checked_cached_gh(
            &format!(
                "pull-request-v4\n{}\n{number}",
                repository.url.trim_end_matches('/')
            ),
            CacheLife::Ttl(PULL_REQUEST_CACHE_TTL),
            refresh,
            pull_request_view_args(repository, number),
            "unable to load pull request",
        )?;
        let mut pull_requests = parse_pull_requests(&response.data, repository, repositories)
            .context("unable to parse exact pull-request metadata")?;
        if pull_requests.len() != 1 {
            bail!(
                "GitHub returned {} records for pull request #{number}",
                pull_requests.len()
            );
        }
        Ok((pull_requests.remove(0), response.disposition))
    }

    pub(super) fn merge_base(&self, base: &str, head: &str) -> Result<String> {
        let output = self.checked([
            OsString::from("merge-base"),
            OsString::from(base),
            OsString::from(head),
        ])?;
        let merge_base = text(trim_ascii(&output));
        if merge_base.is_empty() {
            bail!("Git did not return a pull-request merge base");
        }
        Ok(merge_base)
    }
}
