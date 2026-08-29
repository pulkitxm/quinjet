#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What applying a set of suggestions would do to the working tree, worked"]
#[doc = " out before anything is written so a preview and the write cannot"]
#[doc = " disagree, and so a set that cannot be applied whole is refused whole."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionPlan {
    pub files: Vec<SuggestionFilePlan>,
    pub applied: Vec<String>,
    pub skipped: Vec<SuggestionSkip>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionFilePlan {
    pub path: PathBuf,
    #[serde(skip)]
    pub contents: String,
    pub removed: usize,
    pub added: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionSkip {
    pub id: String,
    pub location: String,
    pub reason: String,
}

impl SuggestionPlan {
    pub(crate) const fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub(crate) fn summary(&self) -> String {
        if self.is_empty() {
            return "Nothing to apply".to_owned();
        }
        let removed: usize = self.files.iter().map(|file| file.removed).sum();
        let added: usize = self.files.iter().map(|file| file.added).sum();
        format!(
            "{} suggestion(s) across {} file(s), +{added} -{removed}",
            self.applied.len(),
            self.files.len()
        )
    }
}

impl Repository {
    #[doc = " Decide what applying these suggestions would produce, without writing"]
    #[doc = " anything. Overlapping suggestions in one file are refused rather than"]
    #[doc = " applied in some order, because there is no order that is obviously"]
    #[doc = " what the reviewers meant."]
    pub(crate) fn plan_suggestions(&self, suggestions: &[&Suggestion]) -> SuggestionPlan {
        let mut plan = SuggestionPlan {
            files: Vec::new(),
            applied: Vec::new(),
            skipped: Vec::new(),
        };
        let mut by_path: Vec<(PathBuf, Vec<&Suggestion>)> = Vec::new();
        for suggestion in suggestions {
            if let Some(blocker) = &suggestion.blocker {
                plan.skipped.push(SuggestionSkip {
                    id: suggestion.id.clone(),
                    location: suggestion.location(),
                    reason: blocker.message(),
                });
                continue;
            }
            match by_path
                .iter_mut()
                .find(|(path, _)| path == &suggestion.path)
            {
                Some((_, grouped)) => grouped.push(suggestion),
                None => by_path.push((suggestion.path.clone(), vec![*suggestion])),
            }
        }
        for (path, mut grouped) in by_path {
            grouped.sort_by_key(|suggestion| suggestion.start_line);
            match self.plan_file(&path, &grouped) {
                Ok(file) => {
                    plan.applied
                        .extend(grouped.iter().map(|suggestion| suggestion.id.clone()));
                    plan.files.push(file);
                }
                Err(error) => {
                    let reason = format!("{error:#}");
                    plan.skipped
                        .extend(grouped.iter().map(|suggestion| SuggestionSkip {
                            id: suggestion.id.clone(),
                            location: suggestion.location(),
                            reason: reason.clone(),
                        }));
                }
            }
        }
        plan.files.sort_by(|left, right| left.path.cmp(&right.path));
        plan.applied.sort_unstable();
        plan.skipped.sort_by(|left, right| left.id.cmp(&right.id));
        plan
    }

    fn plan_file(&self, path: &Path, suggestions: &[&Suggestion]) -> Result<SuggestionFilePlan> {
        let absolute = self.root().join(path);
        let original = std::fs::read_to_string(&absolute)
            .with_context(|| format!("{} is not readable in the working tree", path.display()))?;
        let lines: Vec<&str> = original.split_inclusive('\n').collect();
        let mut previous_end = 0;
        for suggestion in suggestions {
            if suggestion.start_line <= previous_end {
                bail!(
                    "two suggestions overlap at {}:{}",
                    path.display(),
                    suggestion.start_line
                );
            }
            if suggestion.end_line > lines.len() {
                bail!(
                    "{} has only {} lines but a suggestion replaces line {}",
                    path.display(),
                    lines.len(),
                    suggestion.end_line
                );
            }
            previous_end = suggestion.end_line;
        }
        let ends_with_newline = original.ends_with('\n');
        let mut contents = String::with_capacity(original.len());
        let mut cursor = 0;
        let mut removed = 0;
        let mut added = 0;
        for suggestion in suggestions {
            let start = suggestion.start_line - 1;
            for line in lines.get(cursor..start).unwrap_or_default() {
                contents.push_str(line);
            }
            removed += suggestion.end_line - start;
            if !suggestion.replacement.is_empty() {
                contents.push_str(&suggestion.replacement);
                contents.push('\n');
                added += suggestion.replacement.lines().count();
            }
            cursor = suggestion.end_line;
        }
        for line in lines.get(cursor..).unwrap_or_default() {
            contents.push_str(line);
        }
        if !ends_with_newline && let Some(trimmed) = contents.strip_suffix('\n') {
            contents = trimmed.to_owned();
        }
        Ok(SuggestionFilePlan {
            path: path.to_path_buf(),
            contents,
            removed,
            added,
        })
    }

    #[doc = " Write a plan to the working tree. Every file is written or none is:"]
    #[doc = " a partial application would leave a tree nobody asked for."]
    pub(crate) fn write_suggestion_plan(&self, plan: &SuggestionPlan) -> Result<()> {
        let mut written: Vec<(PathBuf, String)> = Vec::new();
        for file in &plan.files {
            let absolute = self.root().join(&file.path);
            let previous = std::fs::read_to_string(&absolute)
                .with_context(|| format!("{} is no longer readable", file.path.display()))?;
            written.push((absolute.clone(), previous));
            if let Err(error) = std::fs::write(&absolute, &file.contents) {
                for (path, contents) in written.iter().rev().skip(1) {
                    drop(std::fs::write(path, contents));
                }
                return Err(error)
                    .with_context(|| format!("failed to write {}", file.path.display()));
            }
        }
        Ok(())
    }

    #[doc = " Refuse to apply anything unless the working tree is the commit the"]
    #[doc = " suggestions were written against and the files are unmodified. A"]
    #[doc = " suggestion's line numbers only mean something against that commit."]
    pub(crate) fn ensure_suggestions_apply_cleanly(
        &self,
        pull_request: &PullRequest,
        paths: &[PathBuf],
    ) -> Result<()> {
        let head = self
            .rev_parse(["--verify", "--quiet", "HEAD^{commit}"])
            .context("this worktree has no commit checked out")?;
        if !head.eq_ignore_ascii_case(&pull_request.head_oid) {
            bail!(
                "this worktree is at {} but the pull request's head is {}; check the branch out first",
                short(&head),
                short(&pull_request.head_oid)
            );
        }
        let dirty = self.modified_paths(paths)?;
        if !dirty.is_empty() {
            bail!(
                "these files have uncommitted changes: {}",
                dirty
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Ok(())
    }

    fn modified_paths(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut arguments = vec![
            std::ffi::OsString::from("status"),
            std::ffi::OsString::from("--porcelain=v1"),
            std::ffi::OsString::from("-z"),
            std::ffi::OsString::from("--"),
        ];
        arguments.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        let output = self.checked(arguments)?;
        Ok(output
            .split(|byte| *byte == 0)
            .filter(|record| record.len() > 3)
            .filter_map(|record| record.get(3..))
            .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
            .filter(|path| paths.contains(path))
            .collect())
    }
}

impl Repository {
    #[doc = " Stage exactly the files a plan changed and record them. Staging by"]
    #[doc = " path rather than by `--all` keeps an unrelated edit elsewhere in the"]
    #[doc = " tree out of the commit."]
    pub(crate) fn commit_suggestion_paths(&self, paths: &[PathBuf], message: &str) -> Result<()> {
        if message.trim().is_empty() {
            bail!("a commit message cannot be empty");
        }
        let mut stage = vec![
            std::ffi::OsString::from("add"),
            std::ffi::OsString::from("--"),
        ];
        stage.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
        drop(self.checked(stage)?);
        drop(self.checked([
            std::ffi::OsString::from("commit"),
            std::ffi::OsString::from("--message"),
            std::ffi::OsString::from(message),
            std::ffi::OsString::from("--"),
        ])?);
        Ok(())
    }
}

fn short(oid: &str) -> String {
    oid.chars().take(12).collect()
}
