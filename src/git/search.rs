#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;
use crate::search::{
    PreparedSearch, SearchFile, SearchHits, SearchMode, SearchRequest, SearchSource, SearchTarget,
};

const COMMIT_BODY_FORMAT: &str = "%H%x1f%B%x1e";

impl Repository {
    pub(crate) fn search(&self, request: &SearchRequest) -> Result<SearchHits> {
        match &request.target {
            SearchTarget::Changes { files } => {
                self.search_changes(&request.query, request.mode, files)
            }
            SearchTarget::History {
                revision,
                skip,
                limit,
                selected,
            } => self.search_history(
                &request.query,
                request.mode,
                revision,
                *skip,
                *limit,
                selected.as_deref(),
            ),
            SearchTarget::PullRequest { .. } => Err(anyhow!(
                "pull-request contents search needs a prepared workspace"
            )),
        }
    }

    fn search_changes(
        &self,
        query: &str,
        mode: SearchMode,
        files: &[SearchFile],
    ) -> Result<SearchHits> {
        let mut search = PreparedSearch::new(query, mode);
        let mut paths = Vec::new();
        for file in files {
            let name_hit =
                mode.includes_name() && search.name_matches(&file.path.to_string_lossy());
            let content_hit = mode.includes_contents()
                && !query.is_empty()
                && self.file_contents_match(&mut search, file)?;
            if query.is_empty() || name_hit || content_hit {
                paths.push(file.path.to_string_lossy().into_owned());
            }
        }
        Ok(SearchHits::empty(mode, query.to_owned()).with_paths(paths))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "bounded history search needs its page and selected patch"
    )]
    fn search_history(
        &self,
        query: &str,
        mode: SearchMode,
        revision: &str,
        skip: usize,
        limit: usize,
        selected: Option<&str>,
    ) -> Result<SearchHits> {
        let commits = self.history(revision, skip, limit)?;
        if query.is_empty() {
            return Ok(SearchHits::empty(mode, query.to_owned())
                .with_commits(commits.into_iter().map(|commit| commit.id).collect()));
        }
        let mut search = PreparedSearch::new(query, mode);
        let mut hits = Vec::new();
        if mode.includes_name() {
            hits.extend(
                commits
                    .iter()
                    .filter(|commit| commit_name_matches(commit, search.normalized_query()))
                    .map(|commit| commit.id.clone()),
            );
        }
        if mode.includes_contents() {
            hits.extend(self.search_commit_messages(&mut search, revision, skip, limit)?);
            if let Some(selected) = selected
                && self.commit_patch_matches(&mut search, selected)
            {
                hits.push(selected.to_owned());
            }
        }
        Ok(SearchHits::empty(mode, query.to_owned()).with_commits(hits))
    }

    fn file_contents_match(&self, search: &mut PreparedSearch, file: &SearchFile) -> Result<bool> {
        match file.source {
            SearchSource::Worktree => {
                let Ok(path) = safe_worktree_path(&self.root, &file.path) else {
                    return Ok(false);
                };
                if !path.is_file() {
                    return Ok(false);
                }
                let mut contents = Vec::new();
                let _ = fs::File::open(&path)
                    .with_context(|| format!("failed to read {}", file.path.display()))?
                    .take(MAX_DIFF_BYTES as u64 + 1)
                    .read_to_end(&mut contents)
                    .with_context(|| format!("failed to read {}", file.path.display()))?;
                contents.truncate(MAX_DIFF_BYTES);
                Ok(search.haystack_matches(&contents))
            }
            SearchSource::Index => {
                let Some(spec) = git_blob_spec(":0", &file.path) else {
                    return Ok(false);
                };
                match self.checked_bounded(
                    [OsString::from("cat-file"), OsString::from("blob"), spec],
                    MAX_DIFF_BYTES,
                ) {
                    Ok((bytes, _)) => Ok(search.haystack_matches(&bytes)),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    fn search_commit_messages(
        &self,
        search: &mut PreparedSearch,
        revision: &str,
        skip: usize,
        limit: usize,
    ) -> Result<Vec<String>> {
        let limit = if limit == 0 {
            DEFAULT_HISTORY_PAGE
        } else {
            limit
        };
        let (output, _) = self.checked_bounded(
            [
                OsString::from("log"),
                OsString::from("--topo-order"),
                OsString::from("--no-color"),
                OsString::from(format!("--skip={skip}")),
                OsString::from(format!("--max-count={limit}")),
                OsString::from(format!("--format={COMMIT_BODY_FORMAT}")),
                OsString::from(revision),
                OsString::from("--"),
            ],
            MAX_DIFF_BYTES,
        )?;
        Ok(parse_matching_commit_bodies(&output, search))
    }

    fn commit_patch_matches(&self, search: &mut PreparedSearch, selected: &str) -> bool {
        if selected.is_empty() || selected.starts_with('-') {
            return false;
        }
        match self.checked_bounded(
            [
                OsString::from("show"),
                OsString::from("--format="),
                OsString::from("--patch"),
                OsString::from("--no-color"),
                OsString::from("--no-ext-diff"),
                OsString::from(selected),
                OsString::from("--"),
            ],
            MAX_DIFF_BYTES,
        ) {
            Ok((bytes, _)) => search.haystack_matches(&bytes),
            Err(_) => false,
        }
    }
}

impl github::PreparedPullRequest {
    pub(crate) fn search_paths(
        &self,
        query: &str,
        mode: SearchMode,
        paths: &[PathBuf],
    ) -> SearchHits {
        let mut search = PreparedSearch::new(query, mode);
        let mut matched = Vec::new();
        for path in paths {
            let name_hit = mode.includes_name() && search.name_matches(&path.to_string_lossy());
            let content_hit = mode.includes_contents()
                && !query.is_empty()
                && self.file_contents_match(&mut search, path);
            if query.is_empty() || name_hit || content_hit {
                matched.push(path.to_string_lossy().into_owned());
            }
        }
        SearchHits::empty(mode, query.to_owned()).with_paths(matched)
    }

    fn file_contents_match(&self, search: &mut PreparedSearch, path: &Path) -> bool {
        self.blob(path)
            .is_ok_and(|bytes| search.haystack_matches(&bytes))
    }
}

fn commit_name_matches(commit: &Commit, query: &str) -> bool {
    commit.subject.to_lowercase().contains(query)
        || commit.author.to_lowercase().contains(query)
        || commit.id.starts_with(query)
        || commit
            .decorations
            .iter()
            .any(|decoration| decoration.to_lowercase().contains(query))
}

fn parse_matching_commit_bodies(output: &[u8], search: &mut PreparedSearch) -> Vec<String> {
    let mut hits = Vec::new();
    for record in output.split(|byte| *byte == 0x1e) {
        let record = trim_ascii(record);
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(2, |byte| *byte == 0x1f);
        let Some(id) = fields.next() else {
            continue;
        };
        let body = fields.next().unwrap_or(b"");
        if search.haystack_matches(body) {
            hits.push(text(id));
        }
    }
    hits
}
