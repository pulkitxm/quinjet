#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) struct ApiPage {
    pub(super) data: Vec<u8>,
    pub(super) truncated: bool,
    pub(super) has_next: bool,
    pub(super) last_page: Option<usize>,
}

impl Repository {
    pub(crate) fn perform_pull_request_operation(
        &self,
        pull_request: &PullRequest,
        operation: &PullRequestOperation,
    ) -> Result<String> {
        let args = pull_request_operation_args(pull_request, operation);
        let output = self.run_gh(args)?;
        if !output.status.success() {
            bail!(
                "{}",
                bounded_command_error("unable to update the pull request", &output)
            );
        }
        Ok(operation.success_message(pull_request))
    }

    /// One bounded page of a listing endpoint: its body trimmed to whole
    /// records, plus whether GitHub advertises another page after it.
    pub(super) fn api_page(
        &self,
        endpoint: &str,
        jq: &str,
        page: usize,
        error_context: &str,
    ) -> Result<ApiPage> {
        let output = self.run_gh([
            OsString::from("api"),
            OsString::from("-i"),
            OsString::from(format!("{endpoint}&page={page}")),
            OsString::from("--jq"),
            OsString::from(jq),
        ])?;
        if !output.status.success() && !output.stdout_truncated {
            bail!("{}", bounded_command_error(error_context, &output));
        }
        let (head, body) = split_http_response(&output.stdout);
        let has_next = has_next_page(head.as_ref());
        let mut data = body.to_vec();
        if output.stdout_truncated {
            while data.last().is_some_and(|byte| *byte != b'\n') {
                let _ = data.pop();
            }
        }
        Ok(ApiPage {
            data,
            truncated: output.stdout_truncated,
            has_next,
            last_page: last_page(head.as_ref()),
        })
    }

    /// Per-file additions and deletions from the pull-request files endpoint.
    /// In the blob-less disposable workspace a local `--numstat` would download
    /// every changed blob just to count lines; GitHub already knows the totals.
    pub(super) fn pull_request_file_counts_from_api(
        &self,
        pull_request: &PullRequest,
    ) -> Option<HashMap<PathBuf, DiffLineCounts>> {
        let base = pull_request.base_oid.trim();
        let head = pull_request.head_oid.trim();
        let repository = &pull_request.base_repository;
        if !is_commit_oid(base) || !is_commit_oid(head) || repository.name_with_owner.is_empty() {
            return None;
        }
        let key = format!(
            "pr-file-counts-v3\n{}\n{}\n{base}\n{head}",
            repository.url.trim_end_matches('/'),
            pull_request.number
        );
        if let Some(data) = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES) {
            return Some(parse_api_file_counts(&data));
        }
        let endpoint = format!(
            "repos/{}/pulls/{}/files?per_page=100",
            repository.name_with_owner, pull_request.number
        );
        let jq = ".[] | [.filename, (.additions|tostring), (.deletions|tostring), .status] | @tsv";
        let mut collected: Vec<u8> = Vec::new();
        let mut complete = false;
        for page in 1..=MAX_FILE_COUNT_PAGES {
            let read = self
                .api_page(&endpoint, jq, page, "unable to list pull-request files")
                .ok()?;
            if read.truncated {
                return None;
            }
            collected.extend_from_slice(&read.data);
            if collected.last().is_some_and(|byte| *byte != b'\n') {
                collected.push(b'\n');
            }
            if !read.has_next {
                complete = true;
                break;
            }
        }
        if complete && collected.len() <= MAX_PR_PATH_BYTES {
            cache_write_bounded(&key, &collected, MAX_PR_PATH_BYTES);
        }
        Some(parse_api_file_counts(&collected))
    }

    /// Ask the GitHub compare API for the merge base of the two immutable PR
    /// commits. One metadata request replaces the deepening fetch ladder, which
    /// cannot reach a merge base thousands of commits behind either tip.
    pub(super) fn merge_base_from_api(&self, pull_request: &PullRequest) -> Option<String> {
        let base = pull_request.base_oid.trim();
        let head = pull_request.head_oid.trim();
        let repository = &pull_request.base_repository;
        if !is_commit_oid(base) || !is_commit_oid(head) || repository.name_with_owner.is_empty() {
            return None;
        }
        let key = format!(
            "pr-merge-base-v1\n{}\n{base}\n{head}",
            repository.url.trim_end_matches('/')
        );
        if let Some(cached) = cache_read(&key, CacheLife::Immutable) {
            let cached = String::from_utf8_lossy(trim_ascii(&cached)).into_owned();
            if is_commit_oid(&cached) {
                return Some(cached);
            }
        }
        let output = self
            .run_gh([
                OsString::from("api"),
                OsString::from(format!(
                    "repos/{}/compare/{base}...{head}",
                    repository.name_with_owner
                )),
                OsString::from("--jq"),
                OsString::from(".merge_base_commit.sha"),
            ])
            .ok()?;
        if !output.status.success() || output.stdout_truncated {
            return None;
        }
        let sha = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
        if !is_commit_oid(&sha) {
            return None;
        }
        cache_write(&key, sha.as_bytes());
        Some(sha)
    }

    pub(super) fn run_gh<I, S>(&self, args: I) -> Result<BoundedOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_gh_bounded(args, MAX_GH_METADATA_BYTES)
    }

    /// Metadata responses are small and share one cap, but a check run log is
    /// arbitrarily large and needs its own.
    pub(super) fn run_gh_bounded<I, S>(&self, args: I, stdout_limit: usize) -> Result<BoundedOutput>
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

#[expect(
    clippy::too_many_lines,
    reason = "the match is the exhaustive non-interactive GitHub argument mapping"
)]
pub(super) fn pull_request_operation_args(
    pull_request: &PullRequest,
    operation: &PullRequestOperation,
) -> Vec<OsString> {
    if matches!(operation, PullRequestOperation::Dequeue) {
        return pull_request_graphql_args(
            pull_request,
            "mutation($id:ID!){dequeuePullRequest(input:{id:$id}){mergeQueueEntry{id}}}",
            None,
        );
    }
    if let PullRequestOperation::Subscribe(subscribe) = operation {
        return pull_request_graphql_args(
            pull_request,
            "mutation($id:ID!,$state:SubscriptionState!){updateSubscription(input:{subscribableId:$id,state:$state}){subscribable{viewerSubscription}}}",
            Some(if *subscribe {
                "SUBSCRIBED"
            } else {
                "UNSUBSCRIBED"
            }),
        );
    }
    let command = match operation {
        PullRequestOperation::Merge { .. } | PullRequestOperation::DisableAutoMerge => "merge",
        PullRequestOperation::SetDraft(_) => "ready",
        PullRequestOperation::Review { .. } => "review",
        PullRequestOperation::Comment { .. } => "comment",
        PullRequestOperation::Edit(_) => "edit",
        PullRequestOperation::UpdateBranch(_) => "update-branch",
        PullRequestOperation::Lock(_) => "lock",
        PullRequestOperation::Unlock => "unlock",
        PullRequestOperation::Revert { .. } => "revert",
        PullRequestOperation::Close => "close",
        PullRequestOperation::Reopen => "reopen",
        PullRequestOperation::Dequeue | PullRequestOperation::Subscribe(_) => return Vec::new(),
    };
    let mut args = vec![
        OsString::from("pr"),
        OsString::from(command),
        OsString::from(pull_request.number.to_string()),
        OsString::from("--repo"),
        OsString::from(&pull_request.base_repository.url),
    ];
    match operation {
        PullRequestOperation::Merge {
            method,
            mode,
            delete_branch,
        } => {
            args.push(OsString::from(method.flag()));
            match mode {
                PullRequestMergeMode::Direct => {}
                PullRequestMergeMode::Auto => args.push(OsString::from("--auto")),
                PullRequestMergeMode::Admin => args.push(OsString::from("--admin")),
            }
            if !pull_request.head_oid.is_empty() {
                args.push(OsString::from("--match-head-commit"));
                args.push(OsString::from(&pull_request.head_oid));
            }
            if *delete_branch {
                args.push(OsString::from("--delete-branch"));
            }
        }
        PullRequestOperation::SetDraft(draft) => {
            if *draft {
                args.push(OsString::from("--undo"));
            }
        }
        PullRequestOperation::Review { kind, body } => {
            args.push(OsString::from(kind.flag()));
            if !body.is_empty() {
                args.push(OsString::from("--body"));
                args.push(OsString::from(body));
            }
        }
        PullRequestOperation::Comment { mode, body } => match mode {
            PullRequestCommentMode::Create => {
                args.push(OsString::from("--body"));
                args.push(OsString::from(body));
            }
            PullRequestCommentMode::EditLast => {
                args.push(OsString::from("--edit-last"));
                args.push(OsString::from("--body"));
                args.push(OsString::from(body));
            }
            PullRequestCommentMode::DeleteLast => {
                args.push(OsString::from("--delete-last"));
                args.push(OsString::from("--yes"));
            }
        },
        PullRequestOperation::Edit(edit) => edit.append_args(&mut args),
        PullRequestOperation::UpdateBranch(PullRequestUpdateMethod::Rebase) => {
            args.push(OsString::from("--rebase"));
        }
        PullRequestOperation::Lock(reason) => {
            if let Some(reason) = reason {
                args.push(OsString::from("--reason"));
                args.push(OsString::from(reason.flag()));
            }
        }
        PullRequestOperation::Revert { draft, title, body } => {
            if *draft {
                args.push(OsString::from("--draft"));
            }
            if !title.is_empty() {
                args.push(OsString::from("--title"));
                args.push(OsString::from(title));
            }
            if !body.is_empty() {
                args.push(OsString::from("--body"));
                args.push(OsString::from(body));
            }
        }
        PullRequestOperation::UpdateBranch(PullRequestUpdateMethod::Merge)
        | PullRequestOperation::DisableAutoMerge
        | PullRequestOperation::Unlock
        | PullRequestOperation::Close
        | PullRequestOperation::Reopen
        | PullRequestOperation::Dequeue
        | PullRequestOperation::Subscribe(_) => {}
    }
    if matches!(operation, PullRequestOperation::DisableAutoMerge) {
        args.push(OsString::from("--disable-auto"));
    }
    args
}

fn pull_request_graphql_args(
    pull_request: &PullRequest,
    query: &str,
    subscription: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("api"),
        OsString::from("graphql"),
        OsString::from("--hostname"),
        OsString::from(pull_request.base_repository.host()),
        OsString::from("-f"),
        OsString::from(format!("id={}", pull_request.action_state.node_id)),
        OsString::from("-f"),
        OsString::from(format!("query={query}")),
    ];
    if let Some(subscription) = subscription {
        args.push(OsString::from("-f"));
        args.push(OsString::from(format!("state={subscription}")));
    }
    args
}
