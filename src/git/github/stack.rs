#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const PULL_REQUEST_STACK_QUERY: &str = concat!(
    "query($owner:String!,$name:String!,$number:Int!){",
    "repository(owner:$owner,name:$name){pullRequest(number:$number){",
    "stackEntry{position}",
    "stack{id number size baseRefName entries(first:100){totalCount nodes{",
    "id position pullRequest{id number title author{login} state isDraft updatedAt url ",
    "baseRefName baseRefOid headRefName headRefOid headRepository{nameWithOwner} ",
    "isCrossRepository additions deletions changedFiles mergeStateStatus mergeable ",
    "reviewDecision mergeQueueEntry{id} commits(last:1){nodes{commit{statusCheckRollup{state}}}}",
    "}}}}}}}"
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestStackSnapshot {
    pub stack: Option<PullRequestStack>,
    pub warnings: Vec<String>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestStack {
    pub node_id: String,
    pub number: u64,
    pub base_ref: String,
    pub size: usize,
    pub selected_position: usize,
    pub members: Vec<PullRequestStackMember>,
    pub truncated: bool,
    pub repository: GitHubRepository,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestStackMember {
    pub node_id: String,
    pub entry_id: String,
    pub position: usize,
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
    pub head_repository: Option<String>,
    pub is_cross_repository: bool,
    pub additions: usize,
    pub deletions: usize,
    pub changed_files: usize,
    pub merge_state: String,
    pub mergeable: String,
    pub review_decision: String,
    pub checks_state: String,
    pub is_queued: bool,
}

impl PullRequestStack {
    pub(crate) fn member(&self, position: usize) -> Option<&PullRequestStackMember> {
        self.members
            .iter()
            .find(|member| member.position == position)
    }

    pub(crate) fn comparison(&self, from: usize, to: usize) -> Result<PullRequest> {
        if from == 0 || to == 0 || from > to {
            bail!("Stack diff positions must form a non-empty bottom-to-top range");
        }
        let floor = self
            .member(from)
            .with_context(|| format!("Stack #{} has no position {from}", self.number))?;
        let ceiling = self
            .member(to)
            .with_context(|| format!("Stack #{} has no position {to}", self.number))?;
        if !is_commit_oid(&floor.base_oid) || !is_commit_oid(&ceiling.head_oid) {
            bail!("Stack members do not contain complete base and head commit IDs");
        }
        Ok(PullRequest {
            number: ceiling.number,
            title: format!(
                "Stack #{}: #{} through #{}",
                self.number, floor.number, ceiling.number
            ),
            description: format!("Composed stack range from position {from} through position {to}"),
            author: ceiling.author.clone(),
            state: ceiling.state.clone(),
            is_draft: ceiling.is_draft,
            created_at: String::new(),
            updated_at: ceiling.updated_at.clone(),
            url: ceiling.url.clone(),
            base_ref: floor.base_ref.clone(),
            base_oid: floor.base_oid.clone(),
            head_ref: ceiling.head_ref.clone(),
            head_oid: ceiling.head_oid.clone(),
            base_repository: self.repository.clone(),
            head_repository: ceiling.head_repository.clone(),
            head_remotes: Vec::new(),
            is_cross_repository: ceiling.is_cross_repository,
            additions: 0,
            deletions: 0,
            changed_files: 0,
            action_state: PullRequestActionState::default(),
        })
    }
}

impl Repository {
    pub(crate) fn pull_request_stack(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<PullRequestStackSnapshot> {
        let repository = &pull_request.base_repository;
        let response = self.checked_cached_gh(
            &format!(
                "pull-request-stack-v1\n{}\n{}",
                repository.url.trim_end_matches('/'),
                pull_request.number
            ),
            CacheLife::Ttl(PULL_REQUEST_CACHE_TTL),
            refresh,
            pull_request_stack_args(repository, pull_request.number)?,
            "unable to load pull-request stack",
        )?;
        let stack = parse_pull_request_stack(&response.data, repository, pull_request.number)?;
        let mut warnings = Vec::new();
        if response.disposition == CacheDisposition::Stale {
            warnings.push(format!(
                "GitHub is unavailable; showing stale cached stack data for #{}",
                pull_request.number
            ));
        }
        Ok(PullRequestStackSnapshot {
            stack,
            warnings,
            from_cache: response.disposition != CacheDisposition::Network,
        })
    }
}

fn pull_request_stack_args(repository: &GitHubRepository, number: u64) -> Result<Vec<OsString>> {
    let (owner, name) = repository
        .name_with_owner
        .split_once('/')
        .context("GitHub repository identity must be OWNER/NAME")?;
    if owner.is_empty() || name.is_empty() {
        bail!("GitHub repository identity must be OWNER/NAME");
    }
    Ok(vec![
        OsString::from("api"),
        OsString::from("graphql"),
        OsString::from("--hostname"),
        OsString::from(repository.host()),
        OsString::from("-f"),
        OsString::from(format!("owner={owner}")),
        OsString::from("-f"),
        OsString::from(format!("name={name}")),
        OsString::from("-F"),
        OsString::from(format!("number={number}")),
        OsString::from("-f"),
        OsString::from(format!("query={PULL_REQUEST_STACK_QUERY}")),
    ])
}

pub(crate) fn parse_pull_request_stack(
    data: &[u8],
    repository: &GitHubRepository,
    selected_number: u64,
) -> Result<Option<PullRequestStack>> {
    let response: StackQueryResponse =
        serde_json::from_slice(data).context("invalid pull-request stack response")?;
    if !response.errors.is_empty() {
        let messages = response
            .errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        bail!("GitHub could not read this pull-request stack: {messages}");
    }
    let pull_request = response
        .data
        .and_then(|value| value.repository)
        .and_then(|value| value.pull_request)
        .context("GitHub did not return the selected pull request")?;
    let Some(stack) = pull_request.stack else {
        return Ok(None);
    };
    let selected_position = pull_request.stack_entry.map(|entry| entry.position);
    let mut members = Vec::with_capacity(stack.entries.nodes.len());
    let mut truncated = stack.size != stack.entries.total_count;
    let mut positions = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    for entry in stack.entries.nodes.into_iter().flatten() {
        let Some(pull_request) = entry.pull_request else {
            truncated = true;
            continue;
        };
        if entry.position == 0
            || !positions.insert(entry.position)
            || !numbers.insert(pull_request.number)
        {
            bail!("GitHub returned duplicate or invalid pull-request stack entries");
        }
        members.push(pull_request.into_member(entry.id, entry.position));
    }
    members.sort_by_key(|member| member.position);
    truncated |= members.len() != stack.size;
    let selected_position = selected_position
        .or_else(|| {
            members
                .iter()
                .find(|member| member.number == selected_number)
                .map(|member| member.position)
        })
        .context("The selected pull request is missing from its stack")?;
    if !members
        .iter()
        .any(|member| member.number == selected_number && member.position == selected_position)
    {
        bail!("GitHub returned inconsistent selected stack membership");
    }
    Ok(Some(PullRequestStack {
        node_id: stack.id,
        number: stack.number,
        base_ref: stack.base_ref_name,
        size: stack.size,
        selected_position,
        members,
        truncated,
        repository: repository.clone(),
    }))
}

#[derive(Deserialize)]
struct StackQueryResponse {
    data: Option<StackQueryData>,
    #[serde(default)]
    errors: Vec<StackQueryError>,
}

#[derive(Deserialize)]
struct StackQueryError {
    message: String,
}

#[derive(Deserialize)]
struct StackQueryData {
    repository: Option<StackQueryRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackQueryRepository {
    pull_request: Option<StackQueryPullRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackQueryPullRequest {
    stack_entry: Option<StackSelectedEntry>,
    stack: Option<StackWire>,
}

#[derive(Deserialize)]
struct StackSelectedEntry {
    position: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackWire {
    id: String,
    number: u64,
    size: usize,
    base_ref_name: String,
    entries: StackEntriesWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackEntriesWire {
    total_count: usize,
    nodes: Vec<Option<StackEntryWire>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackEntryWire {
    id: String,
    position: usize,
    pull_request: Option<StackMemberWire>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackMemberWire {
    id: String,
    number: u64,
    title: String,
    author: Option<StackAuthorWire>,
    state: String,
    is_draft: bool,
    updated_at: String,
    url: String,
    base_ref_name: String,
    base_ref_oid: String,
    head_ref_name: String,
    head_ref_oid: String,
    head_repository: Option<StackRepositoryWire>,
    is_cross_repository: bool,
    additions: usize,
    deletions: usize,
    changed_files: usize,
    merge_state_status: Option<String>,
    mergeable: Option<String>,
    review_decision: Option<String>,
    merge_queue_entry: Option<StackQueueWire>,
    commits: StackCommitsWire,
}

impl StackMemberWire {
    fn into_member(self, entry_id: String, position: usize) -> PullRequestStackMember {
        let checks_state = self
            .commits
            .nodes
            .into_iter()
            .flatten()
            .filter_map(|node| node.commit.status_check_rollup)
            .map(|rollup| rollup.state)
            .next()
            .unwrap_or_default();
        PullRequestStackMember {
            node_id: self.id,
            entry_id,
            position,
            number: self.number,
            title: bounded_text(&self.title, MAX_PULL_REQUEST_TITLE_BYTES),
            author: self.author.map(|author| author.login).unwrap_or_default(),
            state: self.state.to_ascii_uppercase(),
            is_draft: self.is_draft,
            updated_at: self.updated_at,
            url: self.url,
            base_ref: self.base_ref_name,
            base_oid: self.base_ref_oid,
            head_ref: self.head_ref_name,
            head_oid: self.head_ref_oid,
            head_repository: self
                .head_repository
                .map(|repository| repository.name_with_owner),
            is_cross_repository: self.is_cross_repository,
            additions: self.additions,
            deletions: self.deletions,
            changed_files: self.changed_files,
            merge_state: self.merge_state_status.unwrap_or_default(),
            mergeable: self.mergeable.unwrap_or_default(),
            review_decision: self.review_decision.unwrap_or_default(),
            checks_state,
            is_queued: self.merge_queue_entry.is_some(),
        }
    }
}

#[derive(Deserialize)]
struct StackAuthorWire {
    login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackRepositoryWire {
    name_with_owner: String,
}

#[derive(Deserialize)]
struct StackQueueWire {
    #[serde(rename = "id")]
    _id: String,
}

#[derive(Deserialize)]
struct StackCommitsWire {
    nodes: Vec<Option<StackCommitNodeWire>>,
}

#[derive(Deserialize)]
struct StackCommitNodeWire {
    commit: StackCommitWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackCommitWire {
    status_check_rollup: Option<StackCheckRollupWire>,
}

#[derive(Deserialize)]
struct StackCheckRollupWire {
    state: String,
}

#[cfg(test)]
mod query_tests {
    use super::PULL_REQUEST_STACK_QUERY;

    #[test]
    fn graphql_query_has_balanced_delimiters() {
        assert_eq!(
            PULL_REQUEST_STACK_QUERY.matches('{').count(),
            PULL_REQUEST_STACK_QUERY.matches('}').count()
        );
        assert_eq!(
            PULL_REQUEST_STACK_QUERY.matches('(').count(),
            PULL_REQUEST_STACK_QUERY.matches(')').count()
        );
    }
}
