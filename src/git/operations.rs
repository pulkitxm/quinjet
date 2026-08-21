use super::*;

impl Repository {
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn perform(&self, operation: &GitOperation) -> Result<String> {
        match operation {
            GitOperation::Stage(paths) => {
                drop(self.with_paths(["add"], paths)?);
                Ok(plural_message(
                    paths.len(),
                    "change staged",
                    "changes staged",
                ))
            }
            GitOperation::StageAll => {
                drop(self.checked(strings(["add", "-A"]))?);
                Ok("All changes staged".to_owned())
            }
            GitOperation::Unstage(paths) => {
                self.unstage(paths)?;
                Ok(plural_message(
                    paths.len(),
                    "change unstaged",
                    "changes unstaged",
                ))
            }
            GitOperation::UnstageAll => {
                self.unstage_all()?;
                Ok("All changes unstaged".to_owned())
            }
            GitOperation::Discard(changes) => {
                self.discard(changes)?;
                Ok(plural_message(
                    changes.len(),
                    "change discarded",
                    "changes discarded",
                ))
            }
            GitOperation::Remove(paths) => {
                let removed = self.remove(paths)?;
                Ok(plural_message(removed, "file removed", "files removed"))
            }
            GitOperation::Commit { message, amend } => {
                if message.trim().is_empty() {
                    bail!("Commit message cannot be empty");
                }
                let mut args = vec![OsString::from("commit")];
                if *amend {
                    args.push(OsString::from("--amend"));
                }
                args.push(OsString::from("--message"));
                args.push(OsString::from(message));
                drop(self.checked(args)?);
                Ok(if *amend {
                    "Commit amended".to_owned()
                } else {
                    "Commit created".to_owned()
                })
            }
            GitOperation::Fetch => {
                drop(self.checked(strings(["fetch", "--all", "--prune"]))?);
                Ok("Fetch complete".to_owned())
            }
            GitOperation::Pull => {
                drop(self.checked(strings(["pull"]))?);
                Ok("Pull complete".to_owned())
            }
            GitOperation::Push => {
                self.push()?;
                Ok("Push complete".to_owned())
            }
            GitOperation::Sync => {
                drop(self.checked(strings(["pull"]))?);
                self.push()?;
                Ok("Synchronization complete".to_owned())
            }
            GitOperation::Checkout(branch) => {
                drop(self.checked(strings(["switch", "--", branch]))?);
                Ok(format!("Switched to {branch}"))
            }
            GitOperation::CreateBranch { name, start } => {
                self.validate_branch_name(name)?;
                let mut args = vec![
                    OsString::from("switch"),
                    OsString::from("--create"),
                    OsString::from(name),
                ];
                if let Some(start) = start {
                    args.push(OsString::from(start));
                }
                drop(self.checked(args)?);
                Ok(format!("Created and switched to {name}"))
            }
            GitOperation::RenameBranch { old, new } => {
                self.validate_branch_name(new)?;
                if old == new {
                    bail!("New branch name must be different from the current name");
                }
                drop(self.checked(strings(["branch", "--move", "--", old, new]))?);
                Ok(format!("Renamed local branch {old} to {new}"))
            }
            GitOperation::DeleteBranch(branch) => {
                drop(self.checked(strings(["branch", "--delete", "--", branch]))?);
                Ok(format!("Deleted {branch}"))
            }
            GitOperation::StashPush {
                message,
                include_untracked,
                staged,
                paths,
            } => {
                let mut args = vec![OsString::from("stash"), OsString::from("push")];
                if *include_untracked {
                    args.push(OsString::from("--include-untracked"));
                }
                if *staged {
                    args.push(OsString::from("--staged"));
                }
                if !message.trim().is_empty() {
                    args.push(OsString::from("--message"));
                    args.push(OsString::from(message.trim()));
                }
                if !paths.is_empty() {
                    args.push(OsString::from("--"));
                    for path in paths {
                        args.push(path.into());
                    }
                }
                drop(self.checked(args)?);
                Ok("Changes stashed".to_owned())
            }
            GitOperation::StashApply(reference) => {
                validate_stash_reference(reference)?;
                drop(self.checked(strings(["stash", "apply", "--index", reference]))?);
                Ok(format!("Applied {reference}"))
            }
            GitOperation::StashPop(reference) => {
                let mut args = vec![
                    OsString::from("stash"),
                    OsString::from("pop"),
                    OsString::from("--index"),
                ];
                if let Some(reference) = reference {
                    validate_stash_reference(reference)?;
                    args.push(OsString::from(reference));
                }
                drop(self.checked(args)?);
                Ok(reference.as_ref().map_or_else(
                    || "Popped latest stash".to_owned(),
                    |reference| format!("Popped {reference}"),
                ))
            }
            GitOperation::StashDrop(reference) => {
                validate_stash_reference(reference)?;
                drop(self.checked(strings(["stash", "drop", reference]))?);
                Ok(format!("Dropped {reference}"))
            }
            GitOperation::StashClear => {
                drop(self.checked(strings(["stash", "clear"]))?);
                Ok("Dropped all stashes".to_owned())
            }
            GitOperation::ResolveConflict { path, choice } => {
                let side = match choice {
                    ConflictChoice::Ours => "--ours",
                    ConflictChoice::Theirs => "--theirs",
                };
                drop(self.with_paths(["checkout", side], std::slice::from_ref(path))?);
                drop(self.with_paths(["add"], std::slice::from_ref(path))?);
                Ok(format!("Accepted {side} for {}", path.to_string_lossy()))
            }
            GitOperation::CherryPick(commit) => {
                drop(self.checked(strings(["cherry-pick", commit]))?);
                Ok(format!("Cherry-picked {}", short_id(commit)))
            }
            GitOperation::Revert(commit) => {
                drop(self.checked(strings(["revert", "--no-edit", commit]))?);
                Ok(format!("Reverted {}", short_id(commit)))
            }
        }
    }

    #[expect(
        clippy::similar_names,
        reason = "the names follow the Git vocabulary they model"
    )]
    pub(super) fn untracked_patch(&self, change: &Change) -> Result<(Vec<u8>, bool)> {
        let path = safe_worktree_path(&self.root, &change.path)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to read {}", change.display_path()))?;
        let display_path = change.display_path();
        let binary_patch = || {
            format!(
                "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\nBinary files /dev/null and b/{display_path} differ\n"
            )
            .into_bytes()
        };
        if !metadata.is_file() {
            return Ok((binary_patch(), false));
        }

        let mut contents = Vec::with_capacity(64 * 1024);
        let _ = fs::File::open(&path)
            .with_context(|| format!("failed to read {}", change.display_path()))?
            .take(MAX_DIFF_BYTES as u64 + 1)
            .read_to_end(&mut contents)
            .with_context(|| format!("failed to read {}", change.display_path()))?;
        let input_truncated = contents.len() > MAX_DIFF_BYTES;
        contents.truncate(MAX_DIFF_BYTES);
        if contents.contains(&0) {
            return Ok((binary_patch(), input_truncated));
        }

        let body = String::from_utf8_lossy(&contents);
        let line_count = body.lines().count();
        let mut patch = format!(
            "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{display_path}\n@@ -0,0 +1,{line_count} @@\n"
        );
        for line in body.split_inclusive('\n') {
            patch.push('+');
            patch.push_str(line);
        }
        if !body.is_empty() && !body.ends_with('\n') {
            patch.push('\n');
            patch.push_str("\\ No newline at end of file\n");
        }
        let mut patch = patch.into_bytes();
        let patch_truncated = truncate(&mut patch, MAX_DIFF_BYTES);
        Ok((patch, input_truncated || patch_truncated))
    }

    pub(super) fn delete_worktree_entry(&self, relative: &Path) -> Result<()> {
        let path = safe_worktree_path(&self.root, relative)?;
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", relative.display()))?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub(super) fn remove(&self, paths: &[PathBuf]) -> Result<usize> {
        let mut wanted: Vec<PathBuf> = Vec::new();
        for path in paths {
            if !wanted.contains(path) {
                wanted.push(path.clone());
            }
        }
        if wanted.is_empty() {
            return Ok(0);
        }
        let tracked = self.tracked_paths(&wanted)?;
        for path in wanted.iter().filter(|path| !tracked.contains(*path)) {
            self.delete_worktree_entry(path)?;
        }
        if !tracked.is_empty() {
            drop(self.with_paths(["rm", "--force", "-r"], &tracked)?);
        }
        Ok(wanted.len())
    }

    pub(super) fn tracked_paths(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut args: Vec<OsString> = strings(["ls-files", "-z", "--"]).to_vec();
        args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        let output = self.checked(args)?;
        let listed: Vec<PathBuf> = output
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| PathBuf::from(text(entry)))
            .collect();
        Ok(paths
            .iter()
            .filter(|path| {
                listed
                    .iter()
                    .any(|entry| entry == *path || entry.starts_with(path))
            })
            .cloned()
            .collect())
    }

    pub(super) fn discard(&self, changes: &[Change]) -> Result<()> {
        let mut restore_worktree = Vec::new();
        let mut restore_both = Vec::new();
        for change in changes {
            if change.status == ChangeStatus::Untracked {
                self.delete_worktree_entry(&change.path)?;
            } else if change.area == ChangeArea::Staged {
                restore_both.push(change.path.clone());
            } else {
                restore_worktree.push(change.path.clone());
            }
        }

        if !restore_worktree.is_empty() {
            drop(self.with_paths(["restore", "--worktree"], &restore_worktree)?);
        }
        if !restore_both.is_empty() {
            drop(self.with_paths(
                ["restore", "--staged", "--worktree", "--source=HEAD"],
                &restore_both,
            )?);
        }
        Ok(())
    }

    pub(super) fn unstage(&self, paths: &[PathBuf]) -> Result<()> {
        if self.has_head() {
            drop(self.with_paths(["restore", "--staged"], paths)?);
        } else {
            drop(self.with_paths(["rm", "--cached", "--ignore-unmatch"], paths)?);
        }
        Ok(())
    }

    pub(super) fn unstage_all(&self) -> Result<()> {
        if self.has_head() {
            drop(self.checked(strings(["reset", "--mixed", "--quiet", "HEAD", "--"]))?);
        } else {
            let output = self.run(strings(["rm", "--recursive", "--cached", "."]))?;
            if !output.status.success() && !self.status()?.changes.is_empty() {
                bail!("{}", command_error("Unable to unstage changes", &output));
            }
        }
        Ok(())
    }

    pub(super) fn push(&self) -> Result<()> {
        let status = self.status()?;
        if status.branch.upstream.is_some() {
            drop(self.checked(strings(["push"]))?);
            return Ok(());
        }

        let origin = self.run(strings(["remote", "get-url", "origin"]))?;
        if !origin.status.success() {
            bail!("Current branch has no upstream and no `origin` remote exists");
        }
        drop(self.checked(strings(["push", "--set-upstream", "origin", "HEAD"]))?);
        Ok(())
    }

    pub(super) fn validate_branch_name(&self, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            bail!("Branch name cannot be empty");
        }
        drop(self.checked(strings(["check-ref-format", "--branch", name]))?);
        Ok(())
    }

    pub(super) fn has_head(&self) -> bool {
        self.run(strings(["rev-parse", "--verify", "HEAD"]))
            .is_ok_and(|output| output.status.success())
    }

    pub(super) fn with_paths<const N: usize>(
        &self,
        prefix: [&str; N],
        paths: &[PathBuf],
    ) -> Result<Vec<u8>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<OsString> = prefix.into_iter().map(OsString::from).collect();
        args.push(OsString::from("--"));
        args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        self.checked(args)
    }

    pub(super) fn checked_bounded<I, S>(&self, args: I, limit: usize) -> Result<(Vec<u8>, bool)>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        let _ = command
            .arg("-C")
            .arg(&self.root)
            .args(["-c", "core.quotepath=false"])
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0");
        let output = run_bounded_command(&mut command, limit, MAX_GIT_ERROR_BYTES)
            .with_context(|| format!("failed to execute Git in {}", self.root.display()))?;
        if !output.status.success() && !output.stdout_truncated {
            bail!("{}", bounded_command_error("Git command failed", &output));
        }
        Ok((output.stdout, output.stdout_truncated))
    }

    pub(super) fn checked<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run(args)?;
        if !output.status.success() {
            bail!("{}", command_error("Git command failed", &output));
        }
        Ok(output.stdout)
    }

    pub(super) fn run<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        let _ = command
            .arg("-C")
            .arg(&self.root)
            .args(["-c", "core.quotepath=false"])
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0");
        command
            .output()
            .with_context(|| format!("failed to execute Git in {}", self.root.display()))
    }
}
