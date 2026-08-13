use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use super::diff::{DiffDocument, PullRequestDetails, parse_diff};
use super::{MAX_DIFF_BYTES, Repository, command_error, text, trim_ascii, truncate};

const MAX_GIT_REMOTES: usize = 32;
const MAX_REMOTE_URL_ENTRIES: usize = 64;
const MAX_REMOTE_URLS: usize = 32;
const MAX_GITHUB_REPOSITORIES: usize = 16;
const MAX_PULL_REQUESTS_PER_REPOSITORY: usize = 100;
const MAX_PULL_REQUESTS: usize = 500;
const PULL_REQUEST_FIELDS: &str = "number,title,author,state,isDraft,updatedAt,url,baseRefName,headRefName,headRepository,isCrossRepository,additions,deletions,changedFiles";
const PULL_REQUEST_TSV_JQ: &str = r#".[] | [(.number|tostring), .title, (.author.login // "ghost"), .state, (.isDraft|tostring), .updatedAt, .url, .baseRefName, .headRefName, (.headRepository.nameWithOwner // ""), (.isCrossRepository|tostring), (.additions|tostring), (.deletions|tostring), (.changedFiles|tostring)] | @tsv"#;
const REPOSITORY_TSV_TEMPLATE: &str = "{{.nameWithOwner}}{{\"\\t\"}}{{.url}}{{\"\\n\"}}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
    pub name_with_owner: String,
    pub url: String,
    pub remotes: Vec<String>,
}

impl GitHubRepository {
    pub fn selector(&self) -> &str {
        // Passing the canonical URL makes the target unambiguous when a worktree has
        // several GitHub remotes or remotes on different GitHub Enterprise hosts.
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
    pub head_ref: String,
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

    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        query.is_empty()
            || self.title.to_lowercase().contains(&query)
            || self.author.to_lowercase().contains(&query)
            || self.number.to_string().contains(&query)
            || self
                .base_repository
                .name_with_owner
                .to_lowercase()
                .contains(&query)
            || self.base_repository.url.to_lowercase().contains(&query)
            || self.base_ref.to_lowercase().contains(&query)
            || self.head_ref.to_lowercase().contains(&query)
            || self
                .head_repository
                .as_ref()
                .is_some_and(|repository| repository.to_lowercase().contains(&query))
            || self
                .base_repository
                .remotes
                .iter()
                .chain(&self.head_remotes)
                .any(|remote| remote.to_lowercase().contains(&query))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PullRequestSnapshot {
    pub repositories: Vec<GitHubRepository>,
    pub pull_requests: Vec<PullRequest>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteUrl {
    remote: String,
    url: String,
}

impl Repository {
    pub fn pull_requests(&self) -> Result<PullRequestSnapshot> {
        self.ensure_gh_available()?;
        let (repositories, mut warnings) = self.github_repositories()?;
        let mut pull_requests = Vec::new();
        let mut successful_repositories = 0;
        let mut load_errors = Vec::new();

        for repository in &repositories {
            let output = self.run_gh(pull_request_list_args(repository))?;
            if !output.status.success() {
                let error = format!(
                    "{}: {}",
                    repository.name_with_owner,
                    command_error("unable to list pull requests", &output)
                );
                warnings.push(error.clone());
                load_errors.push(error);
                continue;
            }

            match parse_pull_requests(&output.stdout, repository, &repositories) {
                Ok(mut requests) => {
                    successful_repositories += 1;
                    pull_requests.append(&mut requests);
                }
                Err(error) => {
                    let error = format!(
                        "{}: unable to parse GitHub CLI output: {error}",
                        repository.name_with_owner
                    );
                    warnings.push(error.clone());
                    load_errors.push(error);
                }
            }
        }

        if successful_repositories == 0 {
            let detail = load_errors
                .first()
                .map(String::as_str)
                .unwrap_or("no repository returned a result");
            bail!("Unable to load pull requests: {detail}");
        }

        sort_pull_requests(&mut pull_requests);
        if pull_requests.len() > MAX_PULL_REQUESTS {
            pull_requests.truncate(MAX_PULL_REQUESTS);
            warnings.push(format!(
                "Showing the first {MAX_PULL_REQUESTS} pull requests across all remotes"
            ));
        }

        Ok(PullRequestSnapshot {
            repositories,
            pull_requests,
            warnings,
        })
    }

    pub fn pull_request_diff(&self, pull_request: &PullRequest) -> Result<DiffDocument> {
        self.ensure_gh_available()?;
        let output = self.checked_gh(pull_request_diff_args(pull_request))?;
        let mut output = output;
        let truncated = truncate(&mut output, MAX_DIFF_BYTES);
        Ok(pull_request_document(&output, pull_request, truncated))
    }

    fn ensure_gh_available(&self) -> Result<()> {
        let output = self.run_gh([OsString::from("--version")])?;
        if !output.status.success() {
            bail!(
                "{}",
                command_error("GitHub CLI (`gh`) is unavailable", &output)
            );
        }
        Ok(())
    }

    fn github_repositories(&self) -> Result<(Vec<GitHubRepository>, Vec<String>)> {
        let (remote_urls, mut warnings) = self.remote_urls()?;
        let grouped_remote_urls = group_remote_urls(&remote_urls);
        let mut repositories = BTreeMap::new();

        for (index, (url, remotes)) in grouped_remote_urls.iter().take(MAX_REMOTE_URLS).enumerate()
        {
            match self.resolve_github_repository(Some(url)) {
                Ok(repository) => {
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

        // `gh` can infer a repository from GH_REPO or Git configuration. This also
        // provides a useful fallback for repositories without a conventional remote.
        if repositories.is_empty() {
            match self.resolve_github_repository(None) {
                Ok(repository) => merge_repository(&mut repositories, repository, None),
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
        repositories.sort_by_cached_key(|repository| repository.name_with_owner.to_lowercase());
        for repository in &mut repositories {
            repository.remotes.sort();
            repository.remotes.dedup();
        }
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

    fn resolve_github_repository(&self, url: Option<&str>) -> Result<GitHubRepository> {
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
        let output = self.run_gh(args)?;
        if !output.status.success() {
            bail!("{}", command_error("gh repo view failed", &output));
        }
        let record = trim_ascii(&output.stdout);
        let fields = parse_tsv_record(record, 2).context("invalid gh repo view output")?;
        if fields[0].trim().is_empty() || fields[1].trim().is_empty() {
            bail!("gh repo view returned an incomplete repository identity");
        }
        Ok(GitHubRepository {
            name_with_owner: fields[0].clone(),
            url: fields[1].clone(),
            remotes: Vec::new(),
        })
    }

    fn checked_gh<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run_gh(args)?;
        if !output.status.success() {
            bail!("{}", command_error("GitHub CLI command failed", &output));
        }
        Ok(output.stdout)
    }

    fn run_gh<I, S>(&self, args: I) -> Result<Output>
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
        command.output().with_context(|| {
            format!(
                "failed to execute GitHub CLI (`gh`) in {}; install it and run `gh auth login`",
                self.root.display()
            )
        })
    }
}

fn pull_request_document(
    output: &[u8],
    pull_request: &PullRequest,
    truncated: bool,
) -> DiffDocument {
    let mut document = parse_diff(
        output,
        format!(
            "PR #{} — {}  ·  {} → {}",
            pull_request.number,
            pull_request.title,
            pull_request.head_label(),
            pull_request.base_label()
        ),
        None,
        truncated,
    );
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
    });
    document
}

fn pull_request_list_args(repository: &GitHubRepository) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("list"),
        OsString::from("--repo"),
        OsString::from(repository.selector()),
        OsString::from("--state"),
        OsString::from("open"),
        OsString::from("--limit"),
        OsString::from(MAX_PULL_REQUESTS_PER_REPOSITORY.to_string()),
        OsString::from("--json"),
        OsString::from(PULL_REQUEST_FIELDS),
        OsString::from("--jq"),
        OsString::from(PULL_REQUEST_TSV_JQ),
    ]
}

fn pull_request_diff_args(pull_request: &PullRequest) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("diff"),
        OsString::from(pull_request.number.to_string()),
        OsString::from("--repo"),
        OsString::from(pull_request.base_repository.selector()),
        OsString::from("--color=never"),
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
        let fields = parse_tsv_record(record, 14)
            .with_context(|| format!("invalid pull-request record {}", index + 1))?;
        let head_repository = (!fields[9].is_empty()).then(|| fields[9].clone());
        let head_remotes = head_repository
            .as_deref()
            .map(|name| matching_remotes(repositories, base_repository.host(), name))
            .unwrap_or_default();
        pull_requests.push(PullRequest {
            number: parse_field(&fields[0], "number")?,
            title: fields[1].clone(),
            author: fields[2].clone(),
            state: fields[3].clone(),
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
        });
    }
    Ok(pull_requests)
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

    // Convert SCP-like SSH URLs to a credential-free URL before putting one on
    // the `gh` process command line. This also handles nonstandard SSH usernames.
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

fn sort_pull_requests(pull_requests: &mut [PullRequest]) {
    pull_requests.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| {
                left.base_repository
                    .name_with_owner
                    .cmp(&right.base_repository.name_with_owner)
            })
            .then_with(|| right.number.cmp(&left.number))
    });
}

fn repository_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    rest.split('/').next().filter(|host| !host.is_empty())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn git(&self, args: &[&str]) {
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
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_directory() -> TestDirectory {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("quinjet-github-test-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        let directory = TestDirectory(path);
        directory.git(&["init", "--initial-branch=main"]);
        directory
    }

    fn repository(name: &str, url: &str, remotes: &[&str]) -> GitHubRepository {
        GitHubRepository {
            name_with_owner: name.to_owned(),
            url: url.to_owned(),
            remotes: remotes.iter().map(|remote| (*remote).to_owned()).collect(),
        }
    }

    #[test]
    fn discovers_distinct_fetch_and_push_repositories_for_each_remote() {
        let directory = test_directory();
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
        assert_eq!(
            urls,
            vec![
                RemoteUrl {
                    remote: "origin".to_owned(),
                    url: "git@github.com:octocat/widget.git".to_owned(),
                },
                RemoteUrl {
                    remote: "origin".to_owned(),
                    url: "https://github.com/acme/widget.git".to_owned(),
                },
                RemoteUrl {
                    remote: "upstream".to_owned(),
                    url: "https://github.com/acme/widget.git".to_owned(),
                },
            ]
        );
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
    fn groups_duplicate_remote_urls_without_losing_aliases() {
        let grouped = group_remote_urls(&[
            RemoteUrl {
                remote: "upstream".to_owned(),
                url: "https://github.com/acme/widget.git".to_owned(),
            },
            RemoteUrl {
                remote: "origin".to_owned(),
                url: "https://user:secret@github.com/acme/widget.git".to_owned(),
            },
            RemoteUrl {
                remote: "publish".to_owned(),
                url: "git@github.com:octocat/widget.git".to_owned(),
            },
        ]);

        assert_eq!(
            grouped,
            vec![
                (
                    "https://github.com/acme/widget.git".to_owned(),
                    vec!["origin".to_owned(), "upstream".to_owned()]
                ),
                (
                    "ssh://github.com/octocat/widget.git".to_owned(),
                    vec!["publish".to_owned()]
                ),
            ]
        );
    }

    #[test]
    fn parses_cross_repository_pull_requests_and_maps_both_remotes() {
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
        let output = b"42\tShip the rocket\toctocat\tOPEN\ttrue\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t12\t3\t4\n";

        let requests = parse_pull_requests(output, &upstream, &[upstream.clone(), fork]).unwrap();

        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.base_label(), "acme/widget:main");
        assert_eq!(request.head_label(), "octocat/widget:feature/rocket");
        assert_eq!(request.base_repository.remotes, vec!["upstream"]);
        assert_eq!(request.head_remotes, vec!["origin", "publish"]);
        assert!(request.is_cross_repository);
        assert!(request.is_draft);
        assert_eq!(
            (request.changed_files, request.additions, request.deletions),
            (4, 12, 3)
        );
    }

    #[test]
    fn builds_a_typed_multi_file_diff_with_cross_remote_details() {
        let upstream = repository(
            "acme/widget",
            "https://github.com/acme/widget",
            &["upstream"],
        );
        let fork = repository(
            "octocat/widget",
            "https://github.com/octocat/widget",
            &["origin"],
        );
        let output = b"42\tShip the rocket\toctocat\tOPEN\tfalse\t2026-08-13T12:00:00Z\thttps://github.com/acme/widget/pull/42\tmain\tfeature/rocket\toctocat/widget\ttrue\t2\t1\t2\n";
        let request = parse_pull_requests(output, &upstream, &[upstream.clone(), fork])
            .unwrap()
            .remove(0);
        let patch = b"diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -0,0 +1 @@\n+fn main() {}\n";

        let document = pull_request_document(patch, &request, true);

        assert_eq!(document.file_count(), 2);
        assert_eq!(
            (document.addition_count(), document.deletion_count()),
            (2, 1)
        );
        assert!(document.truncated);
        assert!(document.title.contains("octocat/widget:feature/rocket"));
        assert!(document.title.contains("acme/widget:main"));
        let details = document.pull_request_details.unwrap();
        assert_eq!(details.base_remotes, vec!["upstream"]);
        assert_eq!(details.head_remotes, vec!["origin"]);
        assert!(details.is_cross_repository);
        assert_eq!(
            (details.changed_files, details.additions, details.deletions),
            (2, 2, 1)
        );
    }

    #[test]
    fn same_repository_heads_use_the_base_remote_aliases() {
        let base = repository(
            "acme/widget",
            "https://github.com/acme/widget",
            &["origin", "upstream"],
        );
        let output = b"9\tLocal topic\tada\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.com/acme/widget/pull/9\tmain\ttopic\tacme/widget\tfalse\t0\t0\t0\n";

        let requests = parse_pull_requests(output, &base, std::slice::from_ref(&base)).unwrap();

        assert_eq!(requests[0].head_label(), "topic");
        assert_eq!(requests[0].head_remotes, vec!["origin", "upstream"]);
        assert!(!requests[0].is_cross_repository);
    }

    #[test]
    fn rejects_malformed_pull_request_output_without_panicking() {
        let base = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);

        assert!(parse_pull_requests(b"not tsv", &base, std::slice::from_ref(&base)).is_err());
        assert!(
            parse_pull_requests(
                b"not-a-number\ttitle\tuser\tOPEN\tfalse\tdate\turl\tmain\ttopic\trepo\tfalse\t0\t0\t1\n",
                &base,
                std::slice::from_ref(&base)
            )
            .is_err()
        );
    }

    #[test]
    fn handles_deleted_forks_and_ghost_authors() {
        let base = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["enterprise"],
        );
        let output = b"7\tOld contribution\tghost\tOPEN\tfalse\t2026-01-01T00:00:00Z\thttps://github.example.com/acme/widget/pull/7\ttrunk\tlost-branch\t\tfalse\t0\t0\t0\n";

        let requests = parse_pull_requests(output, &base, std::slice::from_ref(&base)).unwrap();

        assert_eq!(requests[0].author, "ghost");
        assert_eq!(requests[0].head_label(), "deleted fork:lost-branch");
        assert!(requests[0].head_remotes.is_empty());
        assert_eq!(base.host(), "github.example.com");
        assert_eq!(base.display_name(), "github.example.com/acme/widget");
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
        merge_repository(
            &mut repositories,
            repository("acme/widget", "https://github.com/acme/widget", &[]),
            Some("origin"),
        );

        let repository = repositories.into_values().next().unwrap();
        assert_eq!(repository.url, "https://github.com/acme/widget");
        assert_eq!(repository.remotes, vec!["origin", "upstream"]);
    }

    #[test]
    fn list_command_is_bounded_and_explicitly_targets_the_repository() {
        let repository = repository(
            "acme/widget",
            "https://github.example.com/acme/widget",
            &["work"],
        );

        let args = pull_request_list_args(&repository);
        let args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();

        assert_eq!(
            args[0..4],
            ["pr", "list", "--repo", repository.url.as_str()]
        );
        assert!(args.windows(2).any(|pair| pair == ["--state", "open"]));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "--limit" && pair[1] == MAX_PULL_REQUESTS_PER_REPOSITORY.to_string().as_str()
        }));
    }

    #[test]
    fn diff_command_always_targets_the_base_repository_url() {
        let request = PullRequest {
            number: 19,
            title: "Cross-fork change".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            updated_at: String::new(),
            url: "https://github.com/acme/widget/pull/19".to_owned(),
            base_ref: "main".to_owned(),
            head_ref: "topic".to_owned(),
            base_repository: repository(
                "acme/widget",
                "https://github.com/acme/widget",
                &["upstream"],
            ),
            head_repository: Some("octocat/widget".to_owned()),
            head_remotes: vec!["origin".to_owned()],
            is_cross_repository: true,
            additions: 0,
            deletions: 0,
            changed_files: 0,
        };

        let args = pull_request_diff_args(&request);
        let args: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        assert_eq!(
            args,
            [
                "pr",
                "diff",
                "19",
                "--repo",
                "https://github.com/acme/widget",
                "--color=never"
            ]
        );
        assert!(!args.iter().any(|arg| arg.contains("octocat/widget")));
    }

    #[test]
    fn pull_request_filter_covers_identity_branches_and_remotes() {
        let request = PullRequest {
            number: 42,
            title: "Ship the rocket".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            updated_at: String::new(),
            url: String::new(),
            base_ref: "main".to_owned(),
            head_ref: "feature/rocket".to_owned(),
            base_repository: repository(
                "acme/widget",
                "https://github.com/acme/widget",
                &["upstream"],
            ),
            head_repository: Some("octocat/widget".to_owned()),
            head_remotes: vec!["origin".to_owned()],
            is_cross_repository: true,
            additions: 0,
            deletions: 0,
            changed_files: 0,
        };

        for query in [
            "rocket",
            "octocat",
            "42",
            "acme/widget",
            "main",
            "feature",
            "upstream",
            "origin",
        ] {
            assert!(request.matches(query), "query {query:?} should match");
        }
        assert!(!request.matches("unrelated"));
    }

    #[test]
    fn sorts_most_recent_requests_first_with_stable_ties() {
        let base = repository("z/repo", "https://github.com/z/repo", &[]);
        let mut requests = [3_u64, 1, 2]
            .into_iter()
            .map(|number| PullRequest {
                number,
                title: String::new(),
                author: String::new(),
                state: String::new(),
                is_draft: false,
                updated_at: if number == 1 {
                    "2026-09-01T00:00:00Z"
                } else {
                    "2026-08-01T00:00:00Z"
                }
                .to_owned(),
                url: String::new(),
                base_ref: String::new(),
                head_ref: String::new(),
                base_repository: base.clone(),
                head_repository: None,
                head_remotes: Vec::new(),
                is_cross_repository: false,
                additions: 0,
                deletions: 0,
                changed_files: 0,
            })
            .collect::<Vec<_>>();

        sort_pull_requests(&mut requests);

        assert_eq!(
            requests
                .iter()
                .map(|request| request.number)
                .collect::<Vec<_>>(),
            vec![1, 3, 2]
        );
    }
}
