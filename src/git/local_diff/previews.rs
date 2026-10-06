use super::{
    Change, ChangeArea, ChangeStatus, DiffDocument, DiffFileIndexEntry, LocalDiffRequest,
    Repository, diff,
};

impl Repository {
    pub(super) fn attach_local_previews(
        &self,
        document: &mut DiffDocument,
        request: &LocalDiffRequest,
        file: &DiffFileIndexEntry,
    ) {
        match request {
            LocalDiffRequest::Changes {
                changes, expanded, ..
            } => {
                let Some(change) = changes.iter().find(|change| change.path == file.path) else {
                    return;
                };
                let (previous, current) = change_blob_origins(change);
                diff::attach_file_previews(
                    document,
                    &diff::RevisionBlobSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous,
                        current,
                    },
                    *expanded,
                );
            }
            LocalDiffRequest::Commit { commit, expanded } => {
                let parent = commit.parent_ids.first().map(String::as_str);
                diff::attach_file_previews(
                    document,
                    &diff::RevisionBlobSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: parent
                            .map_or(diff::BlobOrigin::Missing, diff::BlobOrigin::Revision),
                        current: diff::BlobOrigin::Revision(&commit.id),
                    },
                    *expanded,
                );
            }
            LocalDiffRequest::Branch {
                branch, expanded, ..
            } => {
                diff::attach_file_previews(
                    document,
                    &diff::RevisionBlobSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: diff::BlobOrigin::Revision(&branch.reference),
                        current: diff::BlobOrigin::Revision("HEAD"),
                    },
                    *expanded,
                );
            }
            LocalDiffRequest::Stash { stash, expanded } => {
                let parent = format!("{}^1", stash.reference);
                let untracked = format!("{}^3", stash.reference);
                diff::attach_file_previews(
                    document,
                    &diff::RevisionBlobSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: diff::BlobOrigin::Revision(&parent),
                        current: diff::BlobOrigin::RevisionFallback(&stash.reference, &untracked),
                    },
                    *expanded,
                );
            }
        }
    }
}

const fn change_blob_origins(change: &Change) -> (diff::BlobOrigin<'_>, diff::BlobOrigin<'_>) {
    use diff::BlobOrigin;
    match (change.area, change.status) {
        (_, ChangeStatus::Untracked) | (ChangeArea::Unstaged, ChangeStatus::Added) => {
            (BlobOrigin::Missing, BlobOrigin::Worktree)
        }
        (ChangeArea::Staged, ChangeStatus::Added) => (BlobOrigin::Missing, BlobOrigin::Index),
        (ChangeArea::Staged, ChangeStatus::Deleted) => {
            (BlobOrigin::Revision("HEAD"), BlobOrigin::Missing)
        }
        (_, ChangeStatus::Deleted) => (BlobOrigin::Index, BlobOrigin::Missing),
        (ChangeArea::Staged, _) => (BlobOrigin::Revision("HEAD"), BlobOrigin::Index),
        (ChangeArea::Unstaged | ChangeArea::Conflict, _) => {
            (BlobOrigin::Index, BlobOrigin::Worktree)
        }
    }
}
