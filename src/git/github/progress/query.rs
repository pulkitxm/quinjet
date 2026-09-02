#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

const VIEWER_REVIEW_QUERY: &str = "
query($owner: String!, $name: String!, $number: Int!) {
  viewer { login }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      latestReviews(last: 50) {
        nodes { state submittedAt author { login } commit { oid } }
      }
    }
  }
}
";

#[derive(Deserialize)]
pub(super) struct ViewerReviewData {
    pub(super) viewer: Option<ViewerNode>,
    pub(super) repository: Option<ViewerRepositoryNode>,
}

#[derive(Deserialize)]
pub(super) struct ViewerNode {
    #[serde(default)]
    pub(super) login: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ViewerRepositoryNode {
    pub(super) pull_request: Option<ViewerPullRequestNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ViewerPullRequestNode {
    pub(super) latest_reviews: Option<ViewerReviewConnection>,
}

#[derive(Deserialize)]
pub(super) struct ViewerReviewConnection {
    #[serde(default)]
    pub(super) nodes: Vec<Option<ViewerReviewNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ViewerReviewNode {
    #[serde(default)]
    pub(super) state: String,
    #[serde(default)]
    pub(super) submitted_at: String,
    pub(super) author: Option<ViewerNode>,
    pub(super) commit: Option<ViewerCommitNode>,
}

#[derive(Deserialize)]
pub(super) struct ViewerCommitNode {
    #[serde(default)]
    pub(super) oid: String,
}

#[doc = " Who is reading, and the commit their newest review was written against."]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ViewerReviewMark {
    pub login: String,
    pub commit_oid: String,
    pub state: String,
    pub submitted_at: String,
}

impl Repository {
    #[doc = " The reader's own identity and last review, which is what makes"]
    #[doc = " `--since-review` mean the same thing to two different reviewers on"]
    #[doc = " the same pull request."]
    pub(crate) fn pull_request_viewer_review(
        &self,
        pull_request: &PullRequest,
    ) -> Result<ViewerReviewMark> {
        let (owner, name) = super::super::review::repository_parts(pull_request)?;
        let data: ViewerReviewData = self.graphql(
            pull_request,
            VIEWER_REVIEW_QUERY,
            &json!({ "owner": owner, "name": name, "number": pull_request.number }),
            "unable to read your review history for this pull request",
        )?;
        let login = data.viewer.map(|viewer| viewer.login).unwrap_or_default();
        let reviews = data
            .repository
            .and_then(|repository| repository.pull_request)
            .and_then(|request| request.latest_reviews)
            .map(|reviews| reviews.nodes)
            .unwrap_or_default();
        let mine = reviews
            .into_iter()
            .flatten()
            .filter(|review| {
                review
                    .author
                    .as_ref()
                    .is_some_and(|author| author.login.eq_ignore_ascii_case(&login))
            })
            .max_by(|left, right| left.submitted_at.cmp(&right.submitted_at));
        Ok(ViewerReviewMark {
            login,
            commit_oid: mine
                .as_ref()
                .and_then(|review| review.commit.as_ref())
                .map(|commit| commit.oid.clone())
                .unwrap_or_default(),
            state: mine
                .as_ref()
                .map(|review| review.state.clone())
                .unwrap_or_default(),
            submitted_at: mine.map(|review| review.submitted_at).unwrap_or_default(),
        })
    }

    #[doc = " The paths that changed between two commits. Local Git answers when the"]
    #[doc = " checkout already holds both, which costs nothing; otherwise GitHub's"]
    #[doc = " comparison does. Both sides are immutable, so the answer is cached"]
    #[doc = " forever and a new commit asks a different question."]
    pub(super) fn changed_paths_between(
        &self,
        repository: &GitHubRepository,
        from: &str,
        to: &str,
    ) -> Option<BTreeSet<PathBuf>> {
        if !is_commit_oid(from) || !is_commit_oid(to) {
            return None;
        }
        if from == to {
            return Some(BTreeSet::new());
        }
        let key = format!(
            "pr-changed-paths-v1\n{}\n{from}\n{to}",
            repository.url.trim_end_matches('/')
        );
        if let Some(cached) =
            super::super::cache_read_bounded(&key, CacheLife::Immutable, MAX_COMPARE_PATH_BYTES)
        {
            return Some(decode_paths(&cached));
        }
        let encoded = self
            .local_changed_paths(from, to)
            .or_else(|| self.api_changed_paths(repository, from, to))?;
        super::super::cache_write_bounded(&key, &encoded, MAX_COMPARE_PATH_BYTES);
        Some(decode_paths(&encoded))
    }

    fn local_changed_paths(&self, from: &str, to: &str) -> Option<Vec<u8>> {
        if !self.has_commit(from) || !self.has_commit(to) {
            return None;
        }
        let output = self
            .checked([
                OsString::from("diff"),
                OsString::from("--name-only"),
                OsString::from("-z"),
                OsString::from(from),
                OsString::from(to),
            ])
            .ok()?;
        Some(output)
    }

    fn api_changed_paths(
        &self,
        repository: &GitHubRepository,
        from: &str,
        to: &str,
    ) -> Option<Vec<u8>> {
        if repository.name_with_owner.is_empty() {
            return None;
        }
        let output = self
            .run_gh([
                OsString::from("api"),
                OsString::from(format!(
                    "repos/{}/compare/{from}...{to}?per_page=100",
                    repository.name_with_owner
                )),
                OsString::from("--jq"),
                OsString::from(".files[]?.filename"),
            ])
            .ok()?;
        if !output.status.success() || output.stdout_truncated {
            return None;
        }
        let mut encoded = Vec::new();
        for line in text(trim_ascii(&output.stdout)).lines() {
            if line.is_empty() {
                continue;
            }
            encoded.extend_from_slice(line.as_bytes());
            encoded.push(0);
        }
        Some(encoded)
    }
}

fn decode_paths(encoded: &[u8]) -> BTreeSet<PathBuf> {
    encoded
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| PathBuf::from(text(entry)))
        .collect()
}
