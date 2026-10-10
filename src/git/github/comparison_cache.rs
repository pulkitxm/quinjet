#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use std::sync::OnceLock;

#[derive(Deserialize, Serialize)]
struct CachedComparison {
    merge_base: String,
    index: PullRequestDiffIndex,
}

pub(super) fn load(
    repository: &Repository,
    pull_request: &PullRequest,
    exact_base: bool,
) -> Option<PreparedPullRequest> {
    let key = key(pull_request, exact_base)?;
    let data = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES)?;
    let cached: CachedComparison = serde_json::from_slice(&data).ok()?;
    if !is_commit_oid(&cached.merge_base) || cached.index.files.len() > MAX_PR_PATHS {
        return None;
    }
    let mut pull_request = pull_request.clone();
    if exact_base {
        pull_request.changed_files = cached.index.total_files;
        pull_request.additions = cached
            .index
            .files
            .iter()
            .filter_map(|file| file.counts)
            .map(|counts| counts.additions)
            .sum();
        pull_request.deletions = cached
            .index
            .files
            .iter()
            .filter_map(|file| file.counts)
            .map(|counts| counts.deletions)
            .sum();
    }
    let head = pull_request.head_oid.clone();
    Some(PreparedPullRequest {
        repository: PreparedRepository::Deferred(DeferredRepository {
            source: repository.root().to_path_buf(),
            ready: OnceLock::new(),
        }),
        pull_request,
        merge_base: cached.merge_base,
        head,
        index: cached.index,
        diff_highlighting: repository.diff_highlighting,
    })
}

pub(super) fn save(prepared: &PreparedPullRequest, exact_base: bool) {
    let Some(key) = key(&prepared.pull_request, exact_base) else {
        return;
    };
    if prepared.head != prepared.pull_request.head_oid {
        return;
    }
    let cached = CachedComparison {
        merge_base: prepared.merge_base.clone(),
        index: prepared.index.clone(),
    };
    if let Ok(data) = serde_json::to_vec(&cached) {
        cache_write_bounded(&key, &data, MAX_PR_PATH_BYTES);
    }
}

fn key(pull_request: &PullRequest, exact_base: bool) -> Option<String> {
    (is_commit_oid(&pull_request.base_oid) && is_commit_oid(&pull_request.head_oid)).then(|| {
        format!(
            "pr-comparison-v1\n{}\n{}\n{}\n{exact_base}",
            pull_request.base_repository.url.trim_end_matches('/'),
            pull_request.base_oid,
            pull_request.head_oid,
        )
    })
}

pub(super) struct DeferredRepository {
    pub(super) source: PathBuf,
    ready: OnceLock<TemporaryBareRepository>,
}

impl DeferredRepository {
    pub(super) fn resolve(
        &self,
        pull_request: &PullRequest,
        merge_base: &str,
        head: &str,
    ) -> Result<&Path> {
        if let Some(repository) = self.ready.get() {
            return Ok(&repository.path);
        }
        let opened = Repository {
            root: self.source.clone(),
            github_cli: None,
            diff_highlighting: true,
        };
        if opened.has_commit(merge_base) && opened.has_commit(head) {
            return Ok(&self.source);
        }
        let temporary = TemporaryBareRepository::new()?;
        temporary.borrow_local_objects(&opened);
        let (fetched_base, fetched_head) =
            fetch_pull_request(&temporary.path, pull_request, Some(merge_base), &mut |_| {})?;
        if fetched_head != head || fetched_base != merge_base {
            bail!("The pull-request comparison changed; refresh before loading uncached files");
        }
        drop(self.ready.set(temporary));
        self.ready
            .get()
            .map(|repository| repository.path.as_path())
            .context("the pull-request repository was not prepared")
    }
}
