#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const MAX_LOCAL_HISTORY_COMMITS: usize = 16_384;

impl Repository {
    pub(super) fn local_pull_request_commits(
        &self,
        pull_request: &PullRequest,
    ) -> Result<Option<PullRequestCommits>> {
        let range = format!("{}..{}", pull_request.base_oid, pull_request.head_oid);
        let output = self.local_commit_read(&[
            "rev-list",
            "--boundary",
            "--topo-order",
            &format!("--max-count={}", MAX_LOCAL_HISTORY_COMMITS + 1),
            &range,
            "--",
        ])?;
        if !output.status.success() || output.stdout_truncated {
            return Ok(None);
        }
        let ids = text(&output.stdout);
        let ids = ids.lines().collect::<Vec<_>>();
        let total_commits = ids.iter().filter(|id| !id.starts_with('-')).count();
        if total_commits > MAX_LOCAL_HISTORY_COMMITS {
            return Ok(None);
        }
        let shallow = self.git_common_dir()?.join("shallow");
        if shallow.is_file() {
            let shallow = fs::read_to_string(shallow)?;
            let shallow = shallow.lines().collect::<BTreeSet<_>>();
            if ids
                .iter()
                .any(|id| shallow.contains(id.trim_start_matches('-')))
            {
                return Ok(None);
            }
        }
        let key = format!("local-{}", pull_request_commits_cache_key(pull_request));
        if let Some(cached) = cached_pull_request_commits(&key, pull_request) {
            return Ok(Some(cached));
        }
        let output = self.local_commit_read(&[
            "log",
            "--topo-order",
            &format!("--max-count={MAX_PULL_REQUEST_COMMITS}"),
            &format!("--format={}", crate::git::history::LOG_FORMAT),
            &range,
            "--",
        ])?;
        if !output.status.success() || output.stdout_truncated {
            return Ok(None);
        }
        let mut commits = crate::git::history::parse_log(&output.stdout)
            .into_iter()
            .map(|commit| PullRequestCommit {
                url: format!(
                    "{}/commit/{}",
                    pull_request.base_repository.url.trim_end_matches('/'),
                    commit.id
                ),
                oid: commit.id,
                abbreviated_oid: commit.short_id,
                subject: bounded_text(&commit.subject, MAX_COMMIT_SUBJECT_BYTES),
                author: commit.author,
                author_login: None,
                authored_at: commit.authored_at,
                committed_at: commit.committed_at,
            })
            .collect::<Vec<_>>();
        if commits.len() != total_commits.min(MAX_PULL_REQUEST_COMMITS) {
            return Ok(None);
        }
        commits.reverse();
        let result = PullRequestCommits {
            truncated: total_commits > commits.len(),
            commits,
            total_commits,
            base_oid: pull_request.base_oid.clone(),
            head_oid: pull_request.head_oid.clone(),
            from_cache: false,
        };
        validate_pull_request_commits(pull_request, &result)?;
        if let Ok(data) = serde_json::to_vec(&result) {
            cache_write(&key, &data);
        }
        Ok(Some(result))
    }

    fn local_commit_read(&self, args: &[&str]) -> Result<BoundedOutput> {
        let mut command = Command::new("git");
        let _ = command
            .arg("-C")
            .arg(self.root())
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1");
        run_bounded_command(&mut command, MAX_GH_METADATA_BYTES, MAX_GH_ERROR_BYTES)
    }
}
