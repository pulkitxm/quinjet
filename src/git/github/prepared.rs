#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
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
            return Ok(self.with_images(
                pull_request_file_document(&patch, &self.pull_request, file, false),
                file,
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
        Ok(self.with_images(
            pull_request_file_document(&patch, &self.pull_request, file, truncated),
            file,
        ))
    }

    #[doc = " Produce many file documents from a single `git diff`. Spawning one Git"]
    #[doc = " process per file dominates the cost of a wide pull request, so batching"]
    #[doc = " is what lets the whole diff arrive while the reader is still reading the"]
    #[doc = " first file."]
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
                    self.with_images(
                        pull_request_file_document(body, &self.pull_request, file, false),
                        file,
                    ),
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
                        self.with_images(
                            pull_request_file_document(
                                section.body,
                                &self.pull_request,
                                file,
                                true,
                            ),
                            file,
                        ),
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
                self.with_images(
                    pull_request_file_document(
                        section.body,
                        &self.pull_request,
                        file,
                        section_truncated,
                    ),
                    file,
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

    pub(crate) fn blob(&self, path: &Path) -> Result<Vec<u8>> {
        let spec = crate::git::support::git_blob_spec(&self.head, path)
            .ok_or_else(|| anyhow!("refusing to read {}", path.display()))?;
        let output = run_repository_git(
            self.repository.path(),
            &[OsString::from("cat-file"), OsString::from("blob"), spec],
            MAX_DIFF_BYTES,
            MAX_GH_ERROR_BYTES,
        )?;
        if !output.status.success() && !output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to read pull-request file", &output)
            );
        }
        Ok(output.stdout)
    }

    fn with_images(&self, mut document: DiffDocument, file: &PullRequestFile) -> DiffDocument {
        use crate::git::diff::{BlobOrigin, RevisionImageSource, attach_image_previews};
        let previous = match file.status {
            PullRequestFileStatus::Added => BlobOrigin::Missing,
            _ => BlobOrigin::Revision(self.merge_base.as_str()),
        };
        let current = match file.status {
            PullRequestFileStatus::Deleted => BlobOrigin::Missing,
            _ => BlobOrigin::Revision(self.head.as_str()),
        };
        attach_image_previews(
            &mut document,
            &RevisionImageSource {
                git_dir: self.repository.path(),
                worktree: self.repository.path(),
                previous,
                current,
            },
        );
        document
    }
}
