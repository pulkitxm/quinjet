use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};

use super::diff::{DiffDocument, PullRequestDetails, parse_diff};
use super::{MAX_DIFF_BYTES, Repository, text, trim_ascii};

const MAX_GIT_REMOTES: usize = 32;
const MAX_REMOTE_URL_ENTRIES: usize = 64;
const MAX_REMOTE_URLS: usize = 32;
const MAX_GITHUB_REPOSITORIES: usize = 16;
const MAX_GH_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_PULL_REQUEST_TITLE_BYTES: usize = 16 * 1024;
const MAX_GH_ERROR_BYTES: usize = 256 * 1024;
const MAX_PR_PATH_BYTES: usize = 2 * 1024 * 1024;
const MAX_PR_PATHS: usize = 4_096;
const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 256;
const REPOSITORY_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PULL_REQUEST_PAGE_CACHE_TTL: Duration = Duration::from_secs(60);
const PULL_REQUEST_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const TEMPORARY_REPOSITORY_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_MAGIC: &[u8] = b"quinjet-gh-cache-v1\n";

pub const DEFAULT_PULL_REQUEST_PAGE_SIZE: usize = 25;
pub const DEFAULT_PULL_REQUEST_DIFF_PAGE_SIZE: usize = 20;
pub const MAX_PULL_REQUESTS: usize = 10_000;

const PULL_REQUEST_FIELDS: &str = "number,title,author,state,isDraft,updatedAt,url,baseRefName,baseRefOid,headRefName,headRefOid,headRepository,isCrossRepository,additions,deletions,changedFiles";
const PULL_REQUEST_VIEW_TSV_JQ: &str = r#"[(.number|tostring), .title, (.author.login // "ghost"), .state, (.isDraft|tostring), .updatedAt, .url, .baseRefName, .headRefName, (.headRepository.nameWithOwner // ""), (.isCrossRepository|tostring), (.additions|tostring), (.deletions|tostring), (.changedFiles|tostring), .baseRefOid, .headRefOid] | @tsv"#;
// Keep the GraphQL batch at 50: large enough to populate two UI pages per
// round-trip, but small enough for responsive progress updates and bounded output.
const PULL_REQUEST_GRAPHQL_QUERY: &str = r#"query($owner: String!, $name: String!, $endCursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 50, after: $endCursor, orderBy: {field: UPDATED_AT, direction: DESC}) {
      totalCount
      pageInfo { hasNextPage endCursor }
      nodes {
        number title state isDraft updatedAt url baseRefName baseRefOid headRefName headRefOid
        author { login }
        headRepository { nameWithOwner }
        isCrossRepository additions deletions changedFiles
      }
    }
  }
}"#;
const PULL_REQUEST_GRAPHQL_JQ: &str = r#".data.repository.pullRequests as $prs | (["meta", ($prs.totalCount|tostring), ($prs.pageInfo.hasNextPage|tostring), ($prs.pageInfo.endCursor // "")] | @tsv), ($prs.nodes[] | ["pr", (.number|tostring), .title, (.author.login // "ghost"), .state, (.isDraft|tostring), .updatedAt, .url, .baseRefName, .headRefName, (.headRepository.nameWithOwner // ""), (.isCrossRepository|tostring), (.additions|tostring), (.deletions|tostring), (.changedFiles|tostring), .baseRefOid, .headRefOid] | @tsv)"#;
const REPOSITORY_TSV_TEMPLATE: &str = "{{.nameWithOwner}}{{\"\\t\"}}{{.url}}{{\"\\n\"}}";

static TEMPORARY_REPOSITORY_ID: AtomicU64 = AtomicU64::new(0);
static CACHE_WRITE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
    pub name_with_owner: String,
    pub url: String,
    pub remotes: Vec<String>,
}

impl GitHubRepository {
    pub fn selector(&self) -> &str {
        // A canonical URL keeps PR numbers scoped to the selected repository and
        // also carries the GitHub Enterprise hostname.
        &self.url
    }

    pub fn host(&self) -> &str {
        repository_host(&self.url).unwrap_or_default()
    }

    pub fn display_name(&self) -> String {
        let host = self.host();
        if host.is_empty() || host.eq_ignore_ascii_case("github.com") {
            self.name_with_owner.clone()
        } else {
            format!("{host}/{}", self.name_with_owner)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub updated_at: String,
    pub url: String,
    pub base_ref: String,
    pub base_oid: String,
    pub head_ref: String,
    pub head_oid: String,
    pub base_repository: GitHubRepository,
    pub head_repository: Option<String>,
    pub head_remotes: Vec<String>,
    pub is_cross_repository: bool,
    pub additions: usize,
    pub deletions: usize,
    pub changed_files: usize,
}

impl PullRequest {
    pub fn base_label(&self) -> String {
        format!("{}:{}", self.base_repository.display_name(), self.base_ref)
    }

    pub fn head_label(&self) -> String {
        self.head_repository.as_ref().map_or_else(
            || format!("deleted fork:{}", self.head_ref),
            |repository| {
                if repository.eq_ignore_ascii_case(&self.base_repository.name_with_owner) {
                    self.head_ref.clone()
                } else if self
                    .base_repository
                    .host()
                    .eq_ignore_ascii_case("github.com")
                {
                    format!("{repository}:{}", self.head_ref)
                } else {
                    format!(
                        "{}/{repository}:{}",
                        self.base_repository.host(),
                        self.head_ref
                    )
                }
            },
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestBatch {
    pub repositories: Vec<GitHubRepository>,
    pub selected_repository: Option<GitHubRepository>,
    pub pull_requests: Vec<PullRequest>,
    pub warnings: Vec<String>,
    pub total_count: usize,
    pub fetched_count: usize,
    pub has_next_batch: bool,
    pub next_cursor: Option<String>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestSnapshot {
    pub repositories: Vec<GitHubRepository>,
    pub selected_repository: Option<GitHubRepository>,
    pub pull_requests: Vec<PullRequest>,
    pub warnings: Vec<String>,
    pub exact_number: Option<u64>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteUrl {
    remote: String,
    url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheDisposition {
    Network,
    Fresh,
    Stale,
}

struct GhResponse {
    data: Vec<u8>,
    disposition: CacheDisposition,
}

impl Repository {
    pub fn pull_request_batch(
        &self,
        known_repositories: &[GitHubRepository],
        selected_repository: Option<&GitHubRepository>,
        cursor: Option<&str>,
        refresh: bool,
    ) -> Result<PullRequestBatch> {
        let (repositories, mut warnings) = if known_repositories.is_empty() {
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
                "The previously selected GitHub repository is no longer configured; loading {}",
                repository.display_name()
            ));
        }

        let cursor_key = cursor.unwrap_or("first");
        let response = self.checked_cached_gh(
            &format!(
                "pull-request-batch-v2\n{}\n{cursor_key}",
                repository.url.trim_end_matches('/')
            ),
            PULL_REQUEST_PAGE_CACHE_TTL,
            refresh,
            pull_request_batch_args(&repository, cursor),
            "unable to list pull requests",
        )?;
        let parsed = parse_pull_request_batch(&response.data, &repository, &repositories)
            .context("unable to parse GitHub pull-request batch")?;
        if response.disposition == CacheDisposition::Stale {
            warnings.push(format!(
                "GitHub is unavailable; showing stale cached pull requests for {}",
                repository.display_name()
            ));
        }

        Ok(PullRequestBatch {
            repositories,
            selected_repository: Some(repository),
            pull_requests: parsed.pull_requests,
            warnings,
            total_count: parsed.total_count.min(MAX_PULL_REQUESTS),
            fetched_count: parsed.fetched_count,
            has_next_batch: parsed.has_next_batch && parsed.fetched_count < MAX_PULL_REQUESTS,
            next_cursor: parsed.next_cursor,
            from_cache: response.disposition != CacheDisposition::Network,
        })
    }

    pub fn pull_request_lookup(
        &self,
        repositories: &[GitHubRepository],
        repository: &GitHubRepository,
        number: u64,
        refresh: bool,
    ) -> Result<PullRequestSnapshot> {
        if number == 0 {
            bail!("Pull-request numbers must be positive integers");
        }
        let response = self.pull_request_metadata(repository, repositories, number, refresh)?;
        let mut warnings = Vec::new();
        if response.1 == CacheDisposition::Stale {
            warnings.push(format!(
                "GitHub is unavailable; showing stale cached metadata for #{}",
                number
            ));
        }
        Ok(PullRequestSnapshot {
            repositories: repositories.to_vec(),
            selected_repository: Some(repository.clone()),
            pull_requests: vec![response.0],
            warnings,
            exact_number: Some(number),
            from_cache: response.1 != CacheDisposition::Network,
        })
    }

    pub fn pull_request_diff(
        &self,
        pull_request: &PullRequest,
        file_page: usize,
        file_page_size: usize,
    ) -> Result<DiffDocument> {
        // Page rows intentionally contain only inexpensive list metadata. Enrich the
        // selected PR once (normally from the short-lived cache) before fetching Git
        // objects. If GitHub is temporarily unavailable, the list's immutable OIDs
        // and refs are still sufficient for the isolated fetch.
        let mut detailed = self
            .pull_request_metadata(
                &pull_request.base_repository,
                std::slice::from_ref(&pull_request.base_repository),
                pull_request.number,
                false,
            )
            .map(|(pull_request, _)| pull_request)
            .unwrap_or_else(|_| pull_request.clone());
        if !same_pull_request_oids(&detailed, pull_request) {
            detailed = self
                .pull_request_metadata(
                    &pull_request.base_repository,
                    std::slice::from_ref(&pull_request.base_repository),
                    pull_request.number,
                    true,
                )
                .map(|(pull_request, _)| pull_request)
                .ok()
                .filter(|metadata| same_pull_request_oids(metadata, pull_request))
                .unwrap_or_else(|| pull_request.clone());
        }
        if detailed.head_remotes.is_empty() {
            detailed.head_remotes.clone_from(&pull_request.head_remotes);
        }
        self.local_pull_request_diff(&detailed, file_page, file_page_size)
    }

    fn pull_request_metadata(
        &self,
        repository: &GitHubRepository,
        repositories: &[GitHubRepository],
        number: u64,
        refresh: bool,
    ) -> Result<(PullRequest, CacheDisposition)> {
        let response = self.checked_cached_gh(
            &format!(
                "pull-request\n{}\n{number}",
                repository.url.trim_end_matches('/')
            ),
            PULL_REQUEST_CACHE_TTL,
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

    fn local_pull_request_diff(
        &self,
        pull_request: &PullRequest,
        requested_file_page: usize,
        requested_file_page_size: usize,
    ) -> Result<DiffDocument> {
        let temporary = TemporaryBareRepository::new()?;
        let (merge_base, head) = fetch_pull_request(&temporary.path, pull_request)?;
        let (paths, paths_truncated) = changed_paths(&temporary.path, &merge_base, &head)?;
        let file_page_size =
            requested_file_page_size.clamp(1, DEFAULT_PULL_REQUEST_DIFF_PAGE_SIZE.max(50));
        let page_count = if paths.is_empty() {
            1
        } else {
            paths.len().div_ceil(file_page_size)
        };
        let file_page = requested_file_page.max(1).min(page_count);
        let offset = (file_page - 1).saturating_mul(file_page_size);
        let selected_paths = paths
            .iter()
            .skip(offset)
            .take(file_page_size)
            .cloned()
            .collect::<Vec<_>>();
        let has_previous = file_page > 1;
        let has_next = offset.saturating_add(selected_paths.len()) < paths.len();
        let total_files = if paths_truncated {
            pull_request.changed_files.max(paths.len())
        } else {
            paths.len()
        };

        let (patch, output_truncated) = if selected_paths.is_empty() {
            (Vec::new(), false)
        } else {
            diff_selected_paths(&temporary.path, &merge_base, &head, &selected_paths)?
        };
        let truncated = output_truncated || paths_truncated;
        Ok(pull_request_document(
            &patch,
            pull_request,
            truncated,
            file_page,
            file_page_size,
            selected_paths.len(),
            total_files,
            has_previous,
            has_next,
        ))
    }

    fn github_repositories(&self, refresh: bool) -> Result<(Vec<GitHubRepository>, Vec<String>)> {
        let (remote_urls, mut warnings) = self.remote_urls()?;
        let grouped_remote_urls = group_remote_urls(&remote_urls);
        let mut repositories = BTreeMap::new();

        for (index, (url, remotes)) in grouped_remote_urls.iter().take(MAX_REMOTE_URLS).enumerate()
        {
            match self.resolve_github_repository(Some(url), refresh) {
                Ok((repository, disposition)) => {
                    if disposition == CacheDisposition::Stale {
                        warnings.push(format!(
                            "Using stale cached GitHub identity for remote{} {}",
                            if remotes.len() == 1 { "" } else { "s" },
                            remotes.join(", ")
                        ));
                    }
                    if remotes.is_empty() {
                        merge_repository(&mut repositories, repository, None);
                    } else {
                        for remote in remotes {
                            merge_repository(&mut repositories, repository.clone(), Some(remote));
                        }
                    }
                }
                Err(error) => warnings.push(format!(
                    "remote{} `{}` {} not available through gh: {error}",
                    if remotes.len() == 1 { "" } else { "s" },
                    remotes.join(", "),
                    if remotes.len() == 1 { "is" } else { "are" }
                )),
            }
            if repositories.len() >= MAX_GITHUB_REPOSITORIES {
                if index + 1 < grouped_remote_urls.len().min(MAX_REMOTE_URLS) {
                    warnings.push(format!(
                        "Only the first {MAX_GITHUB_REPOSITORIES} distinct GitHub repositories were loaded"
                    ));
                }
                break;
            }
        }
        if grouped_remote_urls.len() > MAX_REMOTE_URLS {
            warnings.push(format!(
                "Only the first {MAX_REMOTE_URLS} distinct fetch/push remote URLs were inspected"
            ));
        }

        if repositories.is_empty() {
            match self.resolve_github_repository(None, refresh) {
                Ok((repository, disposition)) => {
                    if disposition == CacheDisposition::Stale {
                        warnings.push("Using a stale cached inferred GitHub repository".to_owned());
                    }
                    merge_repository(&mut repositories, repository, None);
                }
                Err(error) => {
                    let remote_hint = if remote_urls.is_empty() {
                        "No Git remotes are configured".to_owned()
                    } else {
                        "No configured fetch or push remote resolves to GitHub".to_owned()
                    };
                    bail!("{remote_hint}; GitHub CLI could not infer a repository: {error}");
                }
            }
        }

        let mut repositories: Vec<_> = repositories.into_values().collect();
        for repository in &mut repositories {
            repository.remotes.sort();
            repository.remotes.dedup();
        }
        repositories.sort_by_key(|repository| {
            (
                !repository.remotes.iter().any(|remote| remote == "origin"),
                repository.display_name().to_lowercase(),
            )
        });
        Ok((repositories, warnings))
    }

    fn remote_urls(&self) -> Result<(Vec<RemoteUrl>, Vec<String>)> {
        let output = self.checked([OsString::from("remote")])?;
        let mut urls = BTreeSet::new();
        let mut warnings = Vec::new();

        let mut remote_count = 0;
        'remotes: for record in output.split(|byte| *byte == b'\n') {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            if remote_count >= MAX_GIT_REMOTES {
                warnings.push(format!(
                    "Only the first {MAX_GIT_REMOTES} Git remotes were inspected"
                ));
                break;
            }
            remote_count += 1;
            let remote = text(record);
            for push in [false, true] {
                let mut args = vec![OsString::from("remote"), OsString::from("get-url")];
                if push {
                    args.push(OsString::from("--push"));
                }
                args.push(OsString::from("--all"));
                args.push(OsString::from(&remote));
                let remote_output = self.run(args)?;
                if !remote_output.status.success() {
                    if !push {
                        warnings.push(format!("Unable to read URL for remote `{remote}`"));
                    }
                    continue;
                }
                for url in remote_output.stdout.split(|byte| *byte == b'\n') {
                    let url = trim_ascii(url);
                    if url.is_empty() {
                        continue;
                    }
                    let entry = (remote.clone(), text(url));
                    if urls.len() >= MAX_REMOTE_URL_ENTRIES && !urls.contains(&entry) {
                        warnings.push(format!(
                            "Only the first {MAX_REMOTE_URL_ENTRIES} configured fetch/push URL entries were inspected"
                        ));
                        break 'remotes;
                    }
                    urls.insert(entry);
                }
            }
        }

        Ok((
            urls.into_iter()
                .map(|(remote, url)| RemoteUrl { remote, url })
                .collect(),
            warnings,
        ))
    }

    fn resolve_github_repository(
        &self,
        url: Option<&str>,
        refresh: bool,
    ) -> Result<(GitHubRepository, CacheDisposition)> {
        let mut args = vec![OsString::from("repo"), OsString::from("view")];
        if let Some(url) = url {
            args.push(OsString::from(url));
        }
        args.extend([
            OsString::from("--json"),
            OsString::from("nameWithOwner,url"),
            OsString::from("--template"),
            OsString::from(REPOSITORY_TSV_TEMPLATE),
        ]);
        let identity = url.map(remote_url_for_gh).unwrap_or_else(|| {
            format!(
                "inferred\n{}\n{}",
                self.root.display(),
                env::var("GH_REPO").unwrap_or_default()
            )
        });
        let key = format!("repository\n{identity}");
        let response = self.checked_cached_gh(
            &key,
            REPOSITORY_CACHE_TTL,
            refresh,
            args,
            "gh repo view failed",
        )?;
        let record = trim_ascii(&response.data);
        let fields = parse_tsv_record(record, 2).context("invalid gh repo view output")?;
        if fields[0].trim().is_empty() || fields[1].trim().is_empty() {
            bail!("gh repo view returned an incomplete repository identity");
        }
        Ok((
            GitHubRepository {
                name_with_owner: fields[0].clone(),
                url: fields[1].trim_end_matches('/').to_owned(),
                remotes: Vec::new(),
            },
            response.disposition,
        ))
    }

    fn checked_cached_gh<I, S>(
        &self,
        cache_key: &str,
        ttl: Duration,
        refresh: bool,
        args: I,
        error_context: &str,
    ) -> Result<GhResponse>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cache = CacheStore::discover();
        let cached = cache.as_ref().and_then(|cache| cache.read(cache_key));
        if !refresh {
            if let Some(entry) = cached.as_ref() {
                if entry.age <= ttl {
                    return Ok(GhResponse {
                        data: entry.data.clone(),
                        disposition: CacheDisposition::Fresh,
                    });
                }
            }
        }

        let output = match self.run_gh(args) {
            Ok(output) => output,
            Err(error) => {
                if let Some(entry) = cached.as_ref() {
                    return Ok(GhResponse {
                        data: entry.data.clone(),
                        disposition: CacheDisposition::Stale,
                    });
                }
                return Err(error);
            }
        };
        if output.status.success() && !output.stdout_truncated {
            if let Some(cache) = cache.as_ref() {
                let _ = cache.write(cache_key, &output.stdout);
            }
            return Ok(GhResponse {
                data: output.stdout,
                disposition: CacheDisposition::Network,
            });
        }
        if let Some(entry) = cached {
            return Ok(GhResponse {
                data: entry.data,
                disposition: CacheDisposition::Stale,
            });
        }
        if output.stdout_truncated {
            bail!("{error_context}: GitHub CLI output exceeded the metadata limit");
        }
        bail!("{}", bounded_command_error(error_context, &output));
    }

    fn run_gh<I, S>(&self, args: I) -> Result<BoundedOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("gh");
        command
            .current_dir(&self.root)
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("NO_COLOR", "1");
        run_bounded_command(&mut command, MAX_GH_METADATA_BYTES, MAX_GH_ERROR_BYTES).with_context(
            || {
                format!(
                    "failed to execute GitHub CLI (`gh`) in {}; install it and run `gh auth login`",
                    self.root.display()
                )
            },
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn pull_request_document(
    output: &[u8],
    pull_request: &PullRequest,
    truncated: bool,
    file_page: usize,
    file_page_size: usize,
    displayed_files: usize,
    total_files: usize,
    has_previous_file_page: bool,
    has_next_file_page: bool,
) -> DiffDocument {
    let range_start = if displayed_files == 0 {
        0
    } else {
        (file_page - 1)
            .saturating_mul(file_page_size)
            .saturating_add(1)
    };
    let range_end = range_start.saturating_add(displayed_files.saturating_sub(1));
    let mut document = parse_diff(
        output,
        format!(
            "PR #{} — {}  ·  {} → {}  ·  files {}–{} of {}",
            pull_request.number,
            pull_request.title,
            pull_request.head_label(),
            pull_request.base_label(),
            range_start,
            range_end,
            total_files,
        ),
        None,
        truncated,
    );
    document.truncated |= truncated;
    document.pull_request_details = Some(PullRequestDetails {
        number: pull_request.number,
        title: pull_request.title.clone(),
        author: pull_request.author.clone(),
        state: pull_request.state.clone(),
        is_draft: pull_request.is_draft,
        updated_at: pull_request.updated_at.clone(),
        url: pull_request.url.clone(),
        base_repository: pull_request.base_repository.display_name(),
        base_ref: pull_request.base_ref.clone(),
        base_remotes: pull_request.base_repository.remotes.clone(),
        head_repository: pull_request.head_repository.as_ref().map(|repository| {
            if pull_request
                .base_repository
                .host()
                .eq_ignore_ascii_case("github.com")
                || pull_request.base_repository.host().is_empty()
            {
                repository.clone()
            } else {
                format!("{}/{repository}", pull_request.base_repository.host())
            }
        }),
        head_ref: pull_request.head_ref.clone(),
        head_remotes: pull_request.head_remotes.clone(),
        is_cross_repository: pull_request.is_cross_repository,
        changed_files: pull_request.changed_files.max(total_files),
        additions: pull_request.additions,
        deletions: pull_request.deletions,
        file_page,
        file_page_size,
        displayed_files,
        total_files,
        has_previous_file_page,
        has_next_file_page,
    });
    document
}

fn pull_request_batch_args(repository: &GitHubRepository, cursor: Option<&str>) -> Vec<OsString> {
    let mut args = vec![OsString::from("api"), OsString::from("graphql")];
    if !repository.host().is_empty() {
        args.extend([
            OsString::from("--hostname"),
            OsString::from(repository.host()),
        ]);
    }
    let (owner, name) = repository
        .name_with_owner
        .split_once('/')
        .unwrap_or((repository.name_with_owner.as_str(), ""));
    args.extend([
        OsString::from("-f"),
        OsString::from(format!("query={PULL_REQUEST_GRAPHQL_QUERY}")),
        OsString::from("-F"),
        OsString::from(format!("owner={owner}")),
        OsString::from("-F"),
        OsString::from(format!("name={name}")),
    ]);
    if let Some(cursor) = cursor {
        args.extend([
            OsString::from("-F"),
            OsString::from(format!("endCursor={cursor}")),
        ]);
    }
    args.extend([
        OsString::from("--jq"),
        OsString::from(PULL_REQUEST_GRAPHQL_JQ),
    ]);
    args
}

fn pull_request_view_args(repository: &GitHubRepository, number: u64) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("view"),
        OsString::from(number.to_string()),
        OsString::from("--repo"),
        OsString::from(repository.selector()),
        OsString::from("--json"),
        OsString::from(PULL_REQUEST_FIELDS),
        OsString::from("--jq"),
        OsString::from(PULL_REQUEST_VIEW_TSV_JQ),
    ]
}

struct ParsedPullRequestBatch {
    pull_requests: Vec<PullRequest>,
    total_count: usize,
    fetched_count: usize,
    has_next_batch: bool,
    next_cursor: Option<String>,
}

fn parse_pull_request_batch(
    output: &[u8],
    base_repository: &GitHubRepository,
    repositories: &[GitHubRepository],
) -> Result<ParsedPullRequestBatch> {
    let mut records = output.split(|byte| *byte == b'\n');
    let metadata = records
        .next()
        .filter(|record| !record.is_empty())
        .context("GitHub returned no pull-request progress metadata")?;
    let fields = parse_tsv_record(metadata, 4).context("invalid pull-request batch metadata")?;
    if fields[0] != "meta" {
        bail!("GitHub pull-request batch did not begin with metadata");
    }
    let total_count = parse_field(&fields[1], "pull-request total")?;
    let has_next_batch = parse_field(&fields[2], "pull-request pagination state")?;
    let next_cursor = (!fields[3].is_empty()).then(|| fields[3].clone());
    let mut requests = Vec::new();
    for record in records {
        if record.is_empty() {
            continue;
        }
        let fields = parse_tsv_record(record, 17).context("invalid pull-request batch record")?;
        if fields[0] != "pr" {
            bail!("unexpected record in pull-request batch");
        }
        requests.push(parse_pull_request_fields(
            &fields[1..],
            base_repository,
            repositories,
        )?);
    }
    Ok(ParsedPullRequestBatch {
        fetched_count: requests.len(),
        pull_requests: requests,
        total_count,
        has_next_batch,
        next_cursor,
    })
}

fn parse_pull_requests(
    output: &[u8],
    base_repository: &GitHubRepository,
    repositories: &[GitHubRepository],
) -> Result<Vec<PullRequest>> {
    let mut pull_requests = Vec::new();
    for (index, record) in output.split(|byte| *byte == b'\n').enumerate() {
        if record.is_empty() {
            continue;
        }
        let fields = parse_tsv_record(record, 16)
            .with_context(|| format!("invalid pull-request record {}", index + 1))?;
        pull_requests.push(parse_pull_request_fields(
            &fields,
            base_repository,
            repositories,
        )?);
    }
    Ok(pull_requests)
}

fn parse_pull_request_fields(
    fields: &[String],
    base_repository: &GitHubRepository,
    repositories: &[GitHubRepository],
) -> Result<PullRequest> {
    if fields.len() != 16 {
        bail!("expected 16 pull-request fields, received {}", fields.len());
    }
    let head_repository = (!fields[9].is_empty()).then(|| fields[9].clone());
    let head_remotes = head_repository
        .as_deref()
        .map(|name| matching_remotes(repositories, base_repository.host(), name))
        .unwrap_or_default();
    Ok(PullRequest {
        number: parse_field(&fields[0], "number")?,
        title: bounded_text(&fields[1], MAX_PULL_REQUEST_TITLE_BYTES),
        author: fields[2].clone(),
        state: fields[3].to_ascii_uppercase(),
        is_draft: parse_field(&fields[4], "draft state")?,
        updated_at: fields[5].clone(),
        url: fields[6].clone(),
        base_ref: fields[7].clone(),
        head_ref: fields[8].clone(),
        base_repository: base_repository.clone(),
        head_repository,
        head_remotes,
        is_cross_repository: parse_field(&fields[10], "cross-repository state")?,
        additions: parse_field(&fields[11], "addition count")?,
        deletions: parse_field(&fields[12], "deletion count")?,
        changed_files: parse_field(&fields[13], "changed-file count")?,
        base_oid: fields[14].clone(),
        head_oid: fields[15].clone(),
    })
}

fn bounded_text(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}…", &value[..end])
}

fn parse_tsv_record(record: &[u8], expected_fields: usize) -> Result<Vec<String>> {
    let record = record.strip_suffix(b"\r").unwrap_or(record);
    let fields: Vec<_> = record.split(|byte| *byte == b'\t').collect();
    if fields.len() != expected_fields {
        bail!(
            "expected {expected_fields} tab-separated fields, received {}",
            fields.len()
        );
    }
    Ok(fields
        .into_iter()
        .map(text)
        .map(|field| unescape_tsv(&field))
        .collect())
}

fn unescape_tsv(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('t') => output.push('\t'),
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('\\') => output.push('\\'),
            Some(other) => {
                output.push('\\');
                output.push(other);
            }
            None => output.push('\\'),
        }
    }
    output
}

fn parse_field<T>(value: &str, label: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid {label} `{value}`: {error}"))
}

fn same_pull_request_oids(left: &PullRequest, right: &PullRequest) -> bool {
    (left.base_oid.is_empty()
        || right.base_oid.is_empty()
        || left.base_oid.eq_ignore_ascii_case(&right.base_oid))
        && (left.head_oid.is_empty()
            || right.head_oid.is_empty()
            || left.head_oid.eq_ignore_ascii_case(&right.head_oid))
}

fn select_repository<'a>(
    repositories: &'a [GitHubRepository],
    selected: Option<&GitHubRepository>,
) -> Option<&'a GitHubRepository> {
    selected
        .and_then(|selected| {
            repositories.iter().find(|repository| {
                repository
                    .url
                    .trim_end_matches('/')
                    .eq_ignore_ascii_case(selected.url.trim_end_matches('/'))
            })
        })
        .or_else(|| repositories.first())
}

fn remote_url_for_gh(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        let authority = authority.split(['?', '#']).next().unwrap_or(authority);
        let path = path.split(['?', '#']).next().unwrap_or(path);
        return if path.is_empty() {
            format!("{scheme}://{authority}")
        } else {
            format!("{scheme}://{authority}/{path}")
        };
    }

    if let Some((_, target)) = url.rsplit_once('@') {
        if let Some((host, path)) = target.split_once(':') {
            if !host.is_empty() && !path.is_empty() {
                return format!("ssh://{host}/{path}");
            }
        }
    }
    url.to_owned()
}

fn group_remote_urls(remote_urls: &[RemoteUrl]) -> Vec<(String, Vec<String>)> {
    let mut grouped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for remote_url in remote_urls {
        grouped
            .entry(remote_url_for_gh(&remote_url.url))
            .or_default()
            .insert(remote_url.remote.clone());
    }
    grouped
        .into_iter()
        .map(|(url, remotes)| (url, remotes.into_iter().collect()))
        .collect()
}

fn matching_remotes(
    repositories: &[GitHubRepository],
    host: &str,
    name_with_owner: &str,
) -> Vec<String> {
    repositories
        .iter()
        .find(|repository| {
            repository.host().eq_ignore_ascii_case(host)
                && repository
                    .name_with_owner
                    .eq_ignore_ascii_case(name_with_owner)
        })
        .map(|repository| repository.remotes.clone())
        .unwrap_or_default()
}

fn merge_repository(
    repositories: &mut BTreeMap<String, GitHubRepository>,
    mut repository: GitHubRepository,
    remote: Option<&str>,
) {
    let key = repository.url.trim_end_matches('/').to_lowercase();
    let entry = repositories.entry(key).or_insert_with(|| {
        repository.url = repository.url.trim_end_matches('/').to_owned();
        repository
    });
    if let Some(remote) = remote {
        if !entry.remotes.iter().any(|existing| existing == remote) {
            entry.remotes.push(remote.to_owned());
        }
    }
}

fn repository_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    rest.split('/').next().filter(|host| !host.is_empty())
}

struct TemporaryBareRepository {
    path: PathBuf,
}

impl TemporaryBareRepository {
    fn new() -> Result<Self> {
        let preferred_parent = cache_root().map(|root| root.join("tmp"));
        let parent = match preferred_parent {
            Some(parent) if create_private_directory(&parent).is_ok() => {
                remove_stale_temporary_repositories(&parent);
                parent
            }
            _ => env::temp_dir(),
        };
        for _ in 0..16 {
            let id = TEMPORARY_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("pr-{}-{id}.git", std::process::id()));
            if path.exists() {
                continue;
            }
            let mut command = Command::new("git");
            command
                .args(["init", "--bare", "--quiet"])
                .arg(&path)
                .env("LC_ALL", "C")
                .env("GIT_TERMINAL_PROMPT", "0");
            let output = run_bounded_command(&mut command, 64 * 1024, 64 * 1024)
                .context("failed to initialize a disposable Git repository")?;
            if !output.status.success() {
                bail!(
                    "{}",
                    bounded_command_error(
                        "unable to initialize disposable Git repository",
                        &output
                    )
                );
            }
            return Ok(Self { path });
        }
        bail!("unable to allocate a unique disposable Git repository")
    }
}

impl Drop for TemporaryBareRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn remove_stale_temporary_repositories(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.filter_map(Result::ok).take(256) {
        let path = entry.path();
        let is_quinjet_pr = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("pr-") && name.ends_with(".git"));
        if !is_quinjet_pr {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= TEMPORARY_REPOSITORY_MAX_AGE);
        if stale {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn fetch_pull_request(temporary: &Path, pull_request: &PullRequest) -> Result<(String, String)> {
    if pull_request.base_ref.is_empty() || pull_request.head_ref.is_empty() {
        bail!("Pull request metadata does not contain complete base/head refs");
    }
    checked_temp_git(
        temporary,
        &[
            OsString::from("remote"),
            OsString::from("add"),
            OsString::from("origin"),
            OsString::from(pull_request.base_repository.selector()),
        ],
        "unable to configure the disposable base remote",
    )?;
    let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
    let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);

    fetch_ref(temporary, "origin", &base_refspec, 64)?;
    let (head_remote, head_refspec) = match fetch_ref(temporary, "origin", &pull_refspec, 64) {
        Ok(()) => ("origin".to_owned(), pull_refspec),
        Err(pull_ref_error) => {
            let Some(head_repository) = pull_request.head_repository.as_deref() else {
                return Err(pull_ref_error).context(
                    "the base repository no longer exposes the PR head and its fork was deleted",
                );
            };
            let head_url = repository_url_for_name(&pull_request.base_repository, head_repository);
            checked_temp_git(
                temporary,
                &[
                    OsString::from("remote"),
                    OsString::from("add"),
                    OsString::from("head"),
                    OsString::from(head_url),
                ],
                "unable to configure the disposable fork remote",
            )?;
            let head_refspec = format!("+refs/heads/{}:refs/quinjet/head", pull_request.head_ref);
            fetch_ref(temporary, "head", &head_refspec, 64).with_context(|| {
                format!(
                    "unable to fetch PR #{} from either the base PR ref or its fork",
                    pull_request.number
                )
            })?;
            ("head".to_owned(), head_refspec)
        }
    };

    for depth in [64_usize, 256, 1_024, 4_096] {
        if depth != 64 {
            fetch_ref(temporary, "origin", &base_refspec, depth)?;
            fetch_ref(temporary, &head_remote, &head_refspec, depth)?;
        }
        let base =
            preferred_fetched_commit(temporary, &pull_request.base_oid, "refs/quinjet/base")?;
        let head =
            preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
        if let Some(merge_base) = try_merge_base(temporary, &base, &head)? {
            return Ok((merge_base, head));
        }
    }
    bail!(
        "Unable to find the PR merge base within 4,096 commits; refusing an unbounded history fetch"
    )
}

fn repository_url_for_name(base: &GitHubRepository, name_with_owner: &str) -> String {
    if let Some((scheme, rest)) = base.url.split_once("://") {
        let host = rest.split('/').next().unwrap_or_default();
        if !host.is_empty() {
            return format!("{scheme}://{host}/{name_with_owner}");
        }
    }
    name_with_owner.to_owned()
}

fn fetch_ref(temporary: &Path, remote: &str, refspec: &str, depth: usize) -> Result<()> {
    let args = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from("--filter=blob:none"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if output.status.success() {
        return Ok(());
    }

    // Older GitHub Enterprise or local test remotes may not support partial clone.
    let fallback = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &fallback, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() {
        bail!(
            "{}",
            bounded_command_error("unable to fetch a pull-request ref", &output)
        );
    }
    Ok(())
}

fn preferred_fetched_commit(temporary: &Path, oid: &str, fallback: &str) -> Result<String> {
    if (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let args = [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(format!("{oid}^{{commit}}")),
        ];
        let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
        if output.status.success() {
            let resolved = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
            if !resolved.is_empty() {
                return Ok(resolved);
            }
        }
    }
    Ok(fallback.to_owned())
}

fn try_merge_base(temporary: &Path, base: &str, head: &str) -> Result<Option<String>> {
    let args = [
        OsString::from("merge-base"),
        OsString::from(base),
        OsString::from(head),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
    if !output.status.success() {
        return Ok(None);
    }
    let merge_base = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
    Ok((!merge_base.is_empty()).then_some(merge_base))
}

fn changed_paths(temporary: &Path, merge_base: &str, head: &str) -> Result<(Vec<OsString>, bool)> {
    let args = [
        OsString::from("diff"),
        OsString::from("--name-only"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    let output = run_temp_git(temporary, &args, MAX_PR_PATH_BYTES, 128 * 1024)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to enumerate pull-request files", &output)
        );
    }
    let mut truncated = output.stdout_truncated;
    let complete_output = if output.stdout_truncated && !output.stdout.ends_with(&[0]) {
        output
            .stdout
            .iter()
            .rposition(|byte| *byte == 0)
            .map_or(&[][..], |index| &output.stdout[..=index])
    } else {
        &output.stdout
    };
    let mut paths = Vec::new();
    for path in complete_output.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        if paths.len() >= MAX_PR_PATHS {
            truncated = true;
            break;
        }
        paths.push(OsString::from(String::from_utf8_lossy(path).into_owned()));
    }
    Ok((paths, truncated))
}

fn diff_selected_paths(
    temporary: &Path,
    merge_base: &str,
    head: &str,
    paths: &[OsString],
) -> Result<(Vec<u8>, bool)> {
    let mut args = vec![
        OsString::from("diff"),
        OsString::from("--no-color"),
        OsString::from("--no-ext-diff"),
        OsString::from("--find-renames"),
        OsString::from("--patch"),
        OsString::from("--unified=3"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    args.extend(paths.iter().cloned());
    let output = run_temp_git(temporary, &args, MAX_DIFF_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to generate the local pull-request diff", &output)
        );
    }
    let mut patch = output.stdout;
    if output.stdout_truncated {
        while patch.last().is_some_and(|byte| *byte != b'\n') {
            patch.pop();
        }
    }
    Ok((patch, output.stdout_truncated))
}

fn checked_temp_git(temporary: &Path, args: &[OsString], context: &str) -> Result<Vec<u8>> {
    let output = run_temp_git(temporary, args, MAX_GH_METADATA_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() || output.stdout_truncated {
        bail!("{}", bounded_command_error(context, &output));
    }
    Ok(output.stdout)
}

fn run_temp_git(
    temporary: &Path,
    args: &[OsString],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(temporary)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    run_bounded_command(&mut command, stdout_limit, stderr_limit)
        .with_context(|| format!("failed to execute Git in {}", temporary.display()))
}

struct BoundedOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
}

fn run_bounded_command(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .context("child process did not expose stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("child process did not expose stderr")?;
    let stderr_reader = thread::spawn(move || read_and_drain(stderr, stderr_limit));

    let mut collected = Vec::with_capacity(stdout_limit.min(64 * 1024));
    let mut buffer = [0_u8; 64 * 1024];
    let mut truncated = false;
    loop {
        let read = match stdout.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stderr_reader.join();
                return Err(error.into());
            }
        };
        let remaining = stdout_limit.saturating_sub(collected.len());
        if read > remaining {
            collected.extend_from_slice(&buffer[..remaining]);
            truncated = true;
            let _ = child.kill();
            break;
        }
        collected.extend_from_slice(&buffer[..read]);
    }
    drop(stdout);
    let status = child.wait()?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader thread panicked"))??;
    Ok(BoundedOutput {
        status,
        stdout: collected,
        stderr,
        stdout_truncated: truncated,
    })
}

fn read_and_drain(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut collected = Vec::with_capacity(limit.min(32 * 1024));
    let mut buffer = [0_u8; 32 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let remaining = limit.saturating_sub(collected.len());
        collected.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(collected)
}

fn bounded_command_error(context: &str, output: &BoundedOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if !stderr.is_empty() { stderr } else { stdout };
    if details.is_empty() {
        format!("{context} (exit status {})", output.status)
    } else {
        format!("{context}: {details}")
    }
}

struct CacheEntry {
    data: Vec<u8>,
    age: Duration,
}

struct CacheStore {
    root: PathBuf,
}

impl CacheStore {
    fn discover() -> Option<Self> {
        cache_root().map(|root| Self {
            root: root.join("github"),
        })
    }

    #[cfg(test)]
    fn at(root: PathBuf) -> Self {
        Self { root }
    }

    fn read(&self, key: &str) -> Option<CacheEntry> {
        let path = self.path(key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > MAX_GH_METADATA_BYTES as u64 + CACHE_MAGIC.len() as u64 {
            let _ = fs::remove_file(path);
            return None;
        }
        let mut data = fs::read(path).ok()?;
        if !data.starts_with(CACHE_MAGIC) {
            return None;
        }
        data.drain(..CACHE_MAGIC.len());
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .unwrap_or_default();
        Some(CacheEntry { data, age })
    }

    fn write(&self, key: &str, data: &[u8]) -> Result<()> {
        if data.len() > MAX_GH_METADATA_BYTES {
            return Ok(());
        }
        create_private_directory(&self.root)?;
        let destination = self.path(key);
        let id = CACHE_WRITE_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = self
            .root
            .join(format!(".write-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(CACHE_MAGIC)?;
        file.write_all(data)?;
        file.flush()?;
        drop(file);
        if destination.exists() {
            let _ = fs::remove_file(&destination);
        }
        if let Err(error) = fs::rename(&temporary, &destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        self.prune();
        Ok(())
    }

    fn path(&self, key: &str) -> PathBuf {
        let (left, right) = stable_cache_hash(key.as_bytes());
        self.root.join(format!("{left:016x}{right:016x}.cache"))
    }

    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return;
        };
        let mut files = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(OsStr::to_str) != Some("cache") {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                Some((
                    metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    metadata.len(),
                    path,
                ))
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|(modified, _, _)| *modified);
        let mut total = files.iter().map(|(_, bytes, _)| *bytes).sum::<u64>();
        let mut count = files.len();
        for (_, bytes, path) in files {
            if count <= MAX_CACHE_ENTRIES && total <= MAX_CACHE_BYTES {
                break;
            }
            if fs::remove_file(path).is_ok() {
                count = count.saturating_sub(1);
                total = total.saturating_sub(bytes);
            }
        }
    }
}

fn cache_root() -> Option<PathBuf> {
    if let Some(path) = env::var_os("QUINJET_CACHE_DIR").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        if let Some(path) = env::var_os("LOCALAPPDATA").filter(|path| !path.is_empty()) {
            return Some(PathBuf::from(path).join("quinjet").join("cache"));
        }
    }
    if let Some(path) = env::var_os("XDG_CACHE_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path).join("quinjet"));
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = env::var_os("HOME").filter(|path| !path.is_empty()) {
            return Some(
                PathBuf::from(path)
                    .join("Library")
                    .join("Caches")
                    .join("quinjet"),
            );
        }
    }
    env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(path).join(".cache").join("quinjet"))
}

fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn stable_cache_hash(value: &[u8]) -> (u64, u64) {
    let mut left = 0xcbf29ce484222325_u64;
    let mut right = 0x84222325cbf29ce4_u64;
    for byte in value {
        left ^= u64::from(*byte);
        left = left.wrapping_mul(0x100000001b3);
        right ^= u64::from(*byte).rotate_left(1);
        right = right.wrapping_mul(0x100000001b3).rotate_left(5);
    }
    (left, right)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .env("LC_ALL", "C")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_directory(label: &str) -> TestDirectory {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "quinjet-github-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TestDirectory(path)
    }

    fn initialized_repository() -> TestDirectory {
        let directory = test_directory("repo");
        directory.git(&["init", "--initial-branch=main"]);
        directory.git(&["config", "user.name", "Quinjet Test"]);
        directory.git(&["config", "user.email", "quinjet@example.com"]);
        fs::write(directory.0.join("README.md"), "base\n").unwrap();
        directory.git(&["add", "README.md"]);
        directory.git(&["commit", "--message=base"]);
        directory
    }

    fn repository(name: &str, url: &str, remotes: &[&str]) -> GitHubRepository {
        GitHubRepository {
            name_with_owner: name.to_owned(),
            url: url.to_owned(),
            remotes: remotes.iter().map(|remote| (*remote).to_owned()).collect(),
        }
    }

    fn pull_request(base: GitHubRepository, number: u64) -> PullRequest {
        PullRequest {
            number,
            title: "Ship the rocket".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            updated_at: "2026-08-13T12:00:00Z".to_owned(),
            url: format!("{}/pull/{number}", base.url),
            base_ref: "main".to_owned(),
            base_oid: String::new(),
            head_ref: "feature/rocket".to_owned(),
            head_oid: String::new(),
            base_repository: base,
            head_repository: Some("octocat/widget".to_owned()),
            head_remotes: vec!["origin".to_owned()],
            is_cross_repository: true,
            additions: 1,
            deletions: 0,
            changed_files: 1,
        }
    }

    #[test]
    fn discovers_distinct_fetch_and_push_repositories_for_each_remote() {
        let directory = initialized_repository();
        directory.git(&[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ]);
        directory.git(&[
            "remote",
            "set-url",
            "--push",
            "origin",
            "git@github.com:octocat/widget.git",
        ]);
        directory.git(&[
            "remote",
            "add",
            "upstream",
            "https://github.com/acme/widget.git",
        ]);
        let repository = Repository {
            root: directory.0.clone(),
        };

        let (urls, warnings) = repository.remote_urls().unwrap();

        assert!(warnings.is_empty());
        assert_eq!(urls.len(), 3);
        assert!(urls.iter().any(|entry| {
            entry.remote == "origin" && entry.url == "git@github.com:octocat/widget.git"
        }));
        assert!(urls.iter().any(|entry| entry.remote == "upstream"));
    }

    #[test]
    fn decodes_gh_tsv_escapes_without_corrupting_literal_backslashes() {
        assert_eq!(
            unescape_tsv(r"line one\nline two\tpath\\file\q"),
            "line one\nline two\tpath\\file\\q"
        );
        assert_eq!(
            parse_tsv_record(b"one\ttwo\\tinside\tthree\r", 3).unwrap(),
            vec!["one", "two\tinside", "three"]
        );
    }

    #[test]
    fn strips_credentials_before_passing_remote_urls_to_gh() {
        assert_eq!(
            remote_url_for_gh("https://user:secret@github.com/acme/widget.git?token=secret"),
            "https://github.com/acme/widget.git"
        );
        assert_eq!(
            remote_url_for_gh("ssh://deploy-key@github.example.com/acme/widget.git"),
            "ssh://github.example.com/acme/widget.git"
        );
        assert_eq!(
            remote_url_for_gh("token-user@github.com:acme/widget.git"),
            "ssh://github.com/acme/widget.git"
        );
    }

    #[test]
    fn parses_cross_repository_pull_requests_with_oids() {
        let upstream = repository(
            "acme/widget",
            "https://github.com/acme/widget",
            &["upstream"],
        );
        let fork = repository(
            "octocat/widget",
            "https://github.com/octocat/widget",
            &["origin", "publish"],
        );
        let output = b"42\tShip the rocket\toctocat\tOPEN\ttrue\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t12\t3\t4\tbaseoid\theadid\n";

        let requests = parse_pull_requests(output, &upstream, &[upstream.clone(), fork]).unwrap();

        let request = &requests[0];
        assert_eq!(request.base_label(), "acme/widget:main");
        assert_eq!(request.head_label(), "octocat/widget:feature/rocket");
        assert_eq!(request.head_remotes, vec!["origin", "publish"]);
        assert_eq!(request.base_oid, "baseoid");
        assert_eq!(request.head_oid, "headid");
        assert!(request.is_cross_repository);
    }

    #[test]
    fn deleted_fork_metadata_uses_the_base_repository_pr_ref() {
        let base = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["enterprise"],
        );
        let output = b"7\tOld contribution\tghost\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.example.com/acme/widget/pull/7\ttrunk\tlost-branch\t\tfalse\t0\t0\t1\tbaseoid\theadid\n";

        let request = parse_pull_requests(output, &base, std::slice::from_ref(&base))
            .unwrap()
            .remove(0);

        assert_eq!(request.author, "ghost");
        assert_eq!(request.head_label(), "deleted fork:lost-branch");
        assert!(request.head_repository.is_none());
        assert!(request.head_remotes.is_empty());
        assert_eq!(request.base_repository.host(), "github.example.com");
    }

    #[test]
    fn exact_lookup_command_is_repository_scoped_and_requests_oids() {
        let repository = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["work"],
        );
        let args = pull_request_view_args(&repository, 19);
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            &args[..5],
            &["pr", "view", "19", "--repo", repository.url.as_str()]
        );
        assert!(args.iter().any(|arg| arg.contains("baseRefOid")));
        assert!(args.iter().any(|arg| arg.contains("headRefOid")));
    }

    #[test]
    fn batch_command_is_bounded_cursor_based_and_targets_enterprise_host() {
        let repository = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["work"],
        );
        let args = pull_request_batch_args(&repository, Some("cursor-1"))
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(&args[..2], &["api", "graphql"]);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--hostname", "github.example.com"])
        );
        assert!(args.iter().any(|arg| arg == "owner=acme"));
        assert!(args.iter().any(|arg| arg == "name=widget"));
        assert!(args.iter().any(|arg| arg == "endCursor=cursor-1"));
        assert!(args.iter().any(|arg| arg.contains("first: 50")));
    }

    #[test]
    fn parses_batch_progress_and_all_pr_states() {
        let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
        let output = b"meta\t73\ttrue\tcursor-50\npr\t42\tOpen change\tada\tOPEN\tfalse\tdate\turl\tmain\ttopic\tacme/widget\tfalse\t1\t2\t3\tbase\thead\npr\t41\tMerged change\tgrace\tMERGED\tfalse\tdate\turl2\tmain\ttopic2\tacme/widget\tfalse\t4\t5\t6\tbase2\thead2\n";

        let batch = parse_pull_request_batch(output, &base, std::slice::from_ref(&base)).unwrap();

        assert_eq!(batch.total_count, 73);
        assert_eq!(batch.fetched_count, 2);
        assert!(batch.has_next_batch);
        assert_eq!(batch.next_cursor.as_deref(), Some("cursor-50"));
        assert_eq!(batch.pull_requests[0].state, "OPEN");
        assert_eq!(batch.pull_requests[1].state, "MERGED");
    }

    #[test]
    fn cache_round_trips_private_metadata_and_uses_stable_keys() {
        let directory = test_directory("cache");
        let cache = CacheStore::at(directory.0.clone());
        cache.write("repo\npage 1", b"metadata\n").unwrap();

        let entry = cache.read("repo\npage 1").unwrap();
        assert_eq!(entry.data, b"metadata\n");
        assert!(entry.age < Duration::from_secs(2));
        assert_eq!(cache.path("same"), cache.path("same"));
        assert_ne!(cache.path("same"), cache.path("different"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(cache.path("repo\npage 1"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
    }

    #[test]
    fn disposable_pr_diff_does_not_checkout_or_add_source_refs() {
        let source = initialized_repository();
        let remote = test_directory("remote.git");
        source.git(&["init", "--bare", remote.0.to_str().unwrap()]);
        source.git(&["remote", "add", "test-origin", remote.0.to_str().unwrap()]);
        source.git(&["push", "test-origin", "main:refs/heads/main"]);
        source.git(&["switch", "-c", "feature/rocket"]);
        for index in 0..21 {
            fs::write(
                source.0.join(format!("rocket-{index:02}.txt")),
                format!("launch {index}\n"),
            )
            .unwrap();
        }
        source.git(&["add", "."]);
        source.git(&["commit", "--message=rocket"]);
        source.git(&["push", "test-origin", "feature/rocket:refs/pull/7/head"]);
        source.git(&["switch", "main"]);

        let before_branch = source.git(&["branch", "--show-current"]);
        let before_status = source.git(&["status", "--porcelain"]);
        let before_refs = source.git(&["show-ref"]);
        let git_repository = Repository {
            root: source.0.clone(),
        };
        let mut request = pull_request(
            repository("acme/widget", remote.0.to_str().unwrap(), &["test-origin"]),
            7,
        );
        request.base_oid = source.git(&["rev-parse", "main"]);
        request.head_oid = source.git(&["rev-parse", "feature/rocket"]);
        request.head_repository = None;
        request.changed_files = 21;

        let first_page = git_repository
            .local_pull_request_diff(&request, 1, DEFAULT_PULL_REQUEST_DIFF_PAGE_SIZE)
            .unwrap();
        let second_page = git_repository
            .local_pull_request_diff(&request, 2, DEFAULT_PULL_REQUEST_DIFF_PAGE_SIZE)
            .unwrap();

        assert_eq!(first_page.file_count(), 20);
        assert_eq!(second_page.file_count(), 1);
        let first_details = first_page.pull_request_details.unwrap();
        let second_details = second_page.pull_request_details.unwrap();
        assert_eq!(
            (first_details.file_page, first_details.total_files),
            (1, 21)
        );
        assert!(first_details.has_next_file_page);
        assert!(!first_details.has_previous_file_page);
        assert_eq!(second_details.file_page, 2);
        assert!(!second_details.has_next_file_page);
        assert!(second_details.has_previous_file_page);
        assert_eq!(source.git(&["branch", "--show-current"]), before_branch);
        assert_eq!(source.git(&["status", "--porcelain"]), before_status);
        assert_eq!(source.git(&["show-ref"]), before_refs);
    }

    #[test]
    fn temporary_bare_repository_is_removed_on_drop() {
        let path = {
            let repository = TemporaryBareRepository::new().unwrap();
            assert!(repository.path.exists());
            repository.path.clone()
        };
        assert!(!path.exists());
    }

    #[test]
    fn bounded_runner_kills_oversized_git_output() {
        let repository = initialized_repository();
        fs::write(repository.0.join("large.txt"), "x".repeat(256 * 1024)).unwrap();
        repository.git(&["add", "large.txt"]);
        repository.git(&["commit", "--message=large"]);
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&repository.0)
            .args(["show", "HEAD:large.txt"]);

        let output = run_bounded_command(&mut command, 1024, 1024).unwrap();

        assert!(output.stdout_truncated);
        assert_eq!(output.stdout.len(), 1024);
    }

    #[test]
    fn rejects_malformed_pull_request_output_without_panicking() {
        let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
        assert!(parse_pull_requests(b"not tsv", &base, std::slice::from_ref(&base)).is_err());
    }

    #[test]
    fn matching_head_remotes_do_not_cross_enterprise_hosts() {
        let dot_com = repository("acme/widget", "https://github.com/acme/widget", &["public"]);
        let enterprise = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["work"],
        );

        assert_eq!(
            matching_remotes(&[dot_com, enterprise], "github.example.com", "ACME/WIDGET"),
            vec!["work"]
        );
    }

    #[test]
    fn merges_fetch_and_push_aliases_for_the_same_repository() {
        let mut repositories = BTreeMap::new();
        merge_repository(
            &mut repositories,
            repository("acme/widget", "https://github.com/acme/widget/", &[]),
            Some("origin"),
        );
        merge_repository(
            &mut repositories,
            repository("acme/widget", "https://github.com/ACME/WIDGET", &[]),
            Some("upstream"),
        );

        let repository = repositories.into_values().next().unwrap();
        assert_eq!(repository.url, "https://github.com/acme/widget");
        assert_eq!(repository.remotes, vec!["origin", "upstream"]);
    }
}
