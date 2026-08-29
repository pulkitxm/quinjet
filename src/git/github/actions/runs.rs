#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    #[doc = " The workflow runs GitHub Actions started for this pull request's head"]
    #[doc = " commit. Rerunning, cancelling, artifacts and deployment approvals are"]
    #[doc = " all addressed by run, so every operation begins here."]
    pub(crate) fn pull_request_workflow_runs(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<PullRequestWorkflowRuns> {
        let repository = &pull_request.base_repository;
        let key = format!(
            "workflow-runs-v1\n{}\n{}",
            repository.url.trim_end_matches('/'),
            pull_request.head_oid
        );
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Ttl(RUN_CACHE_TTL),
            refresh,
            [
                OsString::from("api"),
                OsString::from("--paginate"),
                OsString::from(format!(
                    "repos/{}/actions/runs?head_sha={}&per_page=100",
                    repository.name_with_owner, pull_request.head_oid
                )),
                OsString::from("--jq"),
                OsString::from(RUN_TSV_JQ),
            ],
            "unable to list the workflow runs for this pull request",
        )?;
        let mut runs = parse_workflow_runs(&response.data)?;
        let truncated = runs.len() > MAX_WORKFLOW_RUNS;
        runs.truncate(MAX_WORKFLOW_RUNS);
        runs.sort_by_key(|run| std::cmp::Reverse(run.id));
        Ok(PullRequestWorkflowRuns {
            head_oid: pull_request.head_oid.clone(),
            runs,
            truncated,
            from_cache: response.disposition != super::super::CacheDisposition::Network,
        })
    }

    #[doc = " The Actions job behind a named check run, which is what reruns one"]
    #[doc = " job rather than a whole workflow. A check whose link does not end in"]
    #[doc = " `/job/<id>` has no job to rerun."]
    pub(crate) fn check_job_id(check: &PullRequestCheck) -> Result<u64> {
        check.job_id().with_context(|| {
            format!(
                "the `{}` check is not a GitHub Actions job, so it cannot be rerun on its own",
                check.name
            )
        })
    }

    pub(super) fn actions_post(
        &self,
        repository: &GitHubRepository,
        endpoint: &str,
        fields: &[(&str, String)],
        context: &str,
    ) -> Result<()> {
        let mut args = vec![
            OsString::from("api"),
            OsString::from("--method"),
            OsString::from("POST"),
            OsString::from(format!("repos/{}/{endpoint}", repository.name_with_owner)),
        ];
        for (name, value) in fields {
            args.push(OsString::from("-f"));
            args.push(OsString::from(format!("{name}={value}")));
        }
        let output = self.run_gh(args)?;
        if !output.status.success() {
            bail!("{}", bounded_command_error(context, &output));
        }
        Ok(())
    }
}

fn parse_workflow_runs(output: &[u8]) -> Result<Vec<WorkflowRun>> {
    let mut runs = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [id, name, status, conclusion, url, attempt, event] =
            parse_tsv_record::<RUN_TSV_FIELDS>(record)
                .with_context(|| format!("invalid workflow-run record {}", index + 1))?;
        runs.push(WorkflowRun {
            id: id.parse().unwrap_or_default(),
            name,
            state: WorkflowRunState::parse(&status, &conclusion),
            status,
            conclusion,
            url,
            attempt: attempt.parse().unwrap_or(1),
            event,
        });
    }
    Ok(runs)
}
