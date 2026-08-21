#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) struct TemporaryBareRepository {
    pub(super) path: PathBuf,
}

impl TemporaryBareRepository {
    pub(super) fn new() -> Result<Self> {
        let preferred_parent = cache_root().map(|root| root.join("tmp"));
        let parent = match preferred_parent {
            Some(parent) if create_private_directory(&parent).is_ok() => {
                remove_stale_temporary_repositories(&parent);
                parent
            }
            // nosemgrep: rust.lang.security.temp-dir.temp-dir
            _ => env::temp_dir(),
        };
        for _ in 0..16 {
            let id = TEMPORARY_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("pr-{}-{id}.git", std::process::id()));
            if path.exists() {
                continue;
            }
            let mut command = Command::new("git");
            let _ = command
                .args(["init", "--bare", "--quiet"])
                .arg(&path)
                .env("LC_ALL", "C")
                .env("GIT_TERMINAL_PROMPT", "0");
            let output = run_bounded_command(&mut command, 64 * 1024, 64 * 1024)
                .context("failed to initialize a disposable Git repository")?;
            if !output.status.success() {
                bail!(
                    "{}",
                    bounded_command_error(
                        "unable to initialize disposable Git repository",
                        &output
                    )
                );
            }
            return Ok(Self { path });
        }
        bail!("unable to allocate a unique disposable Git repository")
    }

    /// Let the disposable workspace read the opened repository's objects. A
    /// merged or locally built pull request usually already has most of its
    /// blobs on disk under other refs, so lazy blob reads resolve from the
    /// local store instead of the network. The opened repository is only read.
    pub(super) fn borrow_local_objects(&self, repository: &Repository) {
        let Ok(common) = repository.git_common_dir() else {
            return;
        };
        let objects = common.join("objects");
        if !objects.is_dir() {
            return;
        }
        let info = self.path.join("objects").join("info");
        drop(fs::write(
            info.join("alternates"),
            format!("{}\n", objects.display()),
        ));
    }
}

impl Drop for TemporaryBareRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

pub(super) fn remove_stale_temporary_repositories(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.filter_map(Result::ok).take(256) {
        let path = entry.path();
        let is_quinjet_pr = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| {
                name.starts_with("pr-") && Path::new(name).extension() == Some(OsStr::new("git"))
            });
        if !is_quinjet_pr {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= TEMPORARY_REPOSITORY_MAX_AGE);
        if stale {
            drop(fs::remove_dir_all(path));
        }
    }
}

pub(super) fn fetch_pull_request(
    temporary: &Path,
    pull_request: &PullRequest,
    merge_base_hint: Option<&str>,
    progress: &mut dyn FnMut(PullRequestProgress),
) -> Result<(String, String)> {
    if pull_request.base_ref.is_empty() || pull_request.head_ref.is_empty() {
        bail!("Pull request metadata does not contain complete base/head refs");
    }
    drop(checked_temp_git(
        temporary,
        &[
            OsString::from("remote"),
            OsString::from("add"),
            OsString::from("origin"),
            OsString::from(pull_request.base_repository.selector()),
        ],
        "unable to configure the disposable base remote",
    )?);
    let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
    let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);

    progress(PullRequestProgress::FetchingHead);
    let (head_remote, head_refspec) = match fetch_ref(temporary, "origin", &pull_refspec, 64) {
        Ok(()) => ("origin".to_owned(), pull_refspec),
        Err(pull_ref_error) => {
            let Some(head_repository) = pull_request.head_repository.as_deref() else {
                return Err(pull_ref_error).context(
                    "the base repository no longer exposes the PR head and its fork was deleted",
                );
            };
            let head_url = repository_url_for_name(&pull_request.base_repository, head_repository);
            drop(checked_temp_git(
                temporary,
                &[
                    OsString::from("remote"),
                    OsString::from("add"),
                    OsString::from("head"),
                    OsString::from(head_url),
                ],
                "unable to configure the disposable fork remote",
            )?);
            let head_refspec = format!("+refs/heads/{}:refs/quinjet/head", pull_request.head_ref);
            fetch_ref(temporary, "head", &head_refspec, 64).with_context(|| {
                format!(
                    "unable to fetch PR #{} from either the base PR ref or its fork",
                    pull_request.number
                )
            })?;
            ("head".to_owned(), head_refspec)
        }
    };

    progress(PullRequestProgress::FindingMergeBase);
    if let Some(hint) = merge_base_hint {
        let hint_refspec = format!("+{hint}:refs/quinjet/merge-base");
        if fetch_ref(temporary, "origin", &hint_refspec, 1).is_ok() {
            let head =
                preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
            if head == pull_request.head_oid {
                return Ok((hint.to_owned(), head));
            }
        }
    }

    progress(PullRequestProgress::FetchingBase);
    fetch_ref(temporary, "origin", &base_refspec, 64)?;
    for depth in [64_usize, 256, 1_024, 4_096, 16_384] {
        if depth != 64 {
            fetch_ref(temporary, "origin", &base_refspec, depth)?;
            fetch_ref(temporary, &head_remote, &head_refspec, depth)?;
        }
        let base =
            preferred_fetched_commit(temporary, &pull_request.base_oid, "refs/quinjet/base")?;
        let head =
            preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
        if let Some(merge_base) = try_merge_base(temporary, &base, &head)? {
            return Ok((merge_base, head));
        }
    }
    bail!(
        "Unable to find the PR merge base within 16,384 commits; refusing an unbounded history fetch"
    )
}

pub(super) fn repository_url_for_name(base: &GitHubRepository, name_with_owner: &str) -> String {
    if let Some((scheme, rest)) = base.url.split_once("://") {
        let host = rest.split('/').next().unwrap_or_default();
        if !host.is_empty() {
            return format!("{scheme}://{host}/{name_with_owner}");
        }
    }
    name_with_owner.to_owned()
}

pub(super) fn fetch_ref(temporary: &Path, remote: &str, refspec: &str, depth: usize) -> Result<()> {
    let args = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from("--filter=blob:none"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if output.status.success() {
        return Ok(());
    }

    let fallback = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &fallback, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() {
        bail!(
            "{}",
            bounded_command_error("unable to fetch a pull-request ref", &output)
        );
    }
    Ok(())
}
