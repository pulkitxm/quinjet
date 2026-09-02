#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " One check run on the head commit, and how many annotations it published."]
struct AnnotatedCheckRun {
    id: u64,
    name: String,
    annotations_count: usize,
    url: String,
    status: String,
}

impl Repository {
    #[doc = " Every annotation the head commit's check runs published, flattened into"]
    #[doc = " one ordered list. One request lists the check runs; each run that says"]
    #[doc = " it has annotations costs one more."]
    pub(crate) fn pull_request_annotations(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<PullRequestAnnotations> {
        let repository = &pull_request.base_repository;
        let (runs, from_cache) = self.head_check_runs(pull_request, refresh)?;
        let mut annotations = PullRequestAnnotations {
            head_oid: pull_request.head_oid.clone(),
            from_cache,
            ..PullRequestAnnotations::default()
        };
        let annotated: Vec<AnnotatedCheckRun> = runs
            .into_iter()
            .filter(|run| run.annotations_count > 0)
            .collect();
        let reachable = annotated.len().min(MAX_ANNOTATED_CHECK_RUNS);
        if annotated.len() > reachable {
            annotations.truncated = true;
            annotations.warnings.push(format!(
                "{} check runs published annotations; only the first {reachable} were read",
                annotated.len()
            ));
        }
        let mut total = 0;
        for run in annotated.into_iter().take(reachable) {
            match self.check_run_annotations(&repository.name_with_owner, &run) {
                Err(error) => annotations.warnings.push(format!(
                    "unable to read annotations for {}: {error:#}",
                    run.name
                )),
                Ok(read) => {
                    total += run.annotations_count;
                    annotations.annotations.extend(read);
                }
            }
        }
        if total > MAX_ANNOTATIONS {
            annotations.truncated = true;
            annotations.warnings.push(format!(
                "this pull request has {total} annotations; only the {MAX_ANNOTATIONS} most severe are listed"
            ));
        }
        annotations.finish();
        Ok(annotations)
    }

    #[doc = " The head commit's check runs, which is the only place a check run's"]
    #[doc = " numeric id and its annotation count are exposed together. A check run"]
    #[doc = " id is not the Actions job id `pr logs` uses, so both are carried."]
    fn head_check_runs(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<(Vec<AnnotatedCheckRun>, bool)> {
        let repository = &pull_request.base_repository;
        let key = format!(
            "check-runs-v1\n{}\n{}",
            repository.url.trim_end_matches('/'),
            pull_request.head_oid
        );
        if !refresh
            && let Some(cached) = cache_read(&key, CacheLife::Ttl(ANNOTATION_LIST_CACHE_TTL))
        {
            return Ok((parse_check_runs(&cached)?, true));
        }
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Ttl(ANNOTATION_LIST_CACHE_TTL),
            refresh,
            [
                OsString::from("api"),
                OsString::from("--paginate"),
                OsString::from(format!(
                    "repos/{}/commits/{}/check-runs?per_page=100",
                    repository.name_with_owner, pull_request.head_oid
                )),
                OsString::from("--jq"),
                OsString::from(CHECK_RUN_TSV_JQ),
            ],
            "unable to list the head commit's check runs",
        )?;
        Ok((parse_check_runs(&response.data)?, false))
    }

    #[doc = " A finished run's annotations never change, and a running one's are"]
    #[doc = " keyed by the count it reported, so a new annotation asks a different"]
    #[doc = " question rather than ageing an old answer."]
    fn check_run_annotations(
        &self,
        repository: &str,
        run: &AnnotatedCheckRun,
    ) -> Result<Vec<CheckAnnotation>> {
        let key = format!(
            "check-annotations-v1\n{repository}\n{}\n{}\n{}",
            run.id, run.annotations_count, run.status
        );
        if let Some(cached) = cache_read(&key, CacheLife::Immutable) {
            return parse_annotations(&cached, run.id, &run.name, &run.url);
        }
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Immutable,
            false,
            [
                OsString::from("api"),
                OsString::from("--paginate"),
                OsString::from(format!(
                    "repos/{repository}/check-runs/{}/annotations?per_page=100",
                    run.id
                )),
                OsString::from("--jq"),
                OsString::from(ANNOTATION_TSV_JQ),
            ],
            "unable to read check-run annotations",
        )?;
        parse_annotations(&response.data, run.id, &run.name, &run.url)
    }
}

fn parse_check_runs(output: &[u8]) -> Result<Vec<AnnotatedCheckRun>> {
    let mut runs = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [id, name, annotations_count, url, status] =
            parse_tsv_record::<CHECK_RUN_TSV_FIELDS>(record)
                .with_context(|| format!("invalid check-run record {}", index + 1))?;
        runs.push(AnnotatedCheckRun {
            id: id.parse().unwrap_or_default(),
            name,
            annotations_count: annotations_count.parse().unwrap_or_default(),
            url,
            status,
        });
    }
    Ok(runs)
}

fn parse_annotations(
    output: &[u8],
    check_run_id: u64,
    check: &str,
    check_url: &str,
) -> Result<Vec<CheckAnnotation>> {
    let mut annotations = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let [
            path,
            start_line,
            end_line,
            start_column,
            end_column,
            level,
            title,
            message,
            raw_details,
            url,
        ] = parse_tsv_record::<ANNOTATION_TSV_FIELDS>(record)
            .with_context(|| format!("invalid check-run annotation record {}", index + 1))?;
        let start = start_line.parse().unwrap_or_default();
        let end: usize = end_line.parse().unwrap_or_default();
        annotations.push(CheckAnnotation {
            check: check.to_owned(),
            check_run_id,
            check_url: check_url.to_owned(),
            path: PathBuf::from(path),
            start_line: start,
            end_line: end.max(start),
            start_column: optional_position(&start_column),
            end_column: optional_position(&end_column),
            severity: AnnotationSeverity::parse(&level),
            title,
            message,
            raw_details,
            url,
            placement: AnnotationPlacement::Unknown,
        });
    }
    Ok(annotations)
}

#[doc = " GitHub reports a missing column as null, which the jq program renders as"]
#[doc = " 0; a column of zero does not exist, so both mean absent."]
fn optional_position(value: &str) -> Option<usize> {
    value.parse().ok().filter(|position| *position > 0)
}
