use super::*;

pub(super) enum PreparedRepository {
    Opened(PathBuf),
    Temporary(TemporaryBareRepository),
}

impl PreparedRepository {
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Opened(path) => path,
            Self::Temporary(repository) => &repository.path,
        }
    }
}

pub(crate) struct PreparedPullRequest {
    pub(super) repository: PreparedRepository,
    pub(super) pull_request: PullRequest,
    pub(super) merge_base: String,
    pub(super) head: String,
    pub(super) index: PullRequestDiffIndex,
}

impl PreparedPullRequest {
    pub(crate) fn index(&self) -> PullRequestDiffIndex {
        self.index.clone()
    }

    #[expect(
        clippy::similar_names,
        reason = "the names follow the Git vocabulary they model"
    )]
    pub(crate) fn diff_file(&self, path: &Path) -> Result<DiffDocument> {
        let file = self
            .index
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{} is not part of this pull request", path.display()))?;
        let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
        if let Some(patch) = cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES)
        {
            return Ok(pull_request_file_document(
                &patch,
                &self.pull_request,
                file,
                false,
            ));
        }
        let (patch, truncated) = diff_selected_paths(
            self.repository.path(),
            &self.merge_base,
            &self.head,
            std::slice::from_ref(&file.path),
        )?;
        if !truncated {
            cache_write_bounded(&key, &patch, MAX_CACHED_PATCH_BYTES);
        }
        Ok(pull_request_file_document(
            &patch,
            &self.pull_request,
            file,
            truncated,
        ))
    }

    /// Produce many file documents from a single `git diff`. Spawning one Git
    /// process per file dominates the cost of a wide pull request, so batching
    /// is what lets the whole diff arrive while the reader is still reading the
    /// first file.
    pub(crate) fn diff_files(&self, paths: &[PathBuf]) -> Result<Vec<(PathBuf, DiffDocument)>> {
        let files: Vec<&PullRequestFile> = paths
            .iter()
            .filter_map(|path| self.index.files.iter().find(|file| &file.path == path))
            .collect();
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let mut cached: HashMap<PathBuf, Vec<u8>> = HashMap::new();
        let mut requested: Vec<PathBuf> = Vec::new();
        for file in &files {
            let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
            match cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES) {
                Some(patch) => {
                    drop(cached.insert(file.path.clone(), patch));
                }
                None => requested.push(file.path.clone()),
            }
        }
        let (patch, truncated) = if requested.is_empty() {
            (Vec::new(), false)
        } else {
            diff_selected_paths(
                self.repository.path(),
                &self.merge_base,
                &self.head,
                &requested,
            )?
        };
        let sections = split_patch_by_file(&patch);
        let mut documents = Vec::with_capacity(files.len());
        let mut truncated_fallback = None;
        for file in files {
            if let Some(body) = cached.get(&file.path) {
                documents.push((
                    file.path.clone(),
                    pull_request_file_document(body, &self.pull_request, file, false),
                ));
                continue;
            }
            let Some((index, section)) = sections
                .iter()
                .enumerate()
                .find(|(_, section)| section.matches(&file.path))
            else {
                continue;
            };
            let section_truncated = truncated && index == sections.len().saturating_sub(1);
            if section_truncated && requested.len() > 1 {
                if truncated_fallback.is_none() {
                    truncated_fallback = Some((
                        file.path.clone(),
                        pull_request_file_document(section.body, &self.pull_request, file, true),
                    ));
                }
                continue;
            }
            if !section_truncated {
                let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
                cache_write_bounded(&key, section.body, MAX_CACHED_PATCH_BYTES);
            }
            documents.push((
                file.path.clone(),
                pull_request_file_document(
                    section.body,
                    &self.pull_request,
                    file,
                    section_truncated,
                ),
            ));
        }
        if documents.is_empty()
            && let Some(fallback) = truncated_fallback
        {
            documents.push(fallback);
        }
        Ok(documents)
    }
}
