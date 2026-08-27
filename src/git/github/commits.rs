#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const MAX_PULL_REQUEST_COMMITS: usize = 500;
const MAX_COMMIT_PAGES: usize = 5;
const MAX_COMMITS_PER_PAGE: usize = 100;
const MAX_COMMIT_SUBJECT_BYTES: usize = 1024;
const PULL_REQUEST_COMMITS_QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$before:String){repository(owner:$owner,name:$name){pullRequest(number:$number){baseRefOid headRefOid commits(last:100,before:$before){totalCount nodes{commit{oid abbreviatedOid messageHeadline authoredDate committedDate url author{name user{login}} committer{name user{login}}}} pageInfo{hasPreviousPage startCursor}}}}}";

impl Repository {
    pub(crate) fn pull_request_commits(
        &self,
        pull_request: &PullRequest,
    ) -> Result<PullRequestCommits> {
        validate_pull_request_oids(pull_request)?;
        let key = pull_request_commits_cache_key(pull_request);
        if let Some(cached) = cached_pull_request_commits(&key, pull_request) {
            return Ok(cached);
        }
        let pages = self.pull_request_commit_pages(pull_request)?;
        let commits = finish_commit_pages(pull_request, pages)?;
        if let Ok(data) = serde_json::to_vec(&commits) {
            cache_write(&key, &data);
        }
        Ok(commits)
    }

    fn pull_request_commit_pages(
        &self,
        pull_request: &PullRequest,
    ) -> Result<Vec<PullRequestCommitPage>> {
        let mut pages = Vec::new();
        let mut before = None;
        let mut cursors = BTreeSet::new();
        for _ in 0..MAX_COMMIT_PAGES {
            let page = self.pull_request_commit_page(pull_request, before.as_deref())?;
            let previous = page.page_info.has_previous_page;
            let cursor = page.page_info.start_cursor.clone();
            pages.push(page);
            if !previous {
                break;
            }
            let cursor = cursor
                .filter(|value| !value.is_empty())
                .context("GitHub reported more pull-request commits without a pagination cursor")?;
            if !cursors.insert(cursor.clone()) {
                bail!("GitHub repeated a pull-request commit pagination cursor");
            }
            before = Some(cursor);
        }
        Ok(pages)
    }

    fn pull_request_commit_page(
        &self,
        pull_request: &PullRequest,
        before: Option<&str>,
    ) -> Result<PullRequestCommitPage> {
        let (owner, name) = review::repository_parts(pull_request)?;
        let response: PullRequestCommitsQuery = self.graphql(
            pull_request,
            PULL_REQUEST_COMMITS_QUERY,
            &serde_json::json!({
                "owner": owner,
                "name": name,
                "number": pull_request.number,
                "before": before,
            }),
            "unable to load pull-request commits",
        )?;
        response
            .repository
            .context("GitHub did not return the pull-request repository")?
            .pull_request
            .context("GitHub did not return the pull request")
            .map(Into::into)
    }
}

fn validate_pull_request_oids(pull_request: &PullRequest) -> Result<()> {
    if !is_commit_oid(&pull_request.base_oid) || !is_commit_oid(&pull_request.head_oid) {
        bail!("GitHub did not return valid pull-request base and head commits");
    }
    Ok(())
}

fn pull_request_commits_cache_key(pull_request: &PullRequest) -> String {
    format!(
        "pull-request-commits-v1\n{}\n{}\n{}\n{}",
        pull_request.base_repository.url.trim_end_matches('/'),
        pull_request.number,
        pull_request.base_oid,
        pull_request.head_oid
    )
}

fn cached_pull_request_commits(
    key: &str,
    pull_request: &PullRequest,
) -> Option<PullRequestCommits> {
    let data = cache_read(key, CacheLife::Immutable)?;
    let mut commits = serde_json::from_slice::<PullRequestCommits>(&data).ok()?;
    validate_pull_request_commits(pull_request, &commits).ok()?;
    commits.from_cache = true;
    Some(commits)
}

fn finish_commit_pages(
    pull_request: &PullRequest,
    mut pages: Vec<PullRequestCommitPage>,
) -> Result<PullRequestCommits> {
    let first = pages
        .first()
        .context("GitHub returned no pull-request commit pages")?;
    let total_commits = first.total_count;
    let newest_oid = first.commits.last().map(|commit| commit.oid.as_str());
    if total_commits > 0 && newest_oid != Some(pull_request.head_oid.as_str()) {
        bail!("the pull request changed while its commits were loading; retry with --refresh");
    }
    for page in &pages {
        validate_commit_page(pull_request, page, total_commits)?;
    }
    validate_commit_pagination(&pages, total_commits)?;
    pages.reverse();
    let commits = pages
        .into_iter()
        .flat_map(|page| page.commits)
        .collect::<Vec<_>>();
    let result = PullRequestCommits {
        truncated: total_commits > commits.len(),
        commits,
        total_commits,
        base_oid: pull_request.base_oid.clone(),
        head_oid: pull_request.head_oid.clone(),
        from_cache: false,
    };
    validate_pull_request_commits(pull_request, &result)?;
    Ok(result)
}

fn validate_commit_page(
    pull_request: &PullRequest,
    page: &PullRequestCommitPage,
    total_commits: usize,
) -> Result<()> {
    if page.base_oid != pull_request.base_oid
        || page.head_oid != pull_request.head_oid
        || page.total_count != total_commits
    {
        bail!("the pull request changed while its commits were loading; retry with --refresh");
    }
    if page.commits.len() > MAX_COMMITS_PER_PAGE {
        bail!("GitHub returned too many commits in one page");
    }
    Ok(())
}

fn validate_commit_pagination(pages: &[PullRequestCommitPage], total_commits: usize) -> Result<()> {
    if pages.len() > MAX_COMMIT_PAGES {
        bail!("GitHub returned too many pull-request commit pages");
    }
    for page in pages.iter().take(pages.len().saturating_sub(1)) {
        if !page.page_info.has_previous_page {
            bail!("GitHub ended pull-request commit pagination before the final page");
        }
    }
    let fetched = pages.iter().map(|page| page.commits.len()).sum::<usize>();
    let has_older = pages
        .last()
        .is_some_and(|page| page.page_info.has_previous_page);
    if has_older {
        if pages.len() != MAX_COMMIT_PAGES || fetched >= total_commits {
            bail!("GitHub returned inconsistent pull-request commit pagination");
        }
    } else if fetched != total_commits {
        bail!("GitHub returned an incomplete pull-request commit list");
    }
    Ok(())
}

fn validate_pull_request_commits(
    pull_request: &PullRequest,
    commits: &PullRequestCommits,
) -> Result<()> {
    if commits.base_oid != pull_request.base_oid
        || commits.head_oid != pull_request.head_oid
        || commits.commits.len() > MAX_PULL_REQUEST_COMMITS
        || commits.total_commits < commits.commits.len()
        || commits.truncated != (commits.total_commits > commits.commits.len())
    {
        bail!("pull-request commit snapshot is inconsistent with its metadata");
    }
    let mut seen = BTreeSet::new();
    for commit in &commits.commits {
        if !is_commit_oid(&commit.oid)
            || commit.abbreviated_oid.is_empty()
            || !seen.insert(commit.oid.as_str())
        {
            bail!("GitHub returned an invalid pull-request commit list");
        }
    }
    if commits.total_commits > 0
        && commits.commits.last().map(|commit| commit.oid.as_str())
            != Some(pull_request.head_oid.as_str())
    {
        bail!("pull-request commits do not end at the current head");
    }
    Ok(())
}

fn pull_request_commit(value: PullRequestCommitQuery) -> PullRequestCommit {
    let (author, author_login) = actor_identity(value.author)
        .or_else(|| actor_identity(value.committer))
        .unwrap_or_else(|| ("ghost".to_owned(), None));
    PullRequestCommit {
        oid: value.oid,
        abbreviated_oid: value.abbreviated_oid,
        subject: bounded_text(&value.message_headline, MAX_COMMIT_SUBJECT_BYTES),
        author,
        author_login,
        authored_at: value.authored_date,
        committed_at: value.committed_date,
        url: value.url,
    }
}

fn actor_identity(actor: Option<PullRequestCommitActor>) -> Option<(String, Option<String>)> {
    let actor = actor?;
    let login = actor
        .user
        .map(|user| user.login)
        .filter(|value| !value.is_empty());
    let name = actor
        .name
        .filter(|value| !value.is_empty())
        .or_else(|| login.clone())?;
    Some((name, login))
}

#[derive(Deserialize)]
struct PullRequestCommitsQuery {
    repository: Option<PullRequestCommitsRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCommitsRepository {
    pull_request: Option<PullRequestCommitsQueryValue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCommitsQueryValue {
    base_ref_oid: String,
    head_ref_oid: String,
    commits: PullRequestCommitsConnection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCommitsConnection {
    total_count: usize,
    nodes: Vec<PullRequestCommitNode>,
    page_info: PullRequestCommitPageInfo,
}

#[derive(Deserialize)]
struct PullRequestCommitNode {
    commit: PullRequestCommitQuery,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCommitQuery {
    oid: String,
    abbreviated_oid: String,
    message_headline: String,
    authored_date: String,
    committed_date: String,
    url: String,
    author: Option<PullRequestCommitActor>,
    committer: Option<PullRequestCommitActor>,
}

#[derive(Deserialize)]
struct PullRequestCommitActor {
    name: Option<String>,
    user: Option<PullRequestCommitUser>,
}

#[derive(Deserialize)]
struct PullRequestCommitUser {
    login: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestCommitPageInfo {
    has_previous_page: bool,
    start_cursor: Option<String>,
}

struct PullRequestCommitPage {
    base_oid: String,
    head_oid: String,
    total_count: usize,
    commits: Vec<PullRequestCommit>,
    page_info: PullRequestCommitPageInfo,
}

impl From<PullRequestCommitsQueryValue> for PullRequestCommitPage {
    fn from(value: PullRequestCommitsQueryValue) -> Self {
        Self {
            base_oid: value.base_ref_oid,
            head_oid: value.head_ref_oid,
            total_count: value.commits.total_count,
            commits: value
                .commits
                .nodes
                .into_iter()
                .map(|node| pull_request_commit(node.commit))
                .collect(),
            page_info: value.commits.page_info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_returned_oldest_to_newest() {
        let request = pull_request("c", "d");
        let mut newest = page(&request, 4, &["c", "d"]);
        newest.page_info.has_previous_page = true;
        newest.page_info.start_cursor = Some("older".to_owned());
        let result =
            finish_commit_pages(&request, vec![newest, page(&request, 4, &["a", "b"])]).unwrap();

        assert_eq!(
            result
                .commits
                .iter()
                .map(|commit| commit.abbreviated_oid.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "d"]
        );
        assert!(!result.truncated);
    }

    #[test]
    fn inconsistent_pages_and_duplicate_commits_are_rejected() {
        let request = pull_request("a", "b");
        let mut changed = page(&request, 2, &["a", "b"]);
        changed.base_oid = oid("c");
        let changed_error = finish_commit_pages(&request, vec![changed]).unwrap_err();
        assert!(changed_error.to_string().contains("changed"));

        let duplicate = page(&request, 2, &["b", "b"]);
        let duplicate_error = finish_commit_pages(&request, vec![duplicate]).unwrap_err();
        assert!(duplicate_error.to_string().contains("invalid"));

        let incomplete = page(&request, 2, &["b"]);
        let incomplete_error = finish_commit_pages(&request, vec![incomplete]).unwrap_err();
        assert!(incomplete_error.to_string().contains("incomplete"));
    }

    #[test]
    fn cache_identity_includes_repository_number_and_both_commits() {
        let request = pull_request("a", "b");
        let key = pull_request_commits_cache_key(&request);
        assert!(key.contains("https://github.com/acme/widget\n42"));
        assert!(key.contains(&oid("a")));
        assert!(key.ends_with(&oid("b")));
    }

    #[test]
    fn actors_fall_back_from_names_to_logins_and_committers() {
        let author = PullRequestCommitActor {
            name: None,
            user: Some(PullRequestCommitUser {
                login: "octocat".to_owned(),
            }),
        };
        assert_eq!(
            actor_identity(Some(author)),
            Some(("octocat".to_owned(), Some("octocat".to_owned())))
        );
    }

    fn pull_request(base: &str, head: &str) -> PullRequest {
        PullRequest {
            number: 42,
            base_oid: oid(base),
            head_oid: oid(head),
            base_repository: GitHubRepository {
                name_with_owner: "acme/widget".to_owned(),
                url: "https://github.com/acme/widget".to_owned(),
                remotes: Vec::new(),
            },
            ..PullRequest::default()
        }
    }

    fn page(
        pull_request: &PullRequest,
        total_count: usize,
        abbreviated_oids: &[&str],
    ) -> PullRequestCommitPage {
        PullRequestCommitPage {
            base_oid: pull_request.base_oid.clone(),
            head_oid: pull_request.head_oid.clone(),
            total_count,
            commits: abbreviated_oids
                .iter()
                .map(|value| PullRequestCommit {
                    oid: oid(value),
                    abbreviated_oid: (*value).to_owned(),
                    ..PullRequestCommit::default()
                })
                .collect(),
            page_info: PullRequestCommitPageInfo {
                has_previous_page: false,
                start_cursor: None,
            },
        }
    }

    fn oid(value: &str) -> String {
        value.repeat(40)
    }
}
