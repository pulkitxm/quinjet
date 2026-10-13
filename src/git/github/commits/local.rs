#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(super) fn local_pull_request_commits(
        &self,
        pull_request: &PullRequest,
    ) -> Result<Option<PullRequestCommits>> {
        self.local_commit_snapshot(pull_request, true)
    }

    pub(super) fn local_commit_snapshot(
        &self,
        pull_request: &PullRequest,
        introduced_only: bool,
    ) -> Result<Option<PullRequestCommits>> {
        let range = if introduced_only {
            format!("{}..{}", pull_request.base_oid, pull_request.head_oid)
        } else {
            pull_request.head_oid.clone()
        };
        let shallow = self.git_common_dir()?.join("shallow");
        let shallow = if shallow.is_file() {
            fs::read_to_string(shallow)?
                .lines()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let mut total_commits = 0;
        let mut incomplete = false;
        let complete = self.local_commit_read(
            &["rev-list", "--boundary", "--topo-order", &range, "--"],
            b'\n',
            |record| {
                let id = text(record);
                let id = id.trim();
                incomplete |= shallow.contains(id.trim_start_matches('-'));
                total_commits += usize::from(!id.is_empty() && !id.starts_with('-'));
            },
        )?;
        if !complete || incomplete {
            return Ok(None);
        }
        let key = if introduced_only {
            format!("local-{}", pull_request_commits_cache_key(pull_request))
        } else {
            head_history_cache_key(pull_request)
        };
        if let Some(cached) = cached_pull_request_commits(&key, pull_request) {
            return Ok(Some(cached));
        }
        let mut commits = Vec::new();
        let complete =
            self.local_commit_read(
                &[
                    "log",
                    "--topo-order",
                    &format!("--format={}", crate::git::history::LOG_FORMAT),
                    &range,
                    "--",
                ],
                0x1e,
                |record| {
                    commits.extend(crate::git::history::parse_log(record).into_iter().map(
                        |commit| PullRequestCommit {
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
                        },
                    ));
                },
            )?;
        if !complete || commits.len() != total_commits {
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

    fn local_commit_read(
        &self,
        args: &[&str],
        delimiter: u8,
        consume: impl FnMut(&[u8]),
    ) -> Result<bool> {
        let mut command = Command::new("git");
        let _ = command
            .arg("-C")
            .arg(self.root())
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1");
        local_process::read_records(&mut command, delimiter, consume)
    }
}
