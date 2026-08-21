#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn pull_request_file_document(
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

pub(super) fn count_patch_lines(output: &[u8], marker: u8) -> usize {
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| {
            line.first() == Some(&marker)
                && !line.starts_with(if marker == b'+' { b"+++ " } else { b"--- " })
        })
        .count()
}

pub(super) fn pull_request_view_args(repository: &GitHubRepository, number: u64) -> Vec<OsString> {
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

pub(super) fn parse_pull_requests(
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

pub(super) fn parse_pull_request_fields(
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

pub(super) fn bounded_text(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}…", value.get(..end).unwrap_or_default())
}

pub(super) fn parse_tsv_record<const FIELDS: usize>(record: &[u8]) -> Result<[String; FIELDS]> {
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

pub(super) fn unescape_tsv(value: &str) -> String {
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

pub(super) fn parse_field<T>(value: &str, label: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid {label} `{value}`: {error}"))
}

pub(super) fn select_repository<'a>(
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

pub(super) fn remote_url_for_gh(url: &str) -> String {
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

    if let Some((_, target)) = url.rsplit_once('@')
        && let Some((host, path)) = target.split_once(':')
        && !host.is_empty()
        && !path.is_empty()
    {
        return format!("ssh://{host}/{path}");
    }
    url.to_owned()
}

pub(super) fn repository_from_remote_url(url: &str) -> Option<GitHubRepository> {
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

pub(super) fn group_remote_urls(remote_urls: &[RemoteUrl]) -> Vec<(String, Vec<String>)> {
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

pub(super) fn matching_remotes(
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

pub(super) fn merge_repository(
    repositories: &mut BTreeMap<String, GitHubRepository>,
    mut repository: GitHubRepository,
    remote: Option<&str>,
) {
    let key = repository.url.trim_end_matches('/').to_lowercase();
    let entry = repositories.entry(key).or_insert_with(|| {
        repository.url = repository.url.trim_end_matches('/').to_owned();
        repository
    });
    if let Some(remote) = remote
        && !entry.remotes.iter().any(|existing| existing == remote)
    {
        entry.remotes.push(remote.to_owned());
    }
}

pub(super) fn repository_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    rest.split('/').next().filter(|host| !host.is_empty())
}
