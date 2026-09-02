#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    #[doc = " Every artifact the head commit's workflow runs uploaded, across runs."]
    #[doc = " A run whose artifacts cannot be read leaves a warning rather than"]
    #[doc = " failing the listing."]
    pub(crate) fn pull_request_artifacts(
        &self,
        pull_request: &PullRequest,
        runs: &PullRequestWorkflowRuns,
    ) -> PullRequestArtifacts {
        let repository = &pull_request.base_repository;
        let mut listing = PullRequestArtifacts {
            head_oid: pull_request.head_oid.clone(),
            truncated: runs.truncated,
            ..PullRequestArtifacts::default()
        };
        for run in &runs.runs {
            match self.run_artifacts(repository, run) {
                Err(error) => listing.warnings.push(format!(
                    "unable to list artifacts for {}: {error:#}",
                    run.name
                )),
                Ok(artifacts) => listing.artifacts.extend(artifacts),
            }
        }
        listing.artifacts.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| right.id.cmp(&left.id))
        });
        if listing.artifacts.len() > MAX_ARTIFACTS {
            listing.truncated = true;
            listing.artifacts.truncate(MAX_ARTIFACTS);
        }
        listing
    }

    fn run_artifacts(
        &self,
        repository: &GitHubRepository,
        run: &WorkflowRun,
    ) -> Result<Vec<WorkflowArtifact>> {
        let key = format!(
            "workflow-artifacts-v1\n{}\n{}\n{}\n{}",
            repository.url.trim_end_matches('/'),
            run.id,
            run.attempt,
            run.state.word()
        );
        let life = if run.state.is_active() {
            CacheLife::Ttl(RUN_CACHE_TTL)
        } else {
            CacheLife::Immutable
        };
        let response = self.checked_cached_gh(
            &key,
            life,
            false,
            [
                OsString::from("api"),
                OsString::from("--paginate"),
                OsString::from(format!(
                    "repos/{}/actions/runs/{}/artifacts?per_page=100",
                    repository.name_with_owner, run.id
                )),
                OsString::from("--jq"),
                OsString::from(ARTIFACT_TSV_JQ),
            ],
            "unable to list workflow-run artifacts",
        )?;
        parse_artifacts(&response.data, run)
    }

    #[doc = " Stream one artifact archive to disk. The response is a redirect to a"]
    #[doc = " zip that can be hundreds of megabytes, so it is written straight to a"]
    #[doc = " file rather than read into the bounded stdout buffer every other"]
    #[doc = " GitHub read uses."]
    pub(crate) fn download_artifact(
        &self,
        pull_request: &PullRequest,
        artifact: &WorkflowArtifact,
        directory: &Path,
    ) -> Result<PathBuf> {
        if artifact.expired {
            bail!(
                "artifact `{}` has expired and can no longer be downloaded",
                artifact.name
            );
        }
        let file_name = artifact.safe_file_name()?;
        std::fs::create_dir_all(directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
        let destination = directory.join(&file_name);
        let staging = directory.join(format!("{file_name}.part"));
        let file = std::fs::File::create(&staging)
            .with_context(|| format!("failed to create {}", staging.display()))?;
        let status = self.run_gh_to_file(
            [
                OsString::from("api"),
                OsString::from(format!(
                    "repos/{}/actions/artifacts/{}/zip",
                    pull_request.base_repository.name_with_owner, artifact.id
                )),
            ],
            file,
        );
        match status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                drop(std::fs::remove_file(&staging));
                bail!(
                    "GitHub refused to download artifact `{}` ({status})",
                    artifact.name
                );
            }
            Err(error) => {
                drop(std::fs::remove_file(&staging));
                return Err(error);
            }
        }
        std::fs::rename(&staging, &destination)
            .with_context(|| format!("failed to write {}", destination.display()))?;
        Ok(destination)
    }
}

fn parse_artifacts(output: &[u8], run: &WorkflowRun) -> Result<Vec<WorkflowArtifact>> {
    let mut artifacts = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            id,
            name,
            size,
            expired,
            expires_at,
            created_at,
            run_id,
            download_url,
        ] = parse_tsv_record::<ARTIFACT_TSV_FIELDS>(record)
            .with_context(|| format!("invalid workflow-artifact record {}", index + 1))?;
        artifacts.push(WorkflowArtifact {
            id: id.parse().unwrap_or_default(),
            name,
            size_in_bytes: size.parse().unwrap_or_default(),
            expired: expired == "true",
            expires_at,
            created_at,
            run_id: run_id.parse().unwrap_or(run.id),
            workflow: run.name.clone(),
            download_url,
        });
    }
    Ok(artifacts)
}
