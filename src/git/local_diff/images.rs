use super::{
    Change, ChangeArea, ChangeStatus, DiffDocument, DiffFileIndexEntry, LocalDiffRequest,
    Repository, diff,
};

impl Repository {
    pub(super) fn attach_local_images(
        &self,
        document: &mut DiffDocument,
        request: &LocalDiffRequest,
        file: &DiffFileIndexEntry,
    ) {
        match request {
            LocalDiffRequest::Changes { changes, .. } => {
                let Some(change) = changes.iter().find(|change| change.path == file.path) else {
                    return;
                };
                let (previous, current) = change_blob_origins(change);
                diff::attach_image_previews(
                    document,
                    &diff::RevisionImageSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous,
                        current,
                    },
                );
            }
            LocalDiffRequest::Commit { commit, .. } => {
                let parent = commit.parent_ids.first().map(String::as_str);
                diff::attach_image_previews(
                    document,
                    &diff::RevisionImageSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: parent
                            .map_or(diff::BlobOrigin::Missing, diff::BlobOrigin::Revision),
                        current: diff::BlobOrigin::Revision(&commit.id),
                    },
                );
            }
            LocalDiffRequest::Branch { branch, .. } => {
                diff::attach_image_previews(
                    document,
                    &diff::RevisionImageSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: diff::BlobOrigin::Revision(&branch.reference),
                        current: diff::BlobOrigin::Revision("HEAD"),
                    },
                );
            }
            LocalDiffRequest::Stash { stash, .. } => {
                let parent = format!("{}^1", stash.reference);
                diff::attach_image_previews(
                    document,
                    &diff::RevisionImageSource {
                        git_dir: self.root(),
                        worktree: self.root(),
                        previous: diff::BlobOrigin::Revision(&parent),
                        current: diff::BlobOrigin::Revision(&stash.reference),
                    },
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
