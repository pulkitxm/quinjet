use super::*;

pub(super) fn diff_index_args(base: &str, head: &str) -> Vec<OsString> {
    vec![
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(base),
        OsString::from(head),
        OsString::from("--"),
    ]
}

/// Reuse an index command's own revision range for its totals by swapping the
/// listing option. This keeps the two reads describing exactly the same diff.
pub(super) fn numstat_args(args: &[OsString]) -> Option<Vec<OsString>> {
    let name_status = OsStr::new("--name-status");
    args.iter().any(|arg| arg == name_status).then(|| {
        args.iter()
            .map(|arg| {
                if arg == name_status {
                    OsString::from("--numstat")
                } else {
                    arg.clone()
                }
            })
            .collect()
    })
}

pub(super) fn truncate_diff_index(output: &mut Vec<u8>) -> bool {
    if output.len() <= MAX_DIFF_INDEX_BYTES {
        return false;
    }
    let boundary = output
        .get(..MAX_DIFF_INDEX_BYTES)
        .unwrap_or_default()
        .iter()
        .rposition(|byte| *byte == 0)
        .map_or(0, |index| index + 1);
    output.truncate(boundary);
    true
}

pub(super) const fn diff_status_label(status: u8) -> &'static str {
    match status {
        b'A' => "added",
        b'M' => "modified",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'T' => "type changed",
        b'U' => "unmerged",
        _ => "changed",
    }
}

pub(super) fn append_diff_file_paths(args: &mut Vec<OsString>, file: &DiffFileIndexEntry) {
    if let Some(old_path) = &file.old_path {
        args.push(old_path.as_os_str().to_owned());
    }
    args.push(file.path.as_os_str().to_owned());
}

pub(super) fn commit_details(commit: &Commit) -> CommitDetails {
    CommitDetails {
        id: commit.id.clone(),
        subject: commit.subject.clone(),
        author: commit.author.clone(),
        author_email: commit.author_email.clone(),
        authored_at: commit.authored_at.clone(),
        committer: commit.committer.clone(),
        committer_email: commit.committer_email.clone(),
        committed_at: commit.committed_at.clone(),
    }
}

pub(super) fn validate_history_reference(reference: &str) -> Result<()> {
    if reference.starts_with("refs/heads/") || reference.starts_with("refs/remotes/") {
        Ok(())
    } else {
        bail!("refusing to compare an invalid branch reference")
    }
}

pub(super) fn valid_stash_reference(reference: &str) -> bool {
    reference
        .strip_prefix("stash@{")
        .and_then(|value| value.strip_suffix('}'))
        .is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
}

pub(super) fn validate_stash_reference(reference: &str) -> Result<()> {
    if valid_stash_reference(reference) {
        Ok(())
    } else {
        bail!("refusing to use an invalid stash reference")
    }
}

pub(super) fn parse_stash_subject(subject: &str) -> (String, String) {
    let subject = subject.trim();
    for prefix in ["WIP on ", "On "] {
        if let Some(rest) = subject.strip_prefix(prefix)
            && let Some((branch, message)) = rest.split_once(": ")
        {
            return (branch.to_owned(), message.to_owned());
        }
    }
    (String::new(), subject.to_owned())
}

pub(super) fn parse_worktrees(output: &[u8], session_root: &Path) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut fields = Vec::new();
    for field in output.split(|byte| *byte == 0) {
        if field.is_empty() {
            if !fields.is_empty() {
                if let Some(worktree) = worktree_from_fields(&fields, session_root) {
                    worktrees.push(worktree);
                }
                fields.clear();
            }
            continue;
        }
        fields.push(field);
    }
    if !fields.is_empty()
        && let Some(worktree) = worktree_from_fields(&fields, session_root)
    {
        worktrees.push(worktree);
    }
    worktrees
}

pub(super) fn worktree_from_fields(fields: &[&[u8]], session_root: &Path) -> Option<Worktree> {
    let mut path = None;
    let mut head = String::new();
    let mut branch = None;
    let mut bare = false;
    let mut detached = false;
    let mut locked = None;
    let mut prunable = None;
    for field in fields {
        if let Some(value) = field.strip_prefix(b"worktree ") {
            path = Some(parse_worktree_path(value));
        } else if let Some(value) = field.strip_prefix(b"HEAD ") {
            head = text(value);
        } else if let Some(value) = field.strip_prefix(b"branch ") {
            branch = Some(heads_branch_name(&text(value)));
        } else if *field == b"detached" {
            detached = true;
        } else if *field == b"bare" {
            bare = true;
        } else if let Some(value) = field.strip_prefix(b"locked") {
            locked = Some(text(value).trim().to_owned());
        } else if let Some(value) = field.strip_prefix(b"prunable") {
            prunable = Some(text(value).trim().to_owned());
        }
    }
    let path = path?;
    Some(Worktree {
        current: same_path(&path, session_root),
        path,
        head,
        branch,
        bare,
        detached,
        locked,
        prunable,
    })
}

pub(super) fn heads_branch_name(reference: &str) -> String {
    reference
        .strip_prefix("refs/heads/")
        .unwrap_or(reference)
        .to_owned()
}

pub(super) fn parse_worktree_path(value: &[u8]) -> PathBuf {
    let rendered = text(value);
    #[cfg(windows)]
    {
        PathBuf::from(rendered.replace('/', "\\"))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(rendered)
    }
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
}

pub(super) fn strings<const N: usize>(values: [&str; N]) -> [OsString; N] {
    values.map(OsString::from)
}

pub(super) fn command_error(context: &str, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if stderr.is_empty() { stdout } else { stderr };
    if details.is_empty() {
        format!("{context} (exit status {})", output.status)
    } else {
        format!("{context}: {details}")
    }
}

pub(super) fn safe_worktree_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!(
            "refusing to access path outside the repository: {}",
            relative.display()
        );
    }
    Ok(root.join(relative))
}

pub(super) fn truncate(bytes: &mut Vec<u8>, maximum: usize) -> bool {
    if bytes.len() <= maximum {
        return false;
    }
    bytes.truncate(maximum);
    truncate_to_complete_line(bytes);
    true
}

pub(super) fn truncate_to_complete_line(bytes: &mut Vec<u8>) {
    while bytes.last().is_some_and(|byte| *byte != b'\n') {
        let _ = bytes.pop();
    }
}

pub(super) const fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while let Some((first, rest)) = value.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        value = rest;
    }
    while let Some((last, rest)) = value.split_last() {
        if !last.is_ascii_whitespace() {
            break;
        }
        value = rest;
    }
    value
}

pub(super) fn text(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

pub(super) fn plural_message(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

pub(super) fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
