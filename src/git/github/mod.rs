mod checks;
mod conversation;

pub(crate) use self::checks::{
    CheckLogLine, CheckLogSeverity, CheckRunLog, CheckStep, PullRequestCheck,
    PullRequestCheckStatus, PullRequestChecks, unix_now,
};
pub(crate) use self::conversation::{ConversationEntry, ConversationKind, PullRequestConversation};

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, anyhow, bail};

use super::diff::{
    DiffDocument, DiffLineCounts, PullRequestDetails, parse_diff, parse_numstat,
    split_patch_by_file,
};
use super::{MAX_DIFF_BYTES, Repository, text, trim_ascii};

const MAX_GIT_REMOTES: usize = 32;
const MAX_REMOTE_URL_ENTRIES: usize = 64;
const MAX_REMOTE_URLS: usize = 32;
const MAX_GITHUB_REPOSITORIES: usize = 16;
const MAX_GH_METADATA_BYTES: usize = 2 * 1024 * 1024;
const MAX_PULL_REQUEST_TITLE_BYTES: usize = 16 * 1024;
const MAX_PULL_REQUEST_BODY_BYTES: usize = 256 * 1024;
const MAX_GH_ERROR_BYTES: usize = 256 * 1024;
const MAX_PR_PATH_BYTES: usize = 8 * 1024 * 1024;
const MAX_PR_PATHS: usize = 16_384;
/// A single file's patch is cached only if it is small enough that one file
/// cannot crowd out the rest of a pull request.
const MAX_CACHED_PATCH_BYTES: usize = 1024 * 1024;
/// The cache now holds immutable content (finished run logs, patches for a
/// fixed pair of commits) rather than only small metadata blobs, so the budget
/// is sized for those and pruned oldest-first.
const MAX_CACHE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 2_048;
const REPOSITORY_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const PULL_REQUEST_CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const TEMPORARY_REPOSITORY_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_MAGIC: &[u8] = b"quinjet-gh-cache-v1\n";

const PULL_REQUEST_FIELDS: &str = "number,title,body,author,state,isDraft,createdAt,updatedAt,url,baseRefName,baseRefOid,headRefName,headRefOid,headRepository,isCrossRepository,additions,deletions,changedFiles";
const PULL_REQUEST_VIEW_TSV_JQ: &str = r#"[(.number|tostring), .title, (.body // ""), (.author.login // "ghost"), .state, (.isDraft|tostring), .updatedAt, .url, .baseRefName, .headRefName, (.headRepository.nameWithOwner // ""), (.isCrossRepository|tostring), (.additions|tostring), (.deletions|tostring), (.changedFiles|tostring), .baseRefOid, .headRefOid, .createdAt] | @tsv"#;
const REPOSITORY_TSV_TEMPLATE: &str = "{{.nameWithOwner}}{{\"\\t\"}}{{.url}}{{\"\\n\"}}";
const PULL_REQUEST_TSV_FIELDS: usize = 18;

static TEMPORARY_REPOSITORY_ID: AtomicU64 = AtomicU64::new(0);
static CACHE_WRITE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GitHubRepository {
    pub name_with_owner: String,
    pub url: String,
    pub remotes: Vec<String>,
}

impl GitHubRepository {
    pub(crate) fn selector(&self) -> &str {
        &self.url
    }

    pub(crate) fn host(&self) -> &str {
        repository_host(&self.url).unwrap_or_default()
    }

    pub(crate) fn display_name(&self) -> String {
        let host = self.host();
        if host.is_empty() || host.eq_ignore_ascii_case("github.com") {
            self.name_with_owner.clone()
        } else {
            format!("{host}/{}", self.name_with_owner)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PullRequest {
    pub number: u64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub state: String,
    pub is_draft: bool,
    pub created_at: String,
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
    pub(crate) fn base_label(&self) -> String {
        format!("{}:{}", self.base_repository.display_name(), self.base_ref)
    }

    pub(crate) fn head_label(&self) -> String {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullRequestSnapshot {
    pub repositories: Vec<GitHubRepository>,
    pub selected_repository: Option<GitHubRepository>,
    pub pull_request: PullRequest,
    pub warnings: Vec<String>,
    pub exact_number: Option<u64>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullRequestFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: PullRequestFileStatus,
    pub counts: Option<DiffLineCounts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullRequestDiffIndex {
    pub files: Vec<PullRequestFile>,
    pub total_files: usize,
    pub truncated: bool,
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

/// How long an entry stays usable. `Immutable` is for content whose identity is
/// already in its key: a finished run's log, or a patch between two fixed
/// commits. Such an entry can never become wrong, only evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLife {
    Immutable,
    Ttl(Duration),
}

impl CacheLife {
    fn accepts(self, age: Duration) -> bool {
        match self {
            Self::Immutable => true,
            Self::Ttl(ttl) => age <= ttl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestProgress {
    LoadingMetadata,
    PreparingRepository,
    FetchingBase,
    FetchingHead,
    FindingMergeBase,
    EnumeratingFiles,
}

impl PullRequestProgress {
    pub(crate) const fn percent(self) -> u16 {
        match self {
            Self::LoadingMetadata => 10,
            Self::PreparingRepository => 20,
            Self::FetchingBase => 35,
            Self::FetchingHead => 50,
            Self::FindingMergeBase => 65,
            Self::EnumeratingFiles => 90,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::LoadingMetadata => "Fetching pull-request metadata",
            Self::PreparingRepository => "Preparing an isolated diff workspace",
            Self::FetchingBase => "Fetching the destination commit",
            Self::FetchingHead => "Fetching the source commit",
            Self::FindingMergeBase => "Finding the merge base",
            Self::EnumeratingFiles => "Enumerating changed files",
        }
    }
}

enum PreparedRepository {
    Opened(PathBuf),
    Temporary(TemporaryBareRepository),
}

impl PreparedRepository {
    fn path(&self) -> &Path {
        match self {
            Self::Opened(path) => path,
            Self::Temporary(repository) => &repository.path,
        }
    }
}

pub(crate) struct PreparedPullRequest {
    repository: PreparedRepository,
    pull_request: PullRequest,
    merge_base: String,
    head: String,
    index: PullRequestDiffIndex,
}

impl PreparedPullRequest {
    pub(crate) fn index(&self) -> PullRequestDiffIndex {
        self.index.clone()
    }

    #[expect(
        clippy::similar_names,
        reason = "the names follow the Git vocabulary they model"
    )]
    pub(crate) fn diff_file(&self, path: &Path) -> Result<DiffDocument> {
        let file = self
            .index
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{} is not part of this pull request", path.display()))?;
        let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
        if let Some(patch) = cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES)
        {
            return Ok(pull_request_file_document(
                &patch,
                &self.pull_request,
                file,
                false,
            ));
        }
        let (patch, truncated) = diff_selected_paths(
            self.repository.path(),
            &self.merge_base,
            &self.head,
            std::slice::from_ref(&file.path),
        )?;
        if !truncated {
            cache_write_bounded(&key, &patch, MAX_CACHED_PATCH_BYTES);
        }
        Ok(pull_request_file_document(
            &patch,
            &self.pull_request,
            file,
            truncated,
        ))
    }

    #[expect(
        clippy::option_if_let_else,
        reason = "the branch is one arm of a longer chain that map_or_else cannot express"
    )]
    /// Produce many file documents from a single `git diff`. Spawning one Git
    /// process per file dominates the cost of a wide pull request, so batching
    /// is what lets the whole diff arrive while the reader is still reading the
    /// first file.
    pub(crate) fn diff_files(&self, paths: &[PathBuf]) -> Result<Vec<(PathBuf, DiffDocument)>> {
        let files: Vec<&PullRequestFile> = paths
            .iter()
            .filter_map(|path| self.index.files.iter().find(|file| &file.path == path))
            .collect();
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let mut cached: HashMap<PathBuf, Vec<u8>> = HashMap::new();
        let mut requested: Vec<PathBuf> = Vec::new();
        for file in &files {
            let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
            match cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES) {
                Some(patch) => {
                    drop(cached.insert(file.path.clone(), patch));
                }
                None => requested.push(file.path.clone()),
            }
        }
        let (patch, truncated) = if requested.is_empty() {
            (Vec::new(), false)
        } else {
            diff_selected_paths(
                self.repository.path(),
                &self.merge_base,
                &self.head,
                &requested,
            )?
        };
        let sections = split_patch_by_file(&patch);
        Ok(files
            .into_iter()
            .map(|file| {
                let body = if let Some(patch) = cached.get(&file.path) {
                    patch.as_slice()
                } else {
                    let body = sections
                        .iter()
                        .find(|section| section.matches(&file.path))
                        .map(|section| section.body)
                        .unwrap_or_default();
                    if !truncated {
                        let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
                        cache_write_bounded(&key, body, MAX_CACHED_PATCH_BYTES);
                    }
                    body
                };
                (
                    file.path.clone(),
                    pull_request_file_document(body, &self.pull_request, file, truncated),
                )
            })
            .collect())
    }
}

/// Direct cache access for readers that cannot express themselves as a single
/// `gh` invocation: a response judged by its body rather than its exit status,
/// or bytes produced by Git rather than by GitHub.
pub(crate) fn cache_read(key: &str, life: CacheLife) -> Option<Vec<u8>> {
    cache_read_bounded(key, life, MAX_GH_METADATA_BYTES)
}

pub(crate) fn cache_read_bounded(key: &str, life: CacheLife, limit: usize) -> Option<Vec<u8>> {
    CacheStore::discover()?
        .read(key, limit)
        .filter(|entry| life.accepts(entry.age))
        .map(|entry| entry.data)
}

pub(crate) fn cache_write(key: &str, data: &[u8]) {
    cache_write_bounded(key, data, MAX_GH_METADATA_BYTES);
}

pub(crate) fn cache_write_bounded(key: &str, data: &[u8], limit: usize) {
    if let Some(cache) = CacheStore::discover() {
        drop(cache.write(key, data, limit));
    }
}

/// A validated read: GitHub is asked whether the answer changed, and answers
/// `304 Not Modified` when it did not. That reply carries no body and costs
/// nothing against the rate limit, which is what lets an unchanged thread be
/// re-checked as often as it is worth checking.
///
/// The entry holds the validator on its first line and the body after it, so
/// the two can never be stored out of step with each other.
pub(crate) struct ValidatedRead {
    pub data: Vec<u8>,
    pub unchanged: bool,
}

impl Repository {
    pub(crate) fn validated_gh(&self, key: &str, args: Vec<OsString>) -> Result<ValidatedRead> {
        let cached = cache_read(key, CacheLife::Immutable);
        let validator = cached.as_ref().and_then(|entry| split_validator(entry).0);
        let mut request = vec![OsString::from("api"), OsString::from("-i")];
        if let Some(validator) = validator.as_ref() {
            request.push(OsString::from("-H"));
            request.push(OsString::from(format!("If-None-Match: {validator}")));
        }
        request.extend(args);

        let output = self.run_gh(request)?;
        if !output.status.success() {
            bail!(
                "{}",
                bounded_command_error("unable to read from GitHub", &output)
            );
        }
        let (head, body) = split_http_response(&output.stdout);
        let head = head.as_ref();
        let status =
            String::from_utf8_lossy(head.lines().next().unwrap_or_default().as_bytes()).to_string();
        if status.contains(" 304") {
            if let Some(entry) = cached {
                return Ok(ValidatedRead {
                    data: split_validator(&entry).1.to_vec(),
                    unchanged: true,
                });
            }
        }
        if let Some(etag) = header_value(head, "etag").filter(|_| !has_next_page(head)) {
            let mut entry = etag.into_bytes();
            entry.push(b'\n');
            entry.extend_from_slice(body);
            cache_write(key, &entry);
        }
        Ok(ValidatedRead {
            data: body.to_vec(),
            unchanged: false,
        })
    }
}

/// Split the stored entry into its validator and the body it validates.
fn split_validator(entry: &[u8]) -> (Option<String>, &[u8]) {
    entry
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or((None, entry), |index| {
            let (validator, body) = entry.split_at(index);
            (
                Some(String::from_utf8_lossy(validator).into_owned()),
                body.get(1..).unwrap_or_default(),
            )
        })
}

/// `gh api -i` prints the response head, a blank line, then the body.
fn split_http_response(output: &[u8]) -> (Cow<'_, str>, &[u8]) {
    for separator in [b"\r\n\r\n".as_slice(), b"\n\n".as_slice()] {
        if let Some(index) = output
            .windows(separator.len())
            .position(|window| window == separator)
        {
            let (head, rest) = output.split_at(index);
            return (
                String::from_utf8_lossy(head),
                rest.get(separator.len()..).unwrap_or_default(),
            );
        }
    }
    (String::from_utf8_lossy(output), &[])
}

fn header_value(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

fn has_next_page(head: &str) -> bool {
    header_value(head, "link").is_some_and(|link| link.contains("rel=\"next\""))
}

struct GhResponse {
    data: Vec<u8>,
    disposition: CacheDisposition,
}

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
        mut progress: F,
    ) -> Result<PreparedPullRequest>
    where
        F: FnMut(PullRequestProgress),
    {
        let (repository, merge_base, head) =
            if self.has_commit(&pull_request.base_oid) && self.has_commit(&pull_request.head_oid) {
                progress(PullRequestProgress::FindingMergeBase);
                (
                    PreparedRepository::Opened(self.root().to_path_buf()),
                    self.merge_base(&pull_request.base_oid, &pull_request.head_oid)?,
                    pull_request.head_oid.clone(),
                )
            } else {
                progress(PullRequestProgress::PreparingRepository);
                let temporary = TemporaryBareRepository::new()?;
                let (merge_base, head) =
                    fetch_pull_request(&temporary.path, pull_request, &mut progress)?;
                (PreparedRepository::Temporary(temporary), merge_base, head)
            };
        progress(PullRequestProgress::EnumeratingFiles);
        let (files, truncated) =
            changed_files_in_repository(repository.path(), &merge_base, &head)?;
        let total_files = if truncated {
            pull_request.changed_files.max(files.len())
        } else {
            files.len()
        };
        Ok(PreparedPullRequest {
            repository,
            pull_request: pull_request.clone(),
            merge_base,
            head,
            index: PullRequestDiffIndex {
                files,
                total_files,
                truncated,
            },
        })
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
                "pull-request-v3\n{}\n{number}",
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

    fn merge_base(&self, base: &str, head: &str) -> Result<String> {
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

    pub(crate) fn github_repositories(
        &self,
        refresh: bool,
    ) -> Result<(Vec<GitHubRepository>, Vec<String>)> {
        let (remote_urls, mut warnings) = self.remote_urls()?;
        let grouped_remote_urls = group_remote_urls(&remote_urls);
        let mut repositories = BTreeMap::new();

        for (index, (url, remotes)) in grouped_remote_urls.iter().take(MAX_REMOTE_URLS).enumerate()
        {
            match repository_from_remote_url(url)
                .map(|repository| (repository, CacheDisposition::Fresh))
                .map_or_else(|| self.resolve_github_repository(Some(url), refresh), Ok)
            {
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
                    let _ = urls.insert(entry);
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
        let identity = url.map_or_else(
            || {
                format!(
                    "inferred\n{}\n{}",
                    self.root.display(),
                    env::var("GH_REPO").unwrap_or_default()
                )
            },
            remote_url_for_gh,
        );
        let key = format!("repository\n{identity}");
        let response = self.checked_cached_gh(
            &key,
            CacheLife::Ttl(REPOSITORY_CACHE_TTL),
            refresh,
            args,
            "gh repo view failed",
        )?;
        let record = trim_ascii(&response.data);
        let [name_with_owner, host] =
            parse_tsv_record::<2>(record).context("invalid gh repo view output")?;
        if name_with_owner.trim().is_empty() || host.trim().is_empty() {
            bail!("gh repo view returned an incomplete repository identity");
        }
        let fields = [name_with_owner, host];
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
        life: CacheLife,
        refresh: bool,
        args: I,
        error_context: &str,
    ) -> Result<GhResponse>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.checked_cached_gh_bounded(
            cache_key,
            life,
            refresh,
            args,
            error_context,
            MAX_GH_METADATA_BYTES,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the renderer needs the whole row context in one call"
    )]
    /// `limit` bounds both the response Quinjet will read and the entry it will
    /// keep, so a check log can use the cache without letting metadata grow.
    fn checked_cached_gh_bounded<I, S>(
        &self,
        cache_key: &str,
        life: CacheLife,
        refresh: bool,
        args: I,
        error_context: &str,
        limit: usize,
    ) -> Result<GhResponse>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cache = CacheStore::discover();
        let cached = cache
            .as_ref()
            .and_then(|cache| cache.read(cache_key, limit));
        if !refresh || life == CacheLife::Immutable {
            if let Some(entry) = cached.as_ref() {
                if life.accepts(entry.age) {
                    return Ok(GhResponse {
                        data: entry.data.clone(),
                        disposition: CacheDisposition::Fresh,
                    });
                }
            }
        }

        let output = match self.run_gh_bounded(args, limit) {
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
                drop(cache.write(cache_key, &output.stdout, limit));
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
        self.run_gh_bounded(args, MAX_GH_METADATA_BYTES)
    }

    /// Metadata responses are small and share one cap, but a check run log is
    /// arbitrarily large and needs its own.
    fn run_gh_bounded<I, S>(&self, args: I, stdout_limit: usize) -> Result<BoundedOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("gh");
        let _ = command
            .current_dir(&self.root)
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("NO_COLOR", "1");
        run_bounded_command(&mut command, stdout_limit, MAX_GH_ERROR_BYTES).with_context(|| {
            format!(
                "failed to execute GitHub CLI (`gh`) in {}; install it and run `gh auth login`",
                self.root.display()
            )
        })
    }
}

fn pull_request_file_document(
    output: &[u8],
    pull_request: &PullRequest,
    file: &PullRequestFile,
    truncated: bool,
) -> DiffDocument {
    let file_additions = count_patch_lines(output, b'+');
    let file_deletions = count_patch_lines(output, b'-');
    let mut document = parse_diff(
        output,
        format!("PR #{}  ·  {}", pull_request.number, file.path.display()),
        Some(&file.path),
        truncated,
    );
    document.truncated |= truncated;
    document.pull_request_details = Some(PullRequestDetails {
        number: pull_request.number,
        title: pull_request.title.clone(),
        description: pull_request.description.clone(),
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
        changed_files: pull_request.changed_files,
        additions: pull_request.additions,
        deletions: pull_request.deletions,
        selected_file: Some(file.path.to_string_lossy().into_owned()),
        selected_file_additions: file_additions,
        selected_file_deletions: file_deletions,
    });
    document
}

fn count_patch_lines(output: &[u8], marker: u8) -> usize {
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| {
            line.first() == Some(&marker)
                && !line.starts_with(if marker == b'+' { b"+++ " } else { b"--- " })
        })
        .count()
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
        let fields = parse_tsv_record::<PULL_REQUEST_TSV_FIELDS>(record)
            .with_context(|| format!("invalid pull-request record {}", index + 1))?;
        pull_requests.push(parse_pull_request_fields(
            fields,
            base_repository,
            repositories,
        )?);
    }
    Ok(pull_requests)
}

fn parse_pull_request_fields(
    fields: [String; PULL_REQUEST_TSV_FIELDS],
    base_repository: &GitHubRepository,
    repositories: &[GitHubRepository],
) -> Result<PullRequest> {
    let [
        number,
        title,
        description,
        author,
        state,
        draft,
        updated_at,
        url,
        base_ref,
        head_ref,
        head_repository_name,
        cross_repository,
        additions,
        deletions,
        changed_files,
        base_oid,
        head_oid,
        created_at,
    ] = fields;
    let head_repository = (!head_repository_name.is_empty()).then_some(head_repository_name);
    let head_remotes = head_repository
        .as_deref()
        .map(|name| matching_remotes(repositories, base_repository.host(), name))
        .unwrap_or_default();
    Ok(PullRequest {
        number: parse_field(&number, "number")?,
        title: bounded_text(&title, MAX_PULL_REQUEST_TITLE_BYTES),
        description: bounded_text(&description, MAX_PULL_REQUEST_BODY_BYTES),
        author,
        state: state.to_ascii_uppercase(),
        is_draft: parse_field(&draft, "draft state")?,
        updated_at,
        url,
        base_ref,
        head_ref,
        base_repository: base_repository.clone(),
        head_repository,
        head_remotes,
        is_cross_repository: parse_field(&cross_repository, "cross-repository state")?,
        additions: parse_field(&additions, "addition count")?,
        deletions: parse_field(&deletions, "deletion count")?,
        changed_files: parse_field(&changed_files, "changed-file count")?,
        base_oid,
        head_oid,
        created_at,
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
    format!("{}…", value.get(..end).unwrap_or_default())
}

fn parse_tsv_record<const FIELDS: usize>(record: &[u8]) -> Result<[String; FIELDS]> {
    let record = record.strip_suffix(b"\r").unwrap_or(record);
    let fields: Vec<_> = record
        .split(|byte| *byte == b'\t')
        .map(text)
        .map(|field| unescape_tsv(&field))
        .collect();
    let received = fields.len();
    fields
        .try_into()
        .map_err(|_| anyhow!("expected {FIELDS} tab-separated fields, received {received}"))
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
            Some(other) => {
                if other != '\\' {
                    output.push('\\');
                }
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

fn repository_from_remote_url(url: &str) -> Option<GitHubRepository> {
    let sanitized = remote_url_for_gh(url);
    let (scheme, rest) = sanitized.split_once("://")?;
    if !matches!(scheme, "http" | "https" | "ssh") {
        return None;
    }
    let (host, path) = rest.split_once('/')?;
    if !host.eq_ignore_ascii_case("github.com") || path.is_empty() {
        return None;
    }
    let mut components = path.trim_matches('/').split('/');
    let owner = components.next()?;
    let raw_name = components.next()?;
    let name = raw_name.strip_suffix(".git").unwrap_or(raw_name);
    if owner.is_empty() || name.is_empty() || components.next().is_some() {
        return None;
    }
    let canonical_scheme = if scheme == "ssh" { "https" } else { scheme };
    Some(GitHubRepository {
        name_with_owner: format!("{owner}/{name}"),
        url: format!("{canonical_scheme}://{host}/{owner}/{name}"),
        remotes: Vec::new(),
    })
}

fn group_remote_urls(remote_urls: &[RemoteUrl]) -> Vec<(String, Vec<String>)> {
    let mut grouped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for remote_url in remote_urls {
        let _ = grouped
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
            // nosemgrep: rust.lang.security.temp-dir.temp-dir
            _ => env::temp_dir(),
        };
        for _ in 0..16 {
            let id = TEMPORARY_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("pr-{}-{id}.git", std::process::id()));
            if path.exists() {
                continue;
            }
            let mut command = Command::new("git");
            let _ = command
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
        drop(fs::remove_dir_all(&self.path));
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
            .is_some_and(|name| {
                name.starts_with("pr-") && Path::new(name).extension() == Some(OsStr::new("git"))
            });
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
            drop(fs::remove_dir_all(path));
        }
    }
}

fn fetch_pull_request(
    temporary: &Path,
    pull_request: &PullRequest,
    progress: &mut dyn FnMut(PullRequestProgress),
) -> Result<(String, String)> {
    if pull_request.base_ref.is_empty() || pull_request.head_ref.is_empty() {
        bail!("Pull request metadata does not contain complete base/head refs");
    }
    drop(checked_temp_git(
        temporary,
        &[
            OsString::from("remote"),
            OsString::from("add"),
            OsString::from("origin"),
            OsString::from(pull_request.base_repository.selector()),
        ],
        "unable to configure the disposable base remote",
    )?);
    let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
    let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);

    progress(PullRequestProgress::FetchingBase);
    fetch_ref(temporary, "origin", &base_refspec, 64)?;
    progress(PullRequestProgress::FetchingHead);
    let (head_remote, head_refspec) = match fetch_ref(temporary, "origin", &pull_refspec, 64) {
        Ok(()) => ("origin".to_owned(), pull_refspec),
        Err(pull_ref_error) => {
            let Some(head_repository) = pull_request.head_repository.as_deref() else {
                return Err(pull_ref_error).context(
                    "the base repository no longer exposes the PR head and its fork was deleted",
                );
            };
            let head_url = repository_url_for_name(&pull_request.base_repository, head_repository);
            drop(checked_temp_git(
                temporary,
                &[
                    OsString::from("remote"),
                    OsString::from("add"),
                    OsString::from("head"),
                    OsString::from(head_url),
                ],
                "unable to configure the disposable fork remote",
            )?);
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

    progress(PullRequestProgress::FindingMergeBase);
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

fn changed_files_in_repository(
    repository: &Path,
    merge_base: &str,
    head: &str,
) -> Result<(Vec<PullRequestFile>, bool)> {
    let args = [
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    let counts = numstat_counts(repository, merge_base, head);
    let key = format!("pr-files-v1\n{merge_base}\n{head}");
    let cached = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES);
    let output = if let Some(data) = cached {
        BoundedOutput {
            status: successful_status(),
            stdout: data,
            stderr: Vec::new(),
            stdout_truncated: false,
        }
    } else {
        let output = run_repository_git(repository, &args, MAX_PR_PATH_BYTES, 128 * 1024)?;
        if !output.status.success() && !output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to enumerate pull-request files", &output)
            );
        }
        if !output.stdout_truncated {
            cache_write_bounded(&key, &output.stdout, MAX_PR_PATH_BYTES);
        }
        output
    };
    let mut truncated = output.stdout_truncated;
    let complete_output = if output.stdout_truncated && !output.stdout.ends_with(&[0]) {
        output
            .stdout
            .iter()
            .rposition(|byte| *byte == 0)
            .map_or(&[][..], |index| {
                output.stdout.get(..=index).unwrap_or(&output.stdout)
            })
    } else {
        &output.stdout
    };
    let records = complete_output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < records.len() {
        if files.len() >= MAX_PR_PATHS {
            truncated = true;
            break;
        }
        let Some(status_record) = records.get(index).copied() else {
            break;
        };
        index += 1;
        let status_code = status_record.first().copied().unwrap_or_default();
        let status = match status_code {
            b'A' => PullRequestFileStatus::Added,
            b'M' => PullRequestFileStatus::Modified,
            b'D' => PullRequestFileStatus::Deleted,
            b'R' => PullRequestFileStatus::Renamed,
            b'C' => PullRequestFileStatus::Copied,
            b'T' => PullRequestFileStatus::TypeChanged,
            b'U' => PullRequestFileStatus::Unmerged,
            _ => PullRequestFileStatus::Unknown,
        };
        let rename_or_copy = matches!(
            status,
            PullRequestFileStatus::Renamed | PullRequestFileStatus::Copied
        );
        let Some(first_path) = records.get(index) else {
            truncated = true;
            break;
        };
        index += 1;
        let first_path = PathBuf::from(String::from_utf8_lossy(first_path).into_owned());
        let (old_path, path) = if rename_or_copy {
            let Some(new_path) = records.get(index) else {
                truncated = true;
                break;
            };
            index += 1;
            (
                Some(first_path),
                PathBuf::from(String::from_utf8_lossy(new_path).into_owned()),
            )
        } else {
            (None, first_path)
        };
        let file_counts = counts.get(&path).copied();
        files.push(PullRequestFile {
            path,
            old_path,
            status,
            counts: file_counts,
        });
    }
    Ok((files, truncated))
}

/// Read exact per-file totals alongside the changed-path listing. One extra
/// `--numstat` pass over the same range lets every file header render its real
/// `+n -n` immediately, so the list never fills in unevenly as patches load.
fn numstat_counts(
    repository: &Path,
    merge_base: &str,
    head: &str,
) -> HashMap<PathBuf, DiffLineCounts> {
    let key = format!("pr-numstat-v1\n{merge_base}\n{head}");
    if let Some(data) = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES) {
        return parse_numstat(&data);
    }
    let args = [
        OsString::from("diff"),
        OsString::from("--numstat"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    run_repository_git(repository, &args, MAX_PR_PATH_BYTES, 128 * 1024)
        .ok()
        .filter(|output| output.status.success() && !output.stdout_truncated)
        .map(|output| {
            cache_write_bounded(&key, &output.stdout, MAX_PR_PATH_BYTES);
            parse_numstat(&output.stdout)
        })
        .unwrap_or_default()
}

/// A status that reports success, for feeding cached bytes back through the
/// same path a real command's output takes.
fn successful_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
    #[cfg(not(unix))]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
}

fn patch_cache_key(merge_base: &str, head: &str, path: &Path) -> String {
    format!("pr-patch-v1\n{merge_base}\n{head}\n{}", path.display())
}

fn diff_selected_paths(
    repository: &Path,
    merge_base: &str,
    head: &str,
    paths: &[PathBuf],
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
    args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
    let output = run_repository_git(repository, &args, MAX_DIFF_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to generate the local pull-request diff", &output)
        );
    }
    let mut patch = output.stdout;
    if output.stdout_truncated {
        while patch.last().is_some_and(|byte| *byte != b'\n') {
            let _ = patch.pop();
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
    run_repository_git(temporary, args, stdout_limit, stderr_limit)
}

fn run_repository_git(
    repository: &Path,
    args: &[OsString],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let mut command = Command::new("git");
    let _ = command
        .arg("-C")
        .arg(repository)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    run_bounded_command(&mut command, stdout_limit, stderr_limit)
        .with_context(|| format!("failed to execute Git in {}", repository.display()))
}

pub(crate) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
}

#[expect(
    clippy::large_stack_arrays,
    reason = "the read buffer is deliberately one page of stack"
)]
pub(crate) fn run_bounded_command(
    command: &mut Command,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let _ = command.stdout(Stdio::piped()).stderr(Stdio::piped());
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
                drop(child.kill());
                drop(child.wait());
                drop(stderr_reader.join());
                return Err(error.into());
            }
        };
        let remaining = stdout_limit.saturating_sub(collected.len());
        if read > remaining {
            collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
            truncated = true;
            drop(child.kill());
            break;
        }
        collected.extend_from_slice(buffer.get(..read).unwrap_or(&buffer));
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

#[expect(
    clippy::large_stack_arrays,
    reason = "the read buffer is deliberately one page of stack"
)]
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
        collected.extend_from_slice(buffer.get(..read.min(remaining)).unwrap_or(&buffer));
    }
    Ok(collected)
}

pub(crate) fn bounded_command_error(context: &str, output: &BoundedOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if stderr.is_empty() { stdout } else { stderr };
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
    const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    fn read(&self, key: &str, limit: usize) -> Option<CacheEntry> {
        let path = self.path(key);
        let metadata = fs::metadata(&path).ok()?;
        if metadata.len() > limit as u64 + CACHE_MAGIC.len() as u64 {
            drop(fs::remove_file(path));
            return None;
        }
        let mut data = fs::read(path).ok()?;
        if !data.starts_with(CACHE_MAGIC) {
            return None;
        }
        drop(data.drain(..CACHE_MAGIC.len()));
        let age = metadata
            .modified()
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .unwrap_or_default();
        Some(CacheEntry { data, age })
    }

    fn write(&self, key: &str, data: &[u8], limit: usize) -> Result<()> {
        if data.len() > limit {
            return Ok(());
        }
        create_private_directory(&self.root)?;
        let destination = self.path(key);
        let id = CACHE_WRITE_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = self
            .root
            .join(format!(".write-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        let _ = options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let _ = options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(CACHE_MAGIC)?;
        file.write_all(data)?;
        file.flush()?;
        drop(file);
        if destination.exists() {
            drop(fs::remove_file(&destination));
        }
        if let Err(error) = fs::rename(&temporary, &destination) {
            drop(fs::remove_file(&temporary));
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
    let mut left = 0xcbf2_9ce4_8422_2325_u64;
    let mut right = 0x8422_2325_cbf2_9ce4_u64;
    for byte in value {
        left ^= u64::from(*byte);
        left = left.wrapping_mul(0x0100_0000_01b3);
        right ^= u64::from(*byte).rotate_left(1);
        right = right.wrapping_mul(0x0100_0000_01b3).rotate_left(5);
    }
    (left, right)
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
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
            drop(fs::remove_dir_all(&self.0));
        }
    }

    fn test_directory(label: &str) -> TestDirectory {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        let path = env::temp_dir().join(format!(
            "quinjet-github-{label}-{}-{id}",
            std::process::id()
        ));
        drop(fs::remove_dir_all(&path));
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

    pub(super) fn repository(name: &str, url: &str, remotes: &[&str]) -> GitHubRepository {
        GitHubRepository {
            name_with_owner: name.to_owned(),
            url: url.to_owned(),
            remotes: remotes.iter().map(|remote| (*remote).to_owned()).collect(),
        }
    }

    pub(super) fn pull_request(base: GitHubRepository, number: u64) -> PullRequest {
        PullRequest {
            number,
            title: "Ship the rocket".to_owned(),
            description: "Launch safely".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            created_at: "2026-08-12T09:00:00Z".to_owned(),
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
            parse_tsv_record::<3>(b"one\ttwo\\tinside\tthree\r").unwrap(),
            [
                "one".to_owned(),
                "two\tinside".to_owned(),
                "three".to_owned()
            ]
        );
    }

    #[test]
    fn derives_standard_github_repository_identity_without_network_resolution() {
        assert_eq!(
            repository_from_remote_url("https://github.com/acme/widget.git"),
            Some(GitHubRepository {
                name_with_owner: "acme/widget".to_owned(),
                url: "https://github.com/acme/widget".to_owned(),
                remotes: Vec::new(),
            })
        );
        assert!(
            repository_from_remote_url("git@github.example.com:acme/widget.git").is_none(),
            "enterprise hosts must still be validated through gh"
        );
        assert!(repository_from_remote_url("https://gitlab.com/acme/widget.git").is_none());
        assert!(repository_from_remote_url("file:///tmp/widget.git").is_none());
        assert!(repository_from_remote_url("https://github.com/acme/widget/extra").is_none());
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
        let output = b"42\tShip the rocket\tDetailed\\nbody\toctocat\tOPEN\ttrue\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t12\t3\t4\tbaseoid\theadid\t2026-08-01T09:00:00Z\n";

        let requests = parse_pull_requests(output, &upstream, &[upstream.clone(), fork]).unwrap();

        let request = &requests[0];
        assert_eq!(request.description, "Detailed\nbody");
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
        let output = b"7\tOld contribution\t\tghost\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.example.com/acme/widget/pull/7\ttrunk\tlost-branch\t\tfalse\t0\t0\t1\tbaseoid\theadid\t2025-12-30T00:00:00Z\n";

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
        assert!(args.iter().any(|arg| arg.contains("body")));
    }

    #[test]
    fn cache_round_trips_private_metadata_and_uses_stable_keys() {
        let directory = test_directory("cache");
        let cache = CacheStore::at(directory.0.clone());
        cache
            .write("repo\npage 1", b"metadata\n", MAX_GH_METADATA_BYTES)
            .unwrap();

        let entry = cache.read("repo\npage 1", MAX_GH_METADATA_BYTES).unwrap();
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
    fn selected_file_counts_include_raw_patch_lines_when_rendering_is_truncated() {
        let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
        let mut request = pull_request(base, 9);
        request.changed_files = 1;
        request.additions = 3;
        request.deletions = 2;
        let patch = b"diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1,2 +1,3 @@\n-old one\n-old two\n+new one\n+new two\n+new three\n";

        let file = PullRequestFile {
            path: PathBuf::from("test.txt"),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        };
        let document = pull_request_file_document(patch, &request, &file, true);
        let details = document.pull_request_details.unwrap();

        assert_eq!(
            (
                details.selected_file_additions,
                details.selected_file_deletions
            ),
            (3, 2)
        );
        assert_eq!((details.additions, details.deletions), (3, 2));
    }

    #[test]
    fn changed_file_index_includes_add_modify_delete_and_rename_statuses() {
        let repository = initialized_repository();
        fs::write(repository.0.join("modified.txt"), "before\n").unwrap();
        fs::write(repository.0.join("deleted.txt"), "delete me\n").unwrap();
        fs::write(repository.0.join("renamed.txt"), "keep this content\n").unwrap();
        repository.git(&["add", "."]);
        repository.git(&["commit", "--message=fixtures"]);
        let base = repository.git(&["rev-parse", "HEAD"]);

        fs::write(repository.0.join("modified.txt"), "after\n").unwrap();
        fs::remove_file(repository.0.join("deleted.txt")).unwrap();
        fs::write(repository.0.join("added.txt"), "new\n").unwrap();
        repository.git(&["mv", "renamed.txt", "moved.txt"]);
        repository.git(&["add", "."]);
        repository.git(&["commit", "--message=changes"]);
        let head = repository.git(&["rev-parse", "HEAD"]);

        let (files, truncated) = changed_files_in_repository(&repository.0, &base, &head).unwrap();

        assert!(!truncated);
        assert!(files.iter().any(|file| {
            file.path == Path::new("added.txt") && file.status == PullRequestFileStatus::Added
        }));
        assert!(files.iter().any(|file| {
            file.path == Path::new("modified.txt") && file.status == PullRequestFileStatus::Modified
        }));
        assert!(files.iter().any(|file| {
            file.path == Path::new("deleted.txt") && file.status == PullRequestFileStatus::Deleted
        }));
        assert!(files.iter().any(|file| {
            file.path == Path::new("moved.txt")
                && file.old_path.as_deref() == Some(Path::new("renamed.txt"))
                && file.status == PullRequestFileStatus::Renamed
        }));

        assert!(files.iter().all(|file| file.counts.is_some()));
        assert_eq!(
            files
                .iter()
                .find(|file| file.path == Path::new("modified.txt"))
                .and_then(|file| file.counts),
            Some(DiffLineCounts {
                additions: 1,
                deletions: 1,
                binary: false,
            })
        );
        assert_eq!(
            files
                .iter()
                .find(|file| file.path == Path::new("moved.txt"))
                .and_then(|file| file.counts),
            Some(DiffLineCounts::default())
        );
    }

    #[test]
    fn locally_available_pr_objects_avoid_disposable_fetches() {
        let source = initialized_repository();
        let base_oid = source.git(&["rev-parse", "HEAD"]);
        source.git(&["switch", "-c", "feature/local-preview"]);
        fs::write(source.0.join("local.txt"), "available locally\n").unwrap();
        source.git(&["add", "local.txt"]);
        source.git(&["commit", "--message=local preview"]);
        let head_oid = source.git(&["rev-parse", "HEAD"]);
        let git_repository = Repository {
            root: source.0.clone(),
        };
        let mut request = pull_request(
            repository(
                "acme/widget",
                "https://invalid.example.test/acme/widget",
                &["origin"],
            ),
            7,
        );
        request.base_oid = base_oid;
        request.head_oid = head_oid;
        request.changed_files = 1;

        let started = std::time::Instant::now();
        let workspace = git_repository
            .prepare_pull_request_diff(&request, |_| {})
            .unwrap();
        let index = workspace.index();
        let document = workspace.diff_file(&index.files[0].path).unwrap();
        let elapsed = started.elapsed();

        assert_eq!(index.files.len(), 1);
        assert_eq!(document.file_count(), 1);
        assert!(
            document
                .lines
                .iter()
                .any(|line| line.text().contains("local.txt"))
        );
        assert!(elapsed < Duration::from_secs(2));
    }

    #[test]
    fn disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source() {
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
        request.base_oid.clear();
        request.head_oid.clear();
        request.head_repository = None;
        request.changed_files = 21;
        request.additions = 21;
        request.deletions = 0;

        let workspace = git_repository
            .prepare_pull_request_diff(&request, |_| {})
            .unwrap();
        let temporary_path = match &workspace.repository {
            PreparedRepository::Temporary(repository) => repository.path.clone(),
            PreparedRepository::Opened(_) => panic!("expected an isolated PR workspace"),
        };
        let index = workspace.index();
        assert_eq!(index.files.len(), 21);
        assert_eq!(index.total_files, 21);
        assert!(!index.truncated);
        assert!(temporary_path.exists());

        let mut additions = 0;
        let mut deletions = 0;
        for file in &index.files {
            let document = workspace.diff_file(&file.path).unwrap();
            assert_eq!(document.file_count(), 1);
            additions += document.addition_count();
            deletions += document.deletion_count();
        }
        assert_eq!((additions, deletions), (21, 0));

        let paths: Vec<PathBuf> = index.files.iter().map(|file| file.path.clone()).collect();
        let batch = workspace.diff_files(&paths).unwrap();
        assert_eq!(batch.len(), 21);
        assert_eq!(
            batch
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>(),
            paths
        );
        assert!(batch.iter().all(|(_, document)| document.file_count() == 1));
        assert_eq!(
            batch
                .iter()
                .map(|(_, document)| document.addition_count())
                .sum::<usize>(),
            21
        );
        assert_eq!(
            workspace
                .diff_files(&[PathBuf::from("never-changed.txt")])
                .unwrap(),
            Vec::new()
        );
        drop(workspace);
        assert!(!temporary_path.exists());
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
        parse_pull_requests(b"not tsv", &base, std::slice::from_ref(&base)).unwrap_err();
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

    #[test]
    fn a_response_head_is_read_apart_from_its_body() {
        let response = b"HTTP/2.0 200 OK\r\nEtag: W/\"92ade\"\r\nContent-Type: application/json\r\n\r\n[{\"a\":1}]";
        let (head, body) = split_http_response(response);
        assert!(head.starts_with("HTTP/2.0 200 OK"));
        assert_eq!(body, b"[{\"a\":1}]");
        assert_eq!(header_value(&head, "etag").as_deref(), Some("W/\"92ade\""));
        assert_eq!(header_value(&head, "ETAG").as_deref(), Some("W/\"92ade\""));
        assert_eq!(header_value(&head, "link"), None);
    }

    #[test]
    fn a_body_the_head_cannot_describe_still_arrives_whole() {
        let mut response = b"HTTP/2.0 200 OK\n\n".to_vec();
        response.extend_from_slice(&[0xff, 0xfe, b'o', b'k']);
        let (head, body) = split_http_response(&response);
        assert_eq!(head, "HTTP/2.0 200 OK");
        assert_eq!(body, [0xff, 0xfe, b'o', b'k']);
    }

    #[test]
    fn only_a_single_page_answer_is_worth_a_validator() {
        let paged = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=2>; rel=\"next\"";
        let last = "HTTP/2.0 200 OK\nLink: <https://api.github.com/x?page=1>; rel=\"prev\"";
        assert!(has_next_page(paged));
        assert!(!has_next_page(last));
        assert!(!has_next_page("HTTP/2.0 200 OK"));
    }

    #[test]
    fn a_cache_entry_keeps_its_validator_beside_the_body_it_validates() {
        let entry = b"W/\"92ade\"\nname\tvalue\n";
        let (validator, body) = split_validator(entry);
        assert_eq!(validator.as_deref(), Some("W/\"92ade\""));
        assert_eq!(body, b"name\tvalue\n");

        let (missing, whole) = split_validator(b"no newline here");
        assert_eq!(missing, None);
        assert_eq!(whole, b"no newline here");
    }
}
