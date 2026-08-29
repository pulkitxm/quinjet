#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Repository {
    pub(crate) fn discover(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .env("LC_ALL", "C")
            .output()
            .with_context(|| "failed to run Git; is `git` installed?")?;

        if !output.status.success() {
            bail!("{}", command_error("Not a Git repository", &output));
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            bail!("Git returned an empty repository root");
        }

        Ok(Self {
            root: PathBuf::from(root),
            github_cli: None,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn clone_for_worker(&self) -> Self {
        self.clone()
    }

    pub(crate) fn name(&self) -> String {
        self.root.file_name().map_or_else(
            || self.root.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub(crate) fn status(&self) -> Result<RepoStatus> {
        let output = self.checked([
            OsString::from("status"),
            OsString::from("--porcelain=v2"),
            OsString::from("--branch"),
            OsString::from("-z"),
            OsString::from("--untracked-files=all"),
            OsString::from("--ignore-submodules=none"),
        ])?;
        Ok(parse_porcelain_v2(&output))
    }

    pub(crate) fn resolve_revision(&self, revision: &str) -> Result<String> {
        let revision = revision.trim();
        if revision.is_empty() || revision.starts_with('-') {
            bail!("refusing to resolve `{revision}` as a revision");
        }
        if revision == "HEAD" {
            return Ok(revision.to_owned());
        }
        if let Some(reference) =
            self.rev_parse(["--symbolic-full-name", "--verify", "--quiet", revision])
            && (reference.starts_with("refs/heads/")
                || reference.starts_with("refs/remotes/")
                || reference.starts_with("refs/tags/"))
        {
            return Ok(reference);
        }
        self.rev_parse(["--verify", "--quiet", &format!("{revision}^{{commit}}")])
            .ok_or_else(|| anyhow!("`{revision}` does not name a commit in this repository"))
    }

    pub(crate) fn rev_parse<const N: usize>(&self, args: [&str; N]) -> Option<String> {
        let mut command = vec![OsString::from("rev-parse")];
        command.extend(args.iter().map(OsString::from));
        let output = self.run(command).ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!value.is_empty()).then_some(value)
    }

    pub(crate) fn history(&self, revision: &str, skip: usize, limit: usize) -> Result<Vec<Commit>> {
        if revision != "HEAD"
            && !revision.starts_with("refs/heads/")
            && !revision.starts_with("refs/remotes/")
            && !revision.starts_with("refs/tags/")
            && !is_full_oid(revision)
        {
            bail!("refusing to load history for an invalid branch reference");
        }
        let limit = if limit == 0 {
            DEFAULT_HISTORY_PAGE
        } else {
            limit
        };
        let args = vec![
            OsString::from("log"),
            OsString::from("--topo-order"),
            OsString::from("--decorate=short"),
            OsString::from("--no-color"),
            OsString::from(format!("--skip={skip}")),
            OsString::from(format!("--max-count={limit}")),
            OsString::from(format!("--format={LOG_FORMAT}")),
            OsString::from(revision),
            OsString::from("--"),
        ];
        let output = self.checked(args)?;
        Ok(parse_log(&output))
    }
}
