#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " One page of 100 rollup contexts and 100 review threads bounds the"]
#[doc = " response; anything past that sets the truncation flags the verdict"]
#[doc = " reports rather than silently dropping a requirement."]
const GATE_QUERY: &str = "
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      number
      title
      url
      state
      isDraft
      merged
      mergeable
      mergeStateStatus
      baseRefName
      baseRefOid
      headRefOid
      reviewDecision
      mergeQueueEntry { position state }
      autoMergeRequest { mergeMethod enabledBy { login } }
      reviewRequests(first: 50) {
        totalCount
        nodes {
          requestedReviewer {
            __typename
            ... on User { login }
            ... on Team { name }
            ... on Mannequin { login }
          }
        }
      }
      latestOpinionatedReviews(first: 50) {
        nodes { state author { login } commit { oid } }
      }
      reviewThreads(first: 100) {
        totalCount
        pageInfo { hasNextPage }
        nodes { isResolved isOutdated }
      }
      commits(last: 1) {
        nodes {
          commit {
            oid
            statusCheckRollup {
              state
              contexts(first: 100) {
                totalCount
                pageInfo { hasNextPage }
                nodes {
                  __typename
                  ... on CheckRun {
                    name
                    status
                    conclusion
                    detailsUrl
                    isRequired(pullRequestNumber: $number)
                    checkSuite { workflowRun { workflow { name } } }
                  }
                  ... on StatusContext {
                    context
                    state
                    targetUrl
                    isRequired(pullRequestNumber: $number)
                  }
                }
              }
            }
          }
        }
      }
      baseRef {
        name
        target { oid }
        refUpdateRule {
          requiredApprovingReviewCount
          requiredStatusCheckContexts
          requiresCodeOwnerReviews
          requiresConversationResolution
          requiresLinearHistory
          requiresSignatures
        }
      }
    }
  }
}
";

#[derive(Deserialize, Serialize)]
pub(super) struct GateQueryData {
    pub(super) repository: Option<GateRepositoryNode>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateRepositoryNode {
    pub(super) pull_request: Option<GatePullRequestNode>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GatePullRequestNode {
    pub(super) number: u64,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    pub(super) url: String,
    #[serde(default)]
    pub(super) state: String,
    #[serde(default)]
    pub(super) is_draft: bool,
    #[serde(default)]
    pub(super) merged: bool,
    pub(super) mergeable: Option<String>,
    pub(super) merge_state_status: Option<String>,
    #[serde(default)]
    pub(super) base_ref_name: String,
    #[serde(default)]
    pub(super) base_ref_oid: String,
    #[serde(default)]
    pub(super) head_ref_oid: String,
    pub(super) review_decision: Option<String>,
    pub(super) merge_queue_entry: Option<GateQueueEntry>,
    pub(super) auto_merge_request: Option<GateAutoMergeNode>,
    pub(super) review_requests: Option<GateReviewRequests>,
    pub(super) latest_opinionated_reviews: Option<GateReviewConnection>,
    pub(super) review_threads: Option<GateThreadConnection>,
    pub(super) commits: Option<GateCommitConnection>,
    pub(super) base_ref: Option<GateBaseRef>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateQueueEntry {
    #[serde(default)]
    pub(super) position: usize,
    #[serde(default)]
    pub(super) state: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateAutoMergeNode {
    pub(super) merge_method: Option<String>,
    pub(super) enabled_by: Option<GateActor>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateActor {
    pub(super) login: Option<String>,
    pub(super) name: Option<String>,
}

impl GateActor {
    pub(super) fn display(&self) -> Option<String> {
        self.login.clone().or_else(|| self.name.clone())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateReviewRequests {
    #[serde(default)]
    pub(super) total_count: usize,
    #[serde(default)]
    pub(super) nodes: Vec<Option<GateReviewRequestNode>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateReviewRequestNode {
    pub(super) requested_reviewer: Option<GateActor>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateReviewConnection {
    #[serde(default)]
    pub(super) nodes: Vec<Option<GateReviewNode>>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateReviewNode {
    #[serde(default)]
    pub(super) state: String,
    pub(super) author: Option<GateActor>,
    pub(super) commit: Option<GateCommitOid>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateCommitOid {
    #[serde(default)]
    pub(super) oid: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateThreadConnection {
    #[serde(default)]
    pub(super) total_count: usize,
    #[serde(default)]
    pub(super) page_info: GatePageInfo,
    #[serde(default)]
    pub(super) nodes: Vec<Option<GateThreadNode>>,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct GatePageInfo {
    #[serde(default)]
    pub(super) has_next_page: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateThreadNode {
    #[serde(default)]
    pub(super) is_resolved: bool,
    #[serde(default)]
    pub(super) is_outdated: bool,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateCommitConnection {
    #[serde(default)]
    pub(super) nodes: Vec<Option<GateCommitNode>>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateCommitNode {
    pub(super) commit: Option<GateHeadCommit>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateHeadCommit {
    #[serde(default)]
    pub(super) oid: String,
    pub(super) status_check_rollup: Option<GateRollup>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateRollup {
    pub(super) contexts: Option<GateContextConnection>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateContextConnection {
    #[serde(default)]
    pub(super) total_count: usize,
    #[serde(default)]
    pub(super) page_info: GatePageInfo,
    #[serde(default)]
    pub(super) nodes: Vec<Option<GateContextNode>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateContextNode {
    #[serde(default, rename = "__typename")]
    pub(super) typename: String,
    pub(super) name: Option<String>,
    pub(super) status: Option<String>,
    pub(super) conclusion: Option<String>,
    pub(super) details_url: Option<String>,
    pub(super) context: Option<String>,
    pub(super) state: Option<String>,
    pub(super) target_url: Option<String>,
    #[serde(default)]
    pub(super) is_required: bool,
    pub(super) check_suite: Option<GateCheckSuite>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateCheckSuite {
    pub(super) workflow_run: Option<GateWorkflowRun>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateWorkflowRun {
    pub(super) workflow: Option<GateWorkflowName>,
}

#[derive(Deserialize, Serialize)]
pub(super) struct GateWorkflowName {
    #[serde(default)]
    pub(super) name: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GateBaseRef {
    #[serde(default)]
    pub(super) name: String,
    pub(super) target: Option<GateCommitOid>,
    pub(super) ref_update_rule: Option<GateRefUpdateRule>,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the response mirrors independent constraints in GitHub's ref update rule"
)]
pub(super) struct GateRefUpdateRule {
    #[serde(default)]
    pub(super) required_approving_review_count: Option<usize>,
    #[serde(default)]
    pub(super) required_status_check_contexts: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub(super) requires_code_owner_reviews: bool,
    #[serde(default)]
    pub(super) requires_conversation_resolution: bool,
    #[serde(default)]
    pub(super) requires_linear_history: bool,
    #[serde(default)]
    pub(super) requires_signatures: bool,
}

impl Repository {
    #[doc = " One GraphQL read of everything a merge decision depends on, plus at"]
    #[doc = " most one comparison for base-branch freshness. The verdict is computed"]
    #[doc = " locally so it stays explainable and identical for every caller."]
    pub(crate) fn pull_request_gate(
        &self,
        pull_request: &PullRequest,
        refresh: bool,
    ) -> Result<MergeGate> {
        let key = format!(
            "pr-gate-v1\n{}\n{}\n{}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.number,
            pull_request.head_oid
        );
        if !refresh
            && let Some(cached) = cache_read(&key, CacheLife::Ttl(GATE_CACHE_TTL))
            && let Ok(node) = serde_json::from_slice::<GatePullRequestNode>(&cached)
        {
            let mut gate = self.gate_from_node(pull_request, node, true);
            gate.from_cache = true;
            return Ok(gate);
        }
        let (owner, name) = super::super::review::repository_parts(pull_request)?;
        let data: GateQueryData = self.graphql(
            pull_request,
            GATE_QUERY,
            &json!({ "owner": owner, "name": name, "number": pull_request.number }),
            "unable to read the pull-request merge gate",
        )?;
        let node = data
            .repository
            .and_then(|repository| repository.pull_request)
            .with_context(|| {
                format!(
                    "GitHub returned no pull request #{} for the merge gate",
                    pull_request.number
                )
            })?;
        if let Ok(encoded) = serde_json::to_vec(&node) {
            cache_write(&key, &encoded);
        }
        Ok(self.gate_from_node(pull_request, node, false))
    }

    #[doc = " How many commits the base branch holds that the head does not. Both"]
    #[doc = " sides are immutable object names, so the answer is cached forever and"]
    #[doc = " a moved base simply asks a different question."]
    pub(super) fn commits_behind_base(
        &self,
        repository: &GitHubRepository,
        head: &str,
        base: &str,
    ) -> Option<usize> {
        if !is_commit_oid(head) || !is_commit_oid(base) || repository.name_with_owner.is_empty() {
            return None;
        }
        if head == base {
            return Some(0);
        }
        let key = format!(
            "pr-behind-v1\n{}\n{head}\n{base}",
            repository.url.trim_end_matches('/')
        );
        if let Some(cached) = cache_read(&key, CacheLife::Immutable)
            && let Ok(text) = std::str::from_utf8(&cached)
            && let Ok(behind) = text.trim().parse()
        {
            return Some(behind);
        }
        let output = self
            .run_gh([
                std::ffi::OsString::from("api"),
                std::ffi::OsString::from(format!(
                    "repos/{}/compare/{head}...{base}",
                    repository.name_with_owner
                )),
                std::ffi::OsString::from("--jq"),
                std::ffi::OsString::from(".ahead_by"),
            ])
            .ok()?;
        if !output.status.success() || output.stdout_truncated {
            return None;
        }
        let behind: usize = std::str::from_utf8(&output.stdout)
            .ok()?
            .trim()
            .parse()
            .ok()?;
        cache_write(&key, behind.to_string().as_bytes());
        Some(behind)
    }
}
