use serde::Deserialize;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Deserialize)]
struct CheckPage {
    total_count: usize,
    check_runs: Vec<CommitCheck>,
}

#[derive(Deserialize)]
struct CommitCheck {
    name: String,
    status: String,
    conclusion: Option<String>,
    html_url: Option<String>,
    details_url: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
    app: Option<CheckApp>,
}

#[derive(Deserialize)]
struct CheckApp {
    name: String,
}

#[derive(Deserialize)]
struct StatusPage {
    total_count: usize,
    statuses: Vec<CommitStatus>,
}

#[derive(Deserialize)]
struct CommitStatus {
    context: String,
    state: String,
    description: Option<String>,
    target_url: Option<String>,
    created_at: String,
    updated_at: String,
}

impl Repository {
    pub(crate) fn commit_checks(
        &self,
        request: &PullRequest,
        oid: &str,
        refresh: bool,
    ) -> Result<PullRequestChecks> {
        if !super::super::is_commit_oid(oid) {
            bail!("A full commit hash is required");
        }
        let key = format!("commit-checks-v1\n{}\n{oid}", request.base_repository.url);
        if !refresh
            && let Some(cached) =
                super::super::cache_read(&key, CacheLife::Ttl(CHECK_LIST_CACHE_TTL))
        {
            return Ok(PullRequestChecks {
                checks: serde_json::from_slice(&cached)?,
                from_cache: true,
            });
        }
        let mut checks = Vec::new();
        for page in 1..=5 {
            let data = self.commit_check_page(request, oid, "check-runs", page)?;
            let listing: CheckPage = serde_json::from_slice(&data)?;
            if listing.total_count > 500 {
                bail!("The commit has more than 500 check runs");
            }
            checks.extend(listing.check_runs.into_iter().map(PullRequestCheck::from));
            if page * 100 >= listing.total_count {
                break;
            }
        }
        for page in 1..=5 {
            let data = self.commit_check_page(request, oid, "status", page)?;
            let listing: StatusPage = serde_json::from_slice(&data)?;
            if listing.total_count > 500 {
                bail!("The commit has more than 500 status contexts");
            }
            checks.extend(listing.statuses.into_iter().map(PullRequestCheck::from));
            if page * 100 >= listing.total_count {
                break;
            }
        }
        checks.sort_by_key(|check| (check.workflow.to_lowercase(), check.name.to_lowercase()));
        if let Ok(data) = serde_json::to_vec(&checks) {
            super::super::cache_write(&key, &data);
        }
        Ok(PullRequestChecks {
            checks,
            from_cache: false,
        })
    }

    fn commit_check_page(
        &self,
        request: &PullRequest,
        oid: &str,
        endpoint: &str,
        page: usize,
    ) -> Result<Vec<u8>> {
        let output = self.run_gh([
            "api".to_owned(),
            "--hostname".to_owned(),
            request.base_repository.host().to_owned(),
            format!(
                "repos/{}/commits/{oid}/{endpoint}?per_page=100&page={page}",
                request.base_repository.name_with_owner
            ),
        ])?;
        if !output.status.success() || output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to load commit checks", &output)
            );
        }
        Ok(output.stdout)
    }
}

impl From<CommitCheck> for PullRequestCheck {
    fn from(check: CommitCheck) -> Self {
        let conclusion = check.conclusion.unwrap_or_default();
        Self {
            status: PullRequestCheckStatus::from_conclusion(&check.status, &conclusion),
            name: check.name,
            workflow: check.app.map_or_else(String::new, |app| app.name),
            state: if conclusion.is_empty() {
                check.status
            } else {
                conclusion
            },
            description: String::new(),
            link: check
                .details_url
                .filter(|url| !url.is_empty())
                .or(check.html_url)
                .unwrap_or_default(),
            started_at: check.started_at.unwrap_or_default(),
            completed_at: check.completed_at.unwrap_or_default(),
        }
    }
}

impl From<CommitStatus> for PullRequestCheck {
    fn from(status: CommitStatus) -> Self {
        let running = status.state == "pending";
        Self {
            status: match status.state.as_str() {
                "success" => PullRequestCheckStatus::Passed,
                "failure" | "error" => PullRequestCheckStatus::Failed,
                "pending" => PullRequestCheckStatus::Pending,
                _ => PullRequestCheckStatus::Unknown,
            },
            name: status.context,
            workflow: "Status".to_owned(),
            state: status.state,
            description: status.description.unwrap_or_default(),
            link: status.target_url.unwrap_or_default(),
            started_at: status.created_at,
            completed_at: if running {
                String::new()
            } else {
                status.updated_at
            },
        }
    }
}
