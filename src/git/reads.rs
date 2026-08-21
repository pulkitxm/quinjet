use super::*;

impl Repository {
    pub(crate) fn has_commit(&self, oid: &str) -> bool {
        is_full_oid(oid)
            && self
                .run([
                    OsString::from("cat-file"),
                    OsString::from("-e"),
                    OsString::from(format!("{oid}^{{commit}}")),
                ])
                .is_ok_and(|output| output.status.success())
    }

    pub(crate) fn branches(&self) -> Result<Vec<Branch>> {
        let output = self.checked([
            OsString::from("for-each-ref"),
            OsString::from("--sort=-committerdate"),
            OsString::from(
                "--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1e",
            ),
            OsString::from("refs/heads"),
        ])?;

        let mut branches = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            let [name, head, upstream, relative_date, short_id, ..] = fields.as_slice() else {
                continue;
            };
            let upstream = text(upstream);
            branches.push(Branch {
                name: text(name),
                current: *head == b"*",
                upstream: (!upstream.is_empty()).then_some(upstream),
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        Ok(branches)
    }

    pub(crate) fn history_branches(&self) -> Result<Vec<HistoryBranch>> {
        let output = self.checked([
            OsString::from("for-each-ref"),
            OsString::from("--sort=-committerdate"),
            OsString::from(
                "--format=%(refname:short)%1f%(refname)%1f%(HEAD)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1f%(symref)%1e",
            ),
            OsString::from("refs/heads"),
            OsString::from("refs/remotes"),
        ])?;

        let mut branches = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            let [name, reference, head, relative_date, short_id, symref, ..] = fields.as_slice()
            else {
                continue;
            };
            if !trim_ascii(symref).is_empty() {
                continue;
            }
            let reference = text(reference);
            let remote = reference.starts_with("refs/remotes/");
            if !reference.starts_with("refs/heads/") && !remote {
                continue;
            }
            branches.push(HistoryBranch {
                name: text(name),
                reference,
                current: *head == b"*",
                remote,
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        branches.sort_by_key(|branch| (!branch.current, branch.remote));
        Ok(branches)
    }

    pub(crate) fn stashes(&self) -> Result<Vec<Stash>> {
        let output = self.checked([
            OsString::from("stash"),
            OsString::from("list"),
            OsString::from("--format=%gd%x1f%gs%x1f%cI%x1f%h%x1e"),
        ])?;
        let mut stashes = Vec::new();
        for record in output.split(|byte| *byte == 0x1e) {
            let record = trim_ascii(record);
            if record.is_empty() {
                continue;
            }
            let fields: Vec<_> = record.split(|byte| *byte == 0x1f).collect();
            let [reference, subject, relative_date, short_id, ..] = fields.as_slice() else {
                continue;
            };
            let reference = text(reference);
            if !valid_stash_reference(&reference) {
                continue;
            }
            let subject = text(subject);
            let (branch, message) = parse_stash_subject(&subject);
            stashes.push(Stash {
                reference,
                message,
                branch,
                relative_date: text(relative_date),
                short_id: text(short_id),
            });
        }
        Ok(stashes)
    }

    pub(crate) fn worktrees(&self) -> Result<Vec<Worktree>> {
        self.worktrees_relative_to(&self.root)
    }

    pub(crate) fn worktrees_relative_to(&self, session_root: &Path) -> Result<Vec<Worktree>> {
        let output = self.checked([
            OsString::from("worktree"),
            OsString::from("list"),
            OsString::from("--porcelain"),
            OsString::from("-z"),
        ])?;
        Ok(parse_worktrees(&output, session_root))
    }

    pub(crate) fn git_common_dir(&self) -> Result<PathBuf> {
        let output = self.checked([
            OsString::from("rev-parse"),
            OsString::from("--git-common-dir"),
        ])?;
        let raw = text(trim_ascii(&output));
        if raw.is_empty() {
            bail!("Git returned an empty common directory");
        }
        let path = Path::new(&raw);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        Ok(fs::canonicalize(&resolved).unwrap_or(resolved))
    }
}
