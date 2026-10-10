#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) enum PreparedRepository {
    Opened(PathBuf),
    Temporary(TemporaryBareRepository),
    Deferred(comparison_cache::DeferredRepository),
}

impl PreparedRepository {
    pub(super) fn path(&self) -> &Path {
        match self {
            Self::Opened(path) => path,
            Self::Temporary(repository) => &repository.path,
            Self::Deferred(repository) => &repository.source,
        }
    }
}

pub(crate) struct PreparedPullRequest {
    pub(super) repository: PreparedRepository,
    pub(super) pull_request: PullRequest,
    pub(super) merge_base: String,
    pub(super) head: String,
    pub(super) index: PullRequestDiffIndex,
    pub(super) diff_highlighting: bool,
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
            return Ok(self.file_document(&patch, file, false));
        }
        let (patch, truncated) = diff_selected_paths(
            self.repository_path()?,
            &self.merge_base,
            &self.head,
            std::slice::from_ref(&file.path),
        )?;
        if !truncated {
            cache_write_bounded(&key, &patch, MAX_CACHED_PATCH_BYTES);
        }
        Ok(self.file_document(&patch, file, truncated))
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
                self.repository_path()?,
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
                documents.push((file.path.clone(), self.file_document(body, file, false)));
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
                        self.file_document(section.body, file, true),
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
                self.file_document(section.body, file, section_truncated),
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
            self.repository_path()?,
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

    fn file_document(&self, patch: &[u8], file: &PullRequestFile, truncated: bool) -> DiffDocument {
        use crate::git::diff::attach_file_previews;
        let mut document = pull_request_file_document(
            patch,
            &self.pull_request,
            file,
            truncated,
            self.diff_highlighting,
        );
        attach_file_previews(&mut document, self, false, self.diff_highlighting);
        document
    }
}

impl PreparedPullRequest {
    fn repository_path(&self) -> Result<&Path> {
        match &self.repository {
            PreparedRepository::Deferred(repository) => {
                repository.resolve(&self.pull_request, &self.merge_base, &self.head)
            }
            repository => Ok(repository.path()),
        }
    }
}

impl crate::git::diff::DiffBlobSource for PreparedPullRequest {
    fn load(
        &self,
        path: &Path,
        old_path: Option<&Path>,
        side: crate::git::diff::ImageSide,
    ) -> crate::git::diff::LoadedBlob {
        use crate::git::diff::{BlobOrigin, ImageSide, LoadedBlob, RevisionBlobSource};
        let Some(file) = self.index.files.iter().find(|file| file.path == path) else {
            return LoadedBlob::Missing;
        };
        if matches!(
            (side, file.status),
            (ImageSide::Previous, PullRequestFileStatus::Added)
                | (ImageSide::New, PullRequestFileStatus::Deleted)
        ) {
            return LoadedBlob::Missing;
        }
        let Ok(repository) = self.repository_path() else {
            return LoadedBlob::Missing;
        };
        RevisionBlobSource {
            git_dir: repository,
            worktree: repository,
            previous: BlobOrigin::Revision(&self.merge_base),
            current: BlobOrigin::Revision(&self.head),
        }
        .load(path, old_path, side)
    }
}
