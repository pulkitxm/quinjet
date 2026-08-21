#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(crate) fn prepare_local_diff(
        &self,
        request: &LocalDiffRequest,
    ) -> Result<PreparedLocalDiff> {
        let index = self.local_diff_index(request)?;
        Ok(PreparedLocalDiff {
            repository: self.clone_for_worker(),
            request: request.clone(),
            index,
        })
    }

    #[expect(
        clippy::option_if_let_else,
        reason = "the branch is one arm of a longer chain that map_or_else cannot express"
    )]
    pub(super) fn local_diff_index(&self, request: &LocalDiffRequest) -> Result<DiffIndex> {
        match request {
            LocalDiffRequest::Changes { changes, .. } => {
                let title = changes.first().map_or_else(
                    || "Working Tree".to_owned(),
                    |first| {
                        if changes.len() == 1 {
                            format!(
                                "{} — {} {}",
                                first.display_path(),
                                first.area.label(),
                                first.status.label()
                            )
                        } else {
                            format!("{}  {} files", first.area.label(), changes.len())
                        }
                    },
                );
                let mut files: Vec<_> = changes
                    .iter()
                    .map(|change| {
                        DiffFileIndexEntry::new(
                            change.path.clone(),
                            change.original_path.clone(),
                            change.status.label().to_ascii_lowercase(),
                        )
                    })
                    .collect();
                self.apply_worktree_counts(&mut files, changes);
                Ok(DiffIndex {
                    title,
                    files,
                    truncated: false,
                    commit_details: None,
                })
            }
            LocalDiffRequest::Commit { commit, .. } => {
                let args = if let Some(parent) = commit.parent_ids.first() {
                    diff_index_args(parent, &commit.id)
                } else {
                    vec![
                        OsString::from("diff-tree"),
                        OsString::from("--root"),
                        OsString::from("--no-commit-id"),
                        OsString::from("--name-status"),
                        OsString::from("-z"),
                        OsString::from("-r"),
                        OsString::from("--find-renames"),
                        OsString::from(&commit.id),
                        OsString::from("--"),
                    ]
                };
                let (files, truncated) = self.diff_index_files(args)?;
                Ok(DiffIndex {
                    title: format!("{} — {}", commit.short_id, commit.subject),
                    files,
                    truncated,
                    commit_details: Some(commit_details(commit)),
                })
            }
            LocalDiffRequest::Branch {
                branch, current, ..
            } => {
                validate_history_reference(&branch.reference)?;
                let (files, truncated) =
                    self.diff_index_files(diff_index_args(&branch.reference, "HEAD"))?;
                Ok(DiffIndex {
                    title: format!("{} → {} — branch comparison", branch.name, current),
                    files,
                    truncated,
                    commit_details: None,
                })
            }
            LocalDiffRequest::Stash { stash, .. } => {
                validate_stash_reference(&stash.reference)?;
                let (files, truncated) = self.diff_index_files(vec![
                    OsString::from("stash"),
                    OsString::from("show"),
                    OsString::from("--name-status"),
                    OsString::from("-z"),
                    OsString::from("--include-untracked"),
                    OsString::from(&stash.reference),
                    OsString::from("--"),
                ])?;
                Ok(DiffIndex {
                    title: format!("{} — {}", stash.reference, stash.message),
                    files,
                    truncated,
                    commit_details: None,
                })
            }
        }
    }

    #[doc = " Working-tree changes are already known from the status snapshot, so the"]
    #[doc = " index needs only their totals. One `--numstat` read per populated area"]
    #[doc = " keeps that to at most two extra Git calls regardless of file count."]
    pub(super) fn apply_worktree_counts(
        &self,
        files: &mut [DiffFileIndexEntry],
        changes: &[Change],
    ) {
        let counts_for = |staged: bool| {
            let mut args = vec![OsString::from("diff"), OsString::from("--numstat")];
            if staged {
                args.push(OsString::from("--cached"));
            }
            args.extend([
                OsString::from("-z"),
                OsString::from("--find-renames"),
                OsString::from("--"),
            ]);
            self.numstat_counts(args)
        };
        let staged = if changes
            .iter()
            .any(|change| change.area == ChangeArea::Staged)
        {
            counts_for(true)
        } else {
            HashMap::new()
        };
        let unstaged = if changes
            .iter()
            .any(|change| change.area != ChangeArea::Staged)
        {
            counts_for(false)
        } else {
            HashMap::new()
        };
        for (file, change) in files.iter_mut().zip(changes) {
            let counts = if change.area == ChangeArea::Staged {
                &staged
            } else {
                &unstaged
            };
            file.counts = counts.get(&file.path).copied();
        }
    }

    #[doc = " Counts are a rendering enhancement, never a correctness requirement, so a"]
    #[doc = " failed or bounded read simply leaves the affected headers unresolved."]
    pub(super) fn numstat_counts(&self, args: Vec<OsString>) -> HashMap<PathBuf, DiffLineCounts> {
        self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)
            .map(|(output, _)| parse_numstat(&output))
            .unwrap_or_default()
    }

    pub(super) fn diff_index_files(
        &self,
        args: Vec<OsString>,
    ) -> Result<(Vec<DiffFileIndexEntry>, bool)> {
        let counts = numstat_args(&args)
            .map(|args| self.numstat_counts(args))
            .unwrap_or_default();
        let (mut output, command_truncated) = self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)?;
        let mut truncated = command_truncated || truncate_diff_index(&mut output);
        if command_truncated && !output.ends_with(&[0]) {
            let boundary = output
                .iter()
                .rposition(|byte| *byte == 0)
                .map_or(0, |index| index + 1);
            output.truncate(boundary);
        }
        let records = output
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect::<Vec<_>>();
        let mut files = Vec::new();
        let mut cursor = 0;
        while cursor < records.len() {
            if files.len() >= MAX_DIFF_INDEX_FILES {
                truncated = true;
                break;
            }
            let Some(status) = records.get(cursor).copied() else {
                break;
            };
            cursor += 1;
            let status_code = status.first().copied().unwrap_or_default();
            let rename_or_copy = matches!(status_code, b'R' | b'C');
            let Some(first_path) = records.get(cursor) else {
                truncated = true;
                break;
            };
            cursor += 1;
            let first_path = PathBuf::from(String::from_utf8_lossy(first_path).into_owned());
            let (old_path, path) = if rename_or_copy {
                let Some(new_path) = records.get(cursor) else {
                    truncated = true;
                    break;
                };
                cursor += 1;
                (
                    Some(first_path),
                    PathBuf::from(String::from_utf8_lossy(new_path).into_owned()),
                )
            } else {
                (None, first_path)
            };
            let counts = counts.get(&path).copied();
            files.push(DiffFileIndexEntry {
                path,
                old_path,
                status: diff_status_label(status_code).to_owned(),
                counts,
            });
        }
        Ok((files, truncated))
    }

    pub(super) fn local_diff_file(
        &self,
        request: &LocalDiffRequest,
        index: &DiffIndex,
        path: &Path,
    ) -> Result<DiffDocument> {
        let file = index
            .files
            .iter()
            .find(|file| file.path == path)
            .with_context(|| format!("{} is not part of this diff", path.display()))?;
        match request {
            LocalDiffRequest::Changes {
                changes, expanded, ..
            } => {
                let change = changes
                    .iter()
                    .find(|change| change.path == path)
                    .with_context(|| format!("{} is no longer changed", path.display()))?;
                self.diff_for_change(change, *expanded)
            }
            LocalDiffRequest::Commit { commit, expanded } => {
                let mut document = if let Some(parent) = commit.parent_ids.first() {
                    self.revision_diff_file(parent, &commit.id, file, *expanded, &index.title)?
                } else {
                    self.root_commit_diff_file(commit, file, *expanded, &index.title)?
                };
                document.commit_details = Some(commit_details(commit));
                Ok(document)
            }
            LocalDiffRequest::Branch {
                branch, expanded, ..
            } => self.revision_diff_file(&branch.reference, "HEAD", file, *expanded, &index.title),
            LocalDiffRequest::Stash { stash, expanded } => {
                self.stash_diff_file(stash, file, *expanded, &index.title)
            }
        }
    }

    pub(super) fn revision_diff_file(
        &self,
        base: &str,
        head: &str,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
            OsString::from(base),
            OsString::from(head),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut args, file);
        self.diff_document_from_args(args, title, &file.path)
    }

    pub(super) fn root_commit_diff_file(
        &self,
        commit: &Commit,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let mut args = vec![
            OsString::from("show"),
            OsString::from("--format="),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
            OsString::from(&commit.id),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut args, file);
        self.diff_document_from_args(args, title, &file.path)
    }

    pub(super) fn stash_diff_file(
        &self,
        stash: &Stash,
        file: &DiffFileIndexEntry,
        expanded: bool,
        title: &str,
    ) -> Result<DiffDocument> {
        let context = if expanded {
            "--unified=1000000"
        } else {
            "--unified=3"
        };
        let mut tracked_args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from("--patch"),
            OsString::from(context),
            OsString::from(format!("{}^1", stash.reference)),
            OsString::from(&stash.reference),
            OsString::from("--"),
        ];
        append_diff_file_paths(&mut tracked_args, file);
        let (mut output, mut truncated) = self.checked_bounded(tracked_args, MAX_DIFF_BYTES)?;

        let untracked_commit = format!("{}^3", stash.reference);
        let untracked_exists = self
            .run([
                OsString::from("rev-parse"),
                OsString::from("--verify"),
                OsString::from("--quiet"),
                OsString::from(&untracked_commit),
            ])
            .is_ok_and(|result| result.status.success());
        if untracked_exists && !truncated {
            let mut untracked_args = vec![
                OsString::from("show"),
                OsString::from("--format="),
                OsString::from("--no-color"),
                OsString::from("--no-ext-diff"),
                OsString::from("--find-renames"),
                OsString::from("--patch"),
                OsString::from(context),
                OsString::from(untracked_commit),
                OsString::from("--"),
            ];
            append_diff_file_paths(&mut untracked_args, file);
            let (untracked, untracked_truncated) =
                self.checked_bounded(untracked_args, MAX_DIFF_BYTES.saturating_sub(output.len()))?;
            output.extend(untracked);
            truncated |= untracked_truncated;
        }

        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok(parse_diff(&output, title, Some(&file.path), truncated))
    }

    pub(super) fn diff_document_from_args<I, S>(
        &self,
        args: I,
        title: &str,
        path: &Path,
    ) -> Result<DiffDocument>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let (mut output, truncated) = self.checked_bounded(args, MAX_DIFF_BYTES)?;
        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok(parse_diff(&output, title, Some(path), truncated))
    }

    pub(crate) fn diff_for_change(&self, change: &Change, expanded: bool) -> Result<DiffDocument> {
        let (output, truncated) = self.raw_diff_for_change(change, expanded)?;
        let title = format!(
            "{} — {} {}",
            change.display_path(),
            change.area.label(),
            change.status.label()
        );
        Ok(parse_diff(&output, title, Some(&change.path), truncated))
    }

    pub(super) fn raw_diff_for_change(
        &self,
        change: &Change,
        expanded: bool,
    ) -> Result<(Vec<u8>, bool)> {
        if change.status == ChangeStatus::Untracked {
            return self.untracked_patch(change);
        }

        let mut args = vec![
            OsString::from("diff"),
            OsString::from("--no-color"),
            OsString::from("--no-ext-diff"),
            OsString::from("--find-renames"),
            OsString::from(if expanded {
                "--unified=1000000"
            } else {
                "--unified=3"
            }),
        ];
        if change.area == ChangeArea::Staged {
            args.push(OsString::from("--cached"));
        }
        if change.area == ChangeArea::Conflict {
            args.push(OsString::from("--cc"));
        }
        args.push(OsString::from("--"));
        args.push(change.path.as_os_str().to_owned());
        let (mut output, truncated) = self.checked_bounded(args, MAX_DIFF_BYTES)?;
        if truncated {
            truncate_to_complete_line(&mut output);
        }
        Ok((output, truncated))
    }
}
