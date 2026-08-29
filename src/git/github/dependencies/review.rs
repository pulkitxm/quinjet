#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    #[doc = " What this pull request does to the dependency graph, from GitHub's"]
    #[doc = " comparison of the two commits. Both sides are immutable object names,"]
    #[doc = " so the answer is cached forever and a new head asks a different"]
    #[doc = " question."]
    pub(crate) fn pull_request_dependencies(
        &self,
        pull_request: &PullRequest,
    ) -> Result<PullRequestDependencies> {
        let repository = &pull_request.base_repository;
        let mut listing = PullRequestDependencies {
            base_oid: pull_request.base_oid.clone(),
            head_oid: pull_request.head_oid.clone(),
            ..PullRequestDependencies::default()
        };
        let endpoint = format!(
            "repos/{}/dependency-graph/compare/{}...{}",
            repository.name_with_owner, pull_request.base_oid, pull_request.head_oid
        );
        let key = format!(
            "dependency-review-v1\n{}\n{}\n{}",
            repository.url.trim_end_matches('/'),
            pull_request.base_oid,
            pull_request.head_oid
        );
        let changes = self.dependency_records(&key, &endpoint, DEPENDENCY_TSV_JQ)?;
        listing.changes = pair_changes(parse_dependencies(&changes)?);
        let vulnerabilities = self.dependency_records(
            &format!("{key}\nvulnerabilities"),
            &endpoint,
            VULNERABILITY_TSV_JQ,
        )?;
        listing.vulnerabilities = parse_vulnerabilities(&vulnerabilities)?;
        listing.finish();
        Ok(listing)
    }

    fn dependency_records(&self, key: &str, endpoint: &str, jq: &str) -> Result<Vec<u8>> {
        let response = self.checked_cached_gh(
            key,
            CacheLife::Immutable,
            false,
            [
                OsString::from("api"),
                OsString::from(endpoint),
                OsString::from("--jq"),
                OsString::from(jq),
            ],
            "unable to compare this pull request's dependencies",
        )?;
        Ok(response.data)
    }
}

#[doc = " GitHub reports a version bump as a removal and an addition of the same"]
#[doc = " package. Pairing them is what turns two rows nobody asked about into the"]
#[doc = " one row a reviewer wants: what moved, and from where."]
pub(super) fn pair_changes(records: Vec<DependencyDelta>) -> Vec<DependencyDelta> {
    let mut paired: Vec<DependencyDelta> = Vec::new();
    let (removed, rest): (Vec<DependencyDelta>, Vec<DependencyDelta>) = records
        .into_iter()
        .partition(|record| record.change == DependencyChange::Removed);
    let mut unmatched = removed;
    for mut record in rest {
        if record.change == DependencyChange::Added
            && let Some(index) = unmatched.iter().position(|candidate| {
                candidate.name == record.name
                    && candidate.ecosystem == record.ecosystem
                    && candidate.manifest == record.manifest
            })
        {
            let previous = unmatched.remove(index);
            record.change = DependencyChange::Changed;
            record.previous_version = previous.version;
            record.previous_license = previous.license;
        }
        paired.push(record);
    }
    paired.extend(unmatched);
    paired
}

pub(super) fn parse_dependencies(output: &[u8]) -> Result<Vec<DependencyDelta>> {
    let mut changes = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            change,
            manifest,
            ecosystem,
            name,
            version,
            license,
            scope,
            vulnerabilities,
        ] = parse_tsv_record::<DEPENDENCY_TSV_FIELDS>(record)
            .with_context(|| format!("invalid dependency record {}", index + 1))?;
        let change = match change.to_ascii_lowercase().as_str() {
            "added" => DependencyChange::Added,
            "removed" => DependencyChange::Removed,
            other => bail!("GitHub reported an unknown dependency change `{other}`"),
        };
        changes.push(DependencyDelta {
            change,
            ecosystem,
            name,
            version,
            previous_version: String::new(),
            manifest,
            scope: DependencyScope::parse(&scope),
            license,
            previous_license: String::new(),
            vulnerabilities: vulnerabilities.parse().unwrap_or_default(),
        });
    }
    Ok(changes)
}

pub(super) fn parse_vulnerabilities(output: &[u8]) -> Result<Vec<DependencyVulnerability>> {
    let mut vulnerabilities = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [package, version, severity, advisory, summary, patched] =
            parse_tsv_record::<VULNERABILITY_TSV_FIELDS>(record)
                .with_context(|| format!("invalid vulnerability record {}", index + 1))?;
        vulnerabilities.push(DependencyVulnerability {
            package,
            version,
            severity: AdvisorySeverity::parse(&severity),
            advisory,
            summary,
            first_patched_version: patched,
        });
    }
    Ok(vulnerabilities)
}
