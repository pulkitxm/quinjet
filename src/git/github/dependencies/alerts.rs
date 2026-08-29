#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    #[doc = " What security tooling says about this pull request: the code-scanning"]
    #[doc = " alerts open on its branch, and the vulnerable dependencies it adds."]
    #[doc = ""]
    #[doc = " Either read can be refused by a repository that has the feature off or"]
    #[doc = " a token without the scope. That is a warning rather than an error: a"]
    #[doc = " partial answer that says what it could not see beats no answer, and it"]
    #[doc = " must never read as clean."]
    pub(crate) fn pull_request_security(&self, pull_request: &PullRequest) -> PullRequestSecurity {
        let mut security = PullRequestSecurity {
            head_oid: pull_request.head_oid.clone(),
            ..PullRequestSecurity::default()
        };
        match self.code_scanning_alerts(&pull_request.base_repository, &pull_request.head_ref) {
            Ok(alerts) => security.alerts = alerts,
            Err(error) => security
                .warnings
                .push(format!("code scanning was not readable: {error:#}")),
        }
        match self.pull_request_dependencies(pull_request) {
            Ok(dependencies) => security.vulnerabilities = dependencies.vulnerabilities,
            Err(error) => security
                .warnings
                .push(format!("dependency review was not readable: {error:#}")),
        }
        security.finish();
        security
    }

    fn code_scanning_alerts(
        &self,
        repository: &GitHubRepository,
        head_ref: &str,
    ) -> Result<Vec<CodeScanningAlert>> {
        if head_ref.is_empty() {
            bail!("this pull request has no head branch to query");
        }
        let key = format!(
            "code-scanning-v1\n{}\n{head_ref}",
            repository.url.trim_end_matches('/')
        );
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Ttl(ALERT_CACHE_TTL),
            false,
            [
                OsString::from("api"),
                OsString::from(format!(
                    "repos/{}/code-scanning/alerts?ref=refs/heads/{head_ref}&state=open&per_page=100",
                    repository.name_with_owner
                )),
                OsString::from("--jq"),
                OsString::from(CODE_SCANNING_TSV_JQ),
            ],
            "unable to read code-scanning alerts",
        )?;
        parse_alerts(&response.data)
    }
}

fn parse_alerts(output: &[u8]) -> Result<Vec<CodeScanningAlert>> {
    let mut alerts = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [number, rule, severity, description, path, line, url] =
            parse_tsv_record::<CODE_SCANNING_TSV_FIELDS>(record)
                .with_context(|| format!("invalid code-scanning record {}", index + 1))?;
        alerts.push(CodeScanningAlert {
            number: number.parse().unwrap_or_default(),
            rule,
            severity: AdvisorySeverity::parse(&severity),
            description,
            path,
            line: line.parse().unwrap_or_default(),
            url,
        });
    }
    Ok(alerts)
}
