#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Changes,
    History,
    PullRequests,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Focus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ChangeSection {
    Conflict,
    Staged,
    Unstaged,
}

impl ChangeSection {
    pub(crate) const ALL: [Self; 3] = [Self::Conflict, Self::Staged, Self::Unstaged];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Conflict => "Merge Changes",
            Self::Staged => "Staged Changes",
            Self::Unstaged => "Changes",
        }
    }

    pub(crate) fn matches(self, change: &Change) -> bool {
        match self {
            Self::Conflict => change.area == ChangeArea::Conflict,
            Self::Staged => change.area == ChangeArea::Staged,
            Self::Unstaged => change.area == ChangeArea::Unstaged,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeRow {
    Section {
        section: ChangeSection,
        count: usize,
        collapsed: bool,
    },
    Change {
        section: ChangeSection,
        index: usize,
        cursor: usize,
    },
    Spacer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffLayout {
    Unified,
    SideBySide,
}

#[doc = " The two halves of the pull-request view. `Overview` lists the pull request's"]
#[doc = " checks beside its conversation; `Files` lists the changed files beside their"]
#[doc = " diffs."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestSection {
    Overview,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CheckStatusSection {
    Failed,
    InProgress,
    Successful,
    Skipped,
}

impl CheckStatusSection {
    pub(crate) const ALL: [Self; 4] = [
        Self::Failed,
        Self::InProgress,
        Self::Successful,
        Self::Skipped,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Failed => "Failed",
            Self::InProgress => "In progress",
            Self::Successful => "Successful",
            Self::Skipped => "Skipped",
        }
    }

    pub(crate) const fn matches(self, status: PullRequestCheckStatus) -> bool {
        match self {
            Self::Failed => matches!(status, PullRequestCheckStatus::Failed),
            Self::InProgress => matches!(status, PullRequestCheckStatus::Pending),
            Self::Successful => matches!(status, PullRequestCheckStatus::Passed),
            Self::Skipped => matches!(
                status,
                PullRequestCheckStatus::Skipped
                    | PullRequestCheckStatus::Cancelled
                    | PullRequestCheckStatus::Unknown
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckListRow {
    Conversation,
    Heading,
    Section {
        section: CheckStatusSection,
        count: usize,
        collapsed: bool,
    },
    Check {
        index: usize,
    },
    Spacer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    variant_size_differences,
    reason = "check indices are pointer-sized and boxing would cost an allocation per row"
)]
pub(crate) enum CheckListTarget {
    Conversation,
    Section(CheckStatusSection),
    Check(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestFileView {
    AllFiles,
    SingleFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PullRequestReviewAnchor {
    pub path: PathBuf,
    pub line: usize,
    pub side: PullRequestReviewSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestReviewTarget {
    Line(PullRequestReviewAnchor),
    File(PathBuf),
    Reply(String),
    Edit { comment_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestReviewThreadAction {
    Reply { thread_id: String },
    CopyComment { body: String },
    OpenComment { url: String },
    EditComment { comment_id: String, body: String },
    DeleteComment { comment_id: String },
    Resolve { thread_id: String },
    Reopen { thread_id: String },
}

impl PullRequestReviewThreadAction {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Reply { .. } => "Reply",
            Self::CopyComment { .. } => "Copy latest comment",
            Self::OpenComment { .. } => "Open latest on GitHub",
            Self::EditComment { .. } => "Edit latest comment",
            Self::DeleteComment { .. } => "Delete latest comment",
            Self::Resolve { .. } => "Resolve thread",
            Self::Reopen { .. } => "Reopen thread",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestTreeEntry {
    Directory {
        path: PathBuf,
        label: String,
        depth: usize,
    },
    File {
        index: usize,
        depth: usize,
    },
}

impl PullRequestTreeEntry {
    pub(crate) const fn depth(&self) -> usize {
        match self {
            Self::Directory { depth, .. } | Self::File { depth, .. } => *depth,
        }
    }

    pub(super) const fn directory_depth(&self) -> Option<usize> {
        match self {
            Self::Directory { depth, .. } => Some(*depth),
            Self::File { .. } => None,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PullRequestTreeNode {
    path: PathBuf,
    directories: BTreeMap<OsString, Self>,
    files: Vec<usize>,
}

impl PullRequestTreeNode {
    pub(super) fn insert(&mut self, path: &Path, index: usize) {
        let components = path.components().collect::<Vec<_>>();
        let mut directory = PathBuf::new();
        let mut node = self;
        for component in components.iter().take(components.len().saturating_sub(1)) {
            directory.push(component.as_os_str());
            node = node
                .directories
                .entry(component.as_os_str().to_os_string())
                .or_insert_with(|| Self {
                    path: directory.clone(),
                    ..Self::default()
                });
        }
        node.files.push(index);
    }

    pub(super) fn append_entries(
        &self,
        depth: usize,
        collapsed: &HashSet<PathBuf>,
        entries: &mut Vec<PullRequestTreeEntry>,
    ) {
        for directory in self.directories.values() {
            let mut compacted = directory;
            let mut label = compacted
                .path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            while compacted.files.is_empty() && compacted.directories.len() == 1 {
                let Some(child) = compacted.directories.values().next() else {
                    break;
                };
                label.push('/');
                label.push_str(
                    &child
                        .path
                        .file_name()
                        .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
                );
                compacted = child;
            }
            entries.push(PullRequestTreeEntry::Directory {
                path: compacted.path.clone(),
                label,
                depth,
            });
            if !collapsed.contains(&compacted.path) {
                compacted.append_entries(depth.saturating_add(1), collapsed, entries);
            }
        }
        entries.extend(self.files.iter().map(|index| PullRequestTreeEntry::File {
            index: *index,
            depth,
        }));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenTarget {
    Browser(String),
}

#[derive(Debug, Clone)]
pub(crate) struct LinkHit {
    pub area: Rect,
    pub target: OpenTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextSelection {
    pub pane: Rect,
    pub anchor: (u16, u16),
    pub head: (u16, u16),
}

impl TextSelection {
    pub(crate) const fn ordered_endpoints(self) -> ((u16, u16), (u16, u16)) {
        if self.anchor.1 < self.head.1
            || (self.anchor.1 == self.head.1 && self.anchor.0 <= self.head.0)
        {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeTarget {
    Sidebar,
    Diff,
}
