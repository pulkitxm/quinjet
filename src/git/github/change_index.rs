#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn parse_api_file_counts(data: &[u8]) -> HashMap<PathBuf, DiffLineCounts> {
    let mut counts = HashMap::new();
    for record in data.split(|byte| *byte == b'\n') {
        if record.is_empty() {
            continue;
        }
        let Ok([path, additions, deletions, status]) = parse_tsv_record::<4>(record) else {
            continue;
        };
        let (Ok(additions), Ok(deletions)) = (additions.parse(), deletions.parse()) else {
            continue;
        };
        if additions == 0 && deletions == 0 && status != "renamed" {
            continue;
        }
        let _ = counts.insert(
            PathBuf::from(path),
            DiffLineCounts {
                additions,
                deletions,
                binary: false,
            },
        );
    }
    counts
}

pub(super) fn is_commit_oid(oid: &str) -> bool {
    (oid.len() == 40 || oid.len() == 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn preferred_fetched_commit(
    temporary: &Path,
    oid: &str,
    fallback: &str,
) -> Result<String> {
    if is_commit_oid(oid) {
        let args = [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(format!("{oid}^{{commit}}")),
        ];
        let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
        if output.status.success() {
            let resolved = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
            if !resolved.is_empty() {
                return Ok(resolved);
            }
        }
    }
    Ok(fallback.to_owned())
}

pub(super) fn try_merge_base(temporary: &Path, base: &str, head: &str) -> Result<Option<String>> {
    let args = [
        OsString::from("merge-base"),
        OsString::from(base),
        OsString::from(head),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
    if !output.status.success() {
        return Ok(None);
    }
    let merge_base = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
    Ok((!merge_base.is_empty()).then_some(merge_base))
}

pub(super) fn changed_files_in_repository(
    repository: &Path,
    merge_base: &str,
    head: &str,
    api_counts: Option<HashMap<PathBuf, DiffLineCounts>>,
) -> Result<(Vec<PullRequestFile>, bool)> {
    let args = [
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    let counts = api_counts.unwrap_or_else(|| numstat_counts(repository, merge_base, head));
    let key = format!("pr-files-v1\n{merge_base}\n{head}");
    let cached = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES);
    let output = if let Some(data) = cached {
        BoundedOutput {
            status: successful_status(),
            stdout: data,
            stderr: Vec::new(),
            stdout_truncated: false,
        }
    } else {
        let output = run_repository_git(repository, &args, MAX_PR_PATH_BYTES, 128 * 1024)?;
        if !output.status.success() && !output.stdout_truncated {
            bail!(
                "{}",
                bounded_command_error("unable to enumerate pull-request files", &output)
            );
        }
        if !output.stdout_truncated {
            cache_write_bounded(&key, &output.stdout, MAX_PR_PATH_BYTES);
        }
        output
    };
    let mut truncated = output.stdout_truncated;
    let complete_output = if output.stdout_truncated && !output.stdout.ends_with(&[0]) {
        output
            .stdout
            .iter()
            .rposition(|byte| *byte == 0)
            .map_or(&[][..], |index| {
                output.stdout.get(..=index).unwrap_or(&output.stdout)
            })
    } else {
        &output.stdout
    };
    let records = complete_output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < records.len() {
        if files.len() >= MAX_PR_PATHS {
            truncated = true;
            break;
        }
        let Some(status_record) = records.get(index).copied() else {
            break;
        };
        index += 1;
        let status_code = status_record.first().copied().unwrap_or_default();
        let status = match status_code {
            b'A' => PullRequestFileStatus::Added,
            b'M' => PullRequestFileStatus::Modified,
            b'D' => PullRequestFileStatus::Deleted,
            b'R' => PullRequestFileStatus::Renamed,
            b'C' => PullRequestFileStatus::Copied,
            b'T' => PullRequestFileStatus::TypeChanged,
            b'U' => PullRequestFileStatus::Unmerged,
            _ => PullRequestFileStatus::Unknown,
        };
        let rename_or_copy = matches!(
            status,
            PullRequestFileStatus::Renamed | PullRequestFileStatus::Copied
        );
        let Some(first_path) = records.get(index) else {
            truncated = true;
            break;
        };
        index += 1;
        let first_path = PathBuf::from(String::from_utf8_lossy(first_path).into_owned());
        let (old_path, path) = if rename_or_copy {
            let Some(new_path) = records.get(index) else {
                truncated = true;
                break;
            };
            index += 1;
            (
                Some(first_path),
                PathBuf::from(String::from_utf8_lossy(new_path).into_owned()),
            )
        } else {
            (None, first_path)
        };
        let file_counts = counts.get(&path).copied();
        files.push(PullRequestFile {
            path,
            old_path,
            status,
            counts: file_counts,
        });
    }
    Ok((files, truncated))
}

/// Read exact per-file totals alongside the changed-path listing. One extra
/// `--numstat` pass over the same range lets every file header render its real
/// `+n -n` immediately, so the list never fills in unevenly as patches load.
pub(super) fn numstat_counts(
    repository: &Path,
    merge_base: &str,
    head: &str,
) -> HashMap<PathBuf, DiffLineCounts> {
    let key = format!("pr-numstat-v1\n{merge_base}\n{head}");
    if let Some(data) = cache_read_bounded(&key, CacheLife::Immutable, MAX_PR_PATH_BYTES) {
        return parse_numstat(&data);
    }
    let args = [
        OsString::from("diff"),
        OsString::from("--numstat"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    run_repository_git(repository, &args, MAX_PR_PATH_BYTES, 128 * 1024)
        .ok()
        .filter(|output| output.status.success() && !output.stdout_truncated)
        .map(|output| {
            cache_write_bounded(&key, &output.stdout, MAX_PR_PATH_BYTES);
            parse_numstat(&output.stdout)
        })
        .unwrap_or_default()
}

/// A status that reports success, for feeding cached bytes back through the
/// same path a real command's output takes.
pub(super) fn successful_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
    #[cfg(not(unix))]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
}

pub(super) fn patch_cache_key(merge_base: &str, head: &str, path: &Path) -> String {
    format!("pr-patch-v1\n{merge_base}\n{head}\n{}", path.display())
}

pub(super) fn diff_selected_paths(
    repository: &Path,
    merge_base: &str,
    head: &str,
    paths: &[PathBuf],
) -> Result<(Vec<u8>, bool)> {
    let mut args = vec![
        OsString::from("diff"),
        OsString::from("--no-color"),
        OsString::from("--no-ext-diff"),
        OsString::from("--find-renames"),
        OsString::from("--patch"),
        OsString::from("--unified=3"),
        OsString::from(merge_base),
        OsString::from(head),
        OsString::from("--"),
    ];
    args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
    let output = run_repository_git(repository, &args, MAX_DIFF_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to generate the local pull-request diff", &output)
        );
    }
    let mut patch = output.stdout;
    if output.stdout_truncated {
        while patch.last().is_some_and(|byte| *byte != b'\n') {
            let _ = patch.pop();
        }
    }
    Ok((patch, output.stdout_truncated))
}

pub(super) fn checked_temp_git(
    temporary: &Path,
    args: &[OsString],
    context: &str,
) -> Result<Vec<u8>> {
    let output = run_temp_git(temporary, args, MAX_GH_METADATA_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() || output.stdout_truncated {
        bail!("{}", bounded_command_error(context, &output));
    }
    Ok(output.stdout)
}

pub(super) fn run_temp_git(
    temporary: &Path,
    args: &[OsString],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    run_repository_git(temporary, args, stdout_limit, stderr_limit)
}

pub(super) fn run_repository_git(
    repository: &Path,
    args: &[OsString],
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<BoundedOutput> {
    let mut command = Command::new("git");
    let _ = command
        .arg("-C")
        .arg(repository)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    run_bounded_command(&mut command, stdout_limit, stderr_limit)
        .with_context(|| format!("failed to execute Git in {}", repository.display()))
}
