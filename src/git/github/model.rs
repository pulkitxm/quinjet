#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    #[serde(flatten)]
    pub action_state: PullRequestActionState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "GitHub exposes independent viewer permissions for each pull-request action"
)]
pub(crate) struct PullRequestActionState {
    pub node_id: String,
    pub is_locked: bool,
    pub viewer_can_close: bool,
    pub viewer_can_reopen: bool,
    pub viewer_can_update: bool,
    pub viewer_can_update_branch: bool,
    pub viewer_can_subscribe: bool,
    pub viewer_can_react: bool,
    pub viewer_did_author: bool,
    pub viewer_subscription: String,
    pub merge_state: String,
    pub mergeable: String,
    pub maintainer_can_modify: bool,
    pub viewer_can_merge_as_admin: bool,
    pub auto_merge_method: String,
    pub merge_queue_entry_id: String,
    pub merge_queue_position: usize,
    pub merge_queue_state: String,
    pub merge_queue_id: String,
    pub review_decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecentPullRequest {
    pub number: u64,
    pub title: String,
    pub updated_at: String,
    pub repository: GitHubRepository,
}

impl From<&PullRequest> for RecentPullRequest {
    fn from(pull_request: &PullRequest) -> Self {
        Self {
            number: pull_request.number,
            title: pull_request.title.clone(),
            updated_at: pull_request.updated_at.clone(),
            repository: pull_request.base_repository.clone(),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestSnapshot {
    pub repositories: Vec<GitHubRepository>,
    pub selected_repository: Option<GitHubRepository>,
    pub pull_request: PullRequest,
    pub warnings: Vec<String>,
    pub exact_number: Option<u64>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestCommit {
    pub oid: String,
    pub abbreviated_oid: String,
    pub subject: String,
    pub author: String,
    pub author_login: Option<String>,
    pub authored_at: String,
    pub committed_at: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestCommits {
    pub commits: Vec<PullRequestCommit>,
    pub total_commits: usize,
    pub truncated: bool,
    pub base_oid: String,
    pub head_oid: String,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: PullRequestFileStatus,
    pub counts: Option<DiffLineCounts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDiffIndex {
    pub files: Vec<PullRequestFile>,
    pub total_files: usize,
    pub truncated: bool,
}
