#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(super) fn prepare_root_commit_diff(
        &self,
        pull_request: &PullRequest,
        oid: &str,
    ) -> Result<PreparedPullRequest> {
        let temporary = TemporaryBareRepository::new()?;
        temporary.borrow_local_objects(self);
        let root_repository = Self {
            root: temporary.path.clone(),
            github_cli: None,
            diff_highlighting: self.diff_highlighting,
        };
        if !root_repository.has_commit(oid) {
            let remote = if self.has_commit(oid) {
                self.root().as_os_str().to_owned()
            } else {
                OsString::from(pull_request.base_repository.selector())
            };
            drop(checked_temp_git(
                &temporary.path,
                &[
                    OsString::from("fetch"),
                    OsString::from("--quiet"),
                    OsString::from("--no-tags"),
                    OsString::from("--depth=1"),
                    remote,
                    OsString::from(oid),
                ],
                "unable to fetch the initial commit",
            )?);
        }
        let empty_tree = checked_temp_git(
            &temporary.path,
            &[OsString::from("mktree")],
            "unable to create an empty tree in the disposable repository",
        )?;
        let empty_tree = text(trim_ascii(&empty_tree));
        let (files, truncated) =
            changed_files_in_repository(&temporary.path, &empty_tree, oid, None)?;
        let total_files = files.len();
        let mut request = pull_request.clone();
        empty_tree.clone_into(&mut request.base_oid);
        oid.clone_into(&mut request.head_oid);
        request.changed_files = total_files;
        request.additions = files
            .iter()
            .filter_map(|file| file.counts)
            .map(|counts| counts.additions)
            .sum();
        request.deletions = files
            .iter()
            .filter_map(|file| file.counts)
            .map(|counts| counts.deletions)
            .sum();
        Ok(PreparedPullRequest {
            repository: PreparedRepository::Temporary(temporary),
            pull_request: request,
            merge_base: empty_tree,
            head: oid.to_owned(),
            index: PullRequestDiffIndex {
                files,
                total_files,
                truncated,
            },
            diff_highlighting: self.diff_highlighting,
        })
    }
}
