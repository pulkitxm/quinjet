use std::path::PathBuf;

/// Where a change lives in Git's three-tree model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ChangeArea {
    Conflict,
    Staged,
    Unstaged,
}

impl ChangeArea {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Conflict => "Merge Changes",
            Self::Staged => "Staged Changes",
            Self::Unstaged => "Changes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

impl ChangeStatus {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::TypeChanged => "T",
            Self::Untracked => "U",
            Self::Conflicted => "!",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Deleted => "Deleted",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::TypeChanged => "Type changed",
            Self::Untracked => "Untracked",
            Self::Conflicted => "Conflict",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Change {
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
    pub area: ChangeArea,
    pub status: ChangeStatus,
}

impl Change {
    pub(crate) fn display_path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    pub(crate) fn file_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.display_path(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    pub(crate) fn parent_path(&self) -> String {
        self.path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BranchState {
    pub head: String,
    pub oid: Option<String>,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub detached: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RepoStatus {
    pub branch: BranchState,
    pub changes: Vec<Change>,
}

impl RepoStatus {
    pub(crate) fn staged_count(&self) -> usize {
        self.changes
            .iter()
            .filter(|change| change.area == ChangeArea::Staged)
            .count()
    }
}

/// Parse `git status --porcelain=v2 --branch -z` without depending on localized output.
pub(crate) fn parse_porcelain_v2(output: &[u8]) -> RepoStatus {
    let mut status = RepoStatus::default();
    let records: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    let mut remaining = records.as_slice();

    while let Some((record, rest)) = remaining.split_first() {
        let record = *record;
        remaining = rest;
        let Some((marker, _)) = record.split_first() else {
            continue;
        };

        if record.starts_with(b"# ") {
            parse_branch_header(record, &mut status.branch);
            continue;
        }

        match *marker {
            b'1' => parse_ordinary(record, &mut status.changes),
            b'2' => {
                let (original_path, rest) = remaining
                    .split_first()
                    .map_or_else(|| (b"".as_slice(), remaining), |(path, rest)| (*path, rest));
                remaining = rest;
                parse_renamed(record, original_path, &mut status.changes);
            }
            b'u' => parse_unmerged(record, &mut status.changes),
            b'?' => {
                if let Some(path) = record.get(2..) {
                    status.changes.push(Change {
                        path: bytes_to_path(path),
                        original_path: None,
                        area: ChangeArea::Unstaged,
                        status: ChangeStatus::Untracked,
                    });
                }
            }
            _ => {}
        }
    }

    status.changes.sort_by(|left, right| {
        left.area
            .cmp(&right.area)
            .then_with(|| left.display_path().cmp(&right.display_path()))
    });
    status
}

fn parse_branch_header(record: &[u8], branch: &mut BranchState) {
    let line = String::from_utf8_lossy(record);
    if let Some(value) = line.strip_prefix("# branch.oid ") {
        if value != "(initial)" {
            branch.oid = Some(value.to_owned());
        }
    } else if let Some(value) = line.strip_prefix("# branch.head ") {
        branch.detached = value == "(detached)";
        branch.head = if branch.detached {
            branch.oid.as_deref().map_or_else(
                || "detached".to_owned(),
                |oid| oid.chars().take(8).collect(),
            )
        } else {
            value.to_owned()
        };
    } else if let Some(value) = line.strip_prefix("# branch.upstream ") {
        branch.upstream = Some(value.to_owned());
    } else if let Some(value) = line.strip_prefix("# branch.ab ") {
        for part in value.split_ascii_whitespace() {
            if let Some(ahead) = part.strip_prefix('+') {
                branch.ahead = ahead.parse().unwrap_or_default();
            } else if let Some(behind) = part.strip_prefix('-') {
                branch.behind = behind.parse().unwrap_or_default();
            }
        }
    }
}

fn parse_ordinary(record: &[u8], changes: &mut Vec<Change>) {
    let fields = splitn_bytes(record, b' ', 9);
    let [_, xy, _, _, _, _, _, _, path] = fields.as_slice() else {
        return;
    };
    if xy.len() < 2 {
        return;
    }
    push_xy_changes(xy, path, None, changes);
}

fn parse_renamed(record: &[u8], original_path: &[u8], changes: &mut Vec<Change>) {
    let fields = splitn_bytes(record, b' ', 10);
    let [_, xy, _, _, _, _, _, _, _, path] = fields.as_slice() else {
        return;
    };
    if xy.len() < 2 {
        return;
    }
    push_xy_changes(xy, path, Some(original_path), changes);
}

fn parse_unmerged(record: &[u8], changes: &mut Vec<Change>) {
    let fields = splitn_bytes(record, b' ', 11);
    let [_, _, _, _, _, _, _, _, _, _, path] = fields.as_slice() else {
        return;
    };
    changes.push(Change {
        path: bytes_to_path(path),
        original_path: None,
        area: ChangeArea::Conflict,
        status: ChangeStatus::Conflicted,
    });
}

fn push_xy_changes(
    xy: &[u8],
    path: &[u8],
    original_path: Option<&[u8]>,
    changes: &mut Vec<Change>,
) {
    let ([x, y] | [x, y, ..]) = *xy else {
        return;
    };
    let path = bytes_to_path(path);
    let original_path = original_path
        .filter(|value| !value.is_empty())
        .map(bytes_to_path);

    if x != b'.' {
        changes.push(Change {
            path: path.clone(),
            original_path: original_path.clone(),
            area: ChangeArea::Staged,
            status: status_from_code(x),
        });
    }
    if y != b'.' {
        changes.push(Change {
            path,
            original_path,
            area: ChangeArea::Unstaged,
            status: status_from_code(y),
        });
    }
}

const fn status_from_code(code: u8) -> ChangeStatus {
    match code {
        b'A' => ChangeStatus::Added,
        b'D' => ChangeStatus::Deleted,
        b'R' => ChangeStatus::Renamed,
        b'C' => ChangeStatus::Copied,
        b'T' => ChangeStatus::TypeChanged,
        b'U' => ChangeStatus::Conflicted,
        _ => ChangeStatus::Modified,
    }
}

fn splitn_bytes(input: &[u8], separator: u8, count: usize) -> Vec<&[u8]> {
    input.splitn(count, |byte| *byte == separator).collect()
}

fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn parses_branch_and_all_change_groups() {
        let input = b"# branch.oid 0123456789abcdef\x00# branch.head feature/live\x00# branch.upstream origin/feature/live\x00# branch.ab +2 -3\x001 M. N... 100644 100644 100644 aaaaaaa bbbbbbb src/staged.rs\x001 .M N... 100644 100644 100644 aaaaaaa aaaaaaa src/live.rs\x001 MM N... 100644 100644 100644 aaaaaaa bbbbbbb src/both.rs\x00? notes with spaces.txt\x00u UU N... 100644 100644 100644 100644 aaaaaaa bbbbbbb ccccccc conflict.rs\x00";

        let status = parse_porcelain_v2(input);

        assert_eq!(status.branch.head, "feature/live");
        assert_eq!(
            status.branch.upstream.as_deref(),
            Some("origin/feature/live")
        );
        assert_eq!((status.branch.ahead, status.branch.behind), (2, 3));
        assert_eq!(status.changes.len(), 6);
        assert_eq!(status.staged_count(), 2);
        assert_eq!(
            status
                .changes
                .iter()
                .filter(|change| change.area == ChangeArea::Conflict)
                .count(),
            1
        );
        assert!(status.changes.iter().any(|change| {
            change.path == Path::new("notes with spaces.txt")
                && change.status == ChangeStatus::Untracked
        }));
    }

    #[test]
    fn parses_rename_record_and_original_path() {
        let input =
            b"2 R. N... 100644 100644 100644 aaaaaaa bbbbbbb R100 new name.rs\0old name.rs\0";

        let status = parse_porcelain_v2(input);

        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].path, PathBuf::from("new name.rs"));
        assert_eq!(
            status.changes[0].original_path,
            Some(PathBuf::from("old name.rs"))
        );
        assert_eq!(status.changes[0].status, ChangeStatus::Renamed);
    }
}
