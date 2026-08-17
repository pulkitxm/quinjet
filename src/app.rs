use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fmt::Write;
use std::mem::size_of_val;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::convert::{count, offset};
use crate::git::diff::{DiffDocument, DiffIndex, DiffLineKind, PullRequestDetails};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PullRequest, PullRequestCheck, PullRequestConversation,
    PullRequestDiffIndex, PullRequestFile, PullRequestFileStatus, PullRequestProgress,
};
use crate::git::history::Commit;
use crate::git::status::{Change, ChangeArea, ChangeStatus, RepoStatus};
use crate::git::worker::{WorkerCommand, WorkerEvent};
use crate::git::{Branch, ConflictChoice, GitOperation, HistoryBranch, LocalDiffRequest, Stash};
use crate::theme::{Appearance, AppearanceChoice, Theme, ThemeName};

const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(45);
const RESIZE_DOUBLE_TAP_INTERVAL: Duration = Duration::from_millis(450);
const TOAST_DURATION: Duration = Duration::from_secs(4);
const HISTORY_PAGE_SIZE: usize = 300;
const PULL_REQUEST_PREFETCH_BATCH: usize = 12;
const MAX_PREFETCHED_PULL_REQUEST_FILES: usize = 400;
const MAX_PULL_REQUEST_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
/// Poll cadences for an open pull request. A run in progress changes state in
/// seconds and is worth watching closely; a settled pull request only needs to
/// notice new comments; a pull request nobody is looking at needs less again.
const PULL_REQUEST_ACTIVE_POLL: Duration = Duration::from_secs(5);
const PULL_REQUEST_IDLE_POLL: Duration = Duration::from_secs(20);
const PULL_REQUEST_BACKGROUND_POLL: Duration = Duration::from_secs(120);
/// Each live stream costs its own GitHub requests, so the tick cadence is a
/// ceiling rather than a schedule: check state is the only thing worth reading
/// as often as the tick fires. Metadata, the conversation and a growing log all
/// change on human or build timescales and hold their own floor.
const PULL_REQUEST_DETAIL_POLL: Duration = Duration::from_secs(20);
/// A running job's log grows continuously, so this is a tail interval rather
/// than a staleness bound.
const PULL_REQUEST_LOG_POLL: Duration = Duration::from_secs(8);
const MAX_PULL_REQUEST_NUMBER_DIGITS: usize = 20;
const DEFAULT_SIDEBAR_WIDTH: u16 = 42;
const MIN_SIDEBAR_WIDTH: u16 = 22;
const MIN_CONTENT_WIDTH: u16 = 32;
const DEFAULT_DIFF_SPLIT_PERCENT: u16 = 50;
const MIN_DIFF_SPLIT_PERCENT: u16 = 20;
const MAX_DIFF_SPLIT_PERCENT: u16 = 80;
const OPERATION_SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];

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
    Untracked,
}

impl ChangeSection {
    pub(crate) const ALL: [Self; 4] = [
        Self::Conflict,
        Self::Staged,
        Self::Unstaged,
        Self::Untracked,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Conflict => "Merge Changes",
            Self::Staged => "Staged Changes",
            Self::Unstaged => "Changes",
            Self::Untracked => "Untracked Changes",
        }
    }

    pub(crate) fn matches(self, change: &Change) -> bool {
        match self {
            Self::Conflict => change.area == ChangeArea::Conflict,
            Self::Staged => change.area == ChangeArea::Staged,
            Self::Unstaged => {
                change.area == ChangeArea::Unstaged && change.status != ChangeStatus::Untracked
            }
            Self::Untracked => change.status == ChangeStatus::Untracked,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffLayout {
    Unified,
    SideBySide,
}

/// The two halves of the pull-request view. `Overview` lists the pull request's
/// checks beside its conversation; `Files` lists the changed files beside their
/// diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestSection {
    Overview,
    Files,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestFileView {
    AllFiles,
    SingleFile,
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

    const fn directory_depth(&self) -> Option<usize> {
        match self {
            Self::Directory { depth, .. } => Some(*depth),
            Self::File { .. } => None,
        }
    }
}

#[derive(Debug, Default)]
struct PullRequestTreeNode {
    path: PathBuf,
    directories: BTreeMap<OsString, Self>,
    files: Vec<usize>,
}

impl PullRequestTreeNode {
    fn insert(&mut self, path: &Path, index: usize) {
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

    fn append_entries(
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
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub(crate) struct LinkHit {
    pub area: Rect,
    pub target: OpenTarget,
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

#[derive(Debug, Clone, Default)]
pub(crate) struct TextBuffer {
    pub value: String,
    pub cursor: usize,
}

impl TextBuffer {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self { value, cursor }
    }

    pub(crate) fn insert(&mut self, character: char) {
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub(crate) fn insert_str(&mut self, text: &str) {
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    /// The text on each side of the cursor, never panicking when the cursor
    /// sits between two bytes of the same character.
    pub(crate) fn before_cursor(&self) -> &str {
        self.value.get(..self.cursor).unwrap_or_default()
    }

    pub(crate) fn after_cursor(&self) -> &str {
        self.value.get(self.cursor..).unwrap_or_default()
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self
            .before_cursor()
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or_default();
        drop(self.value.drain(previous..self.cursor));
        self.cursor = previous;
    }

    pub(crate) fn delete(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let length = self
            .after_cursor()
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or_default();
        drop(self.value.drain(self.cursor..self.cursor + length));
    }

    pub(crate) fn move_left(&mut self) {
        self.cursor = self
            .before_cursor()
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or_default();
    }

    pub(crate) fn move_right(&mut self) {
        if let Some(character) = self.after_cursor().chars().next() {
            self.cursor += character.len_utf8();
        }
    }

    pub(crate) fn home(&mut self) {
        let line_start = self
            .before_cursor()
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.cursor = line_start;
    }

    pub(crate) fn end(&mut self) {
        self.cursor = self
            .after_cursor()
            .find('\n')
            .map_or(self.value.len(), |index| self.cursor + index);
    }

    pub(crate) const fn document_start(&mut self) {
        self.cursor = 0;
    }

    pub(crate) const fn document_end(&mut self) {
        self.cursor = self.value.len();
    }

    pub(crate) fn delete_word_backward(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut start = self.cursor;
        while let Some((index, character)) = previous_character(&self.value, start) {
            if !character.is_whitespace() {
                break;
            }
            start = index;
        }
        while let Some((index, character)) = previous_character(&self.value, start) {
            if !is_word_character(character) {
                break;
            }
            start = index;
        }
        if start == self.cursor
            && let Some((index, _)) = previous_character(&self.value, start)
        {
            start = index;
        }
        drop(self.value.drain(start..self.cursor));
        self.cursor = start;
    }

    pub(crate) fn delete_word_forward(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let mut end = self.cursor;
        while let Some((next, character)) = next_character(&self.value, end) {
            if !character.is_whitespace() {
                break;
            }
            end = next;
        }
        while let Some((next, character)) = next_character(&self.value, end) {
            if !is_word_character(character) {
                break;
            }
            end = next;
        }
        if end == self.cursor
            && let Some((next, _)) = next_character(&self.value, end)
        {
            end = next;
        }
        drop(self.value.drain(self.cursor..end));
    }

    pub(crate) fn delete_to_line_start(&mut self) {
        let start = self
            .before_cursor()
            .rfind('\n')
            .map_or(0, |index| index + 1);
        drop(self.value.drain(start..self.cursor));
        self.cursor = start;
    }

    pub(crate) fn delete_to_line_end(&mut self) {
        let end = self
            .after_cursor()
            .find('\n')
            .map_or(self.value.len(), |index| self.cursor + index);
        drop(self.value.drain(self.cursor..end));
    }

    pub(crate) fn move_word_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut cursor = self.cursor;
        while let Some((index, character)) = previous_character(&self.value, cursor) {
            if !character.is_whitespace() {
                break;
            }
            cursor = index;
        }
        while let Some((index, character)) = previous_character(&self.value, cursor) {
            if !is_word_character(character) {
                break;
            }
            cursor = index;
        }
        self.cursor = cursor;
    }

    pub(crate) fn move_word_right(&mut self) {
        let mut cursor = self.cursor;
        while let Some((next, character)) = next_character(&self.value, cursor) {
            if !is_word_character(character) {
                break;
            }
            cursor = next;
        }
        while let Some((next, character)) = next_character(&self.value, cursor) {
            if !character.is_whitespace() {
                break;
            }
            cursor = next;
        }
        self.cursor = cursor;
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PromptKind {
    Filter {
        previous: String,
    },
    CreateBranch {
        start: Option<String>,
    },
    RenameBranch {
        old: String,
    },
    StashPush {
        include_untracked: bool,
        staged: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum Modal {
    Help {
        scroll: usize,
    },
    Commit {
        input: TextBuffer,
        amend: bool,
    },
    Prompt {
        title: String,
        input: TextBuffer,
        kind: PromptKind,
    },
    Confirm {
        title: String,
        message: String,
        operation: GitOperation,
    },
    Branches {
        items: Vec<Branch>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
    },
    HistoryBranches {
        items: Vec<HistoryBranch>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
    },
    CompareBranches {
        items: Vec<HistoryBranch>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
    },
    Stashes {
        items: Vec<Stash>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
    },
    PullRequestRepositories {
        items: Vec<GitHubRepository>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
    },
    CommandPalette {
        query: TextBuffer,
        selected: usize,
    },
    Themes {
        selected: usize,
        original: ThemeName,
    },
    Appearances {
        selected: usize,
    },
    Conflict {
        change: Change,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteCommand {
    Refresh,
    StageAll,
    UnstageAll,
    Commit,
    Amend,
    Fetch,
    Pull,
    Push,
    Sync,
    Stash,
    StashStaged,
    StashIncludeUntracked,
    StashPop,
    ManageStashes,
    Branches,
    CompareBranch,
    RenameCurrentBranch,
    ToggleDiffLayout,
    ToggleAllFiles,
    ShowChanges,
    ShowHistory,
    ShowPullRequests,
    ChangeTheme,
    ChangeAppearance,
    Help,
    Quit,
}

impl PaletteCommand {
    pub(crate) const ALL: [Self; 26] = [
        Self::Refresh,
        Self::StageAll,
        Self::UnstageAll,
        Self::Commit,
        Self::Amend,
        Self::Fetch,
        Self::Pull,
        Self::Push,
        Self::Sync,
        Self::Stash,
        Self::StashStaged,
        Self::StashIncludeUntracked,
        Self::StashPop,
        Self::ManageStashes,
        Self::Branches,
        Self::CompareBranch,
        Self::RenameCurrentBranch,
        Self::ToggleDiffLayout,
        Self::ToggleAllFiles,
        Self::ShowChanges,
        Self::ShowHistory,
        Self::ShowPullRequests,
        Self::ChangeTheme,
        Self::ChangeAppearance,
        Self::Help,
        Self::Quit,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Refresh => "Refresh Repository",
            Self::StageAll => "Stage All Changes",
            Self::UnstageAll => "Unstage All Changes",
            Self::Commit => "Commit Staged Changes",
            Self::Amend => "Amend Previous Commit",
            Self::Fetch => "Fetch All Remotes",
            Self::Pull => "Pull",
            Self::Push => "Push",
            Self::Sync => "Synchronize (Pull, Then Push)",
            Self::Stash => "Stash Changes…",
            Self::StashStaged => "Stash Staged Changes…",
            Self::StashIncludeUntracked => "Stash Changes Including Untracked…",
            Self::StashPop => "Pop Latest Stash",
            Self::ManageStashes => "View and Manage Stashes…",
            Self::Branches => "Switch Branch…",
            Self::CompareBranch => "Compare Current Branch With…",
            Self::RenameCurrentBranch => "Rename Current Branch…",
            Self::ToggleDiffLayout => "Toggle Unified / Side-by-Side Diff",
            Self::ToggleAllFiles => "Collapse / Expand All Files",
            Self::ShowChanges => "Show Changes",
            Self::ShowHistory => "Show Commit History",
            Self::ShowPullRequests => "Show Pull Requests",
            Self::ChangeTheme => "Change Theme…",
            Self::ChangeAppearance => "Change Appearance…",
            Self::Help => "Keyboard Shortcuts",
            Self::Quit => "Quit Quinjet",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SidebarHit {
    ChangeSection(ChangeSection),
    Change(usize),
    Commit(usize),
    PullRequestFiles,
    PullRequestOverview,
    PullRequestRefresh,
    PullRequestConversation,
    PullRequestConversationRefresh,
    PullRequestChooseRepository,
    PullRequestLookup,
    PullRequestDirectory(PathBuf),
    PullRequestFile(usize),
    PullRequestCheck(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct SidebarHitArea {
    pub area: Rect,
    pub target: SidebarHit,
}

#[derive(Debug, Clone)]
pub(crate) struct ContentFileHit {
    pub area: Rect,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct ContentStepHit {
    pub area: Rect,
    pub step: usize,
}

pub(crate) struct PullRequestContentRow {
    pub line: Line<'static>,
    pub step: Option<usize>,
    pub wide: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScmAction {
    Stage(usize),
    Unstage(usize),
    Resolve(usize),
    StageSection(ChangeSection),
    UnstageSection(ChangeSection),
    StageAll,
    UnstageAll,
    Commit,
    Stashes,
    CompareBranch,
}

#[derive(Debug, Clone)]
pub(crate) struct ScmActionHit {
    pub area: Rect,
    pub action: ScmAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuxiliaryPreview {
    Branch(HistoryBranch),
    Stash(Stash),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    variant_size_differences,
    reason = "both variants are pointer-sized in practice and boxing would cost an allocation per row"
)]
enum ChangeTarget {
    Section(ChangeSection),
    Change(usize),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UiGeometry {
    pub changes_tab: Rect,
    pub history_tab: Rect,
    pub pull_requests_tab: Rect,
    pub main: Rect,
    pub sidebar: Rect,
    pub sidebar_divider: Rect,
    pub content: Rect,
    pub diff_divider: Option<Rect>,
    pub sidebar_hits: Vec<SidebarHitArea>,
    pub scm_action_hits: Vec<ScmActionHit>,
    pub content_file_hits: Vec<ContentFileHit>,
    pub content_step_hits: Vec<ContentStepHit>,
    pub link_hits: Vec<LinkHit>,
}

#[derive(Debug)]
pub(crate) enum AppEffect {
    Git(Box<WorkerCommand>),
    SetMouseCapture(bool),
    Open(OpenTarget),
    Quit,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent piece of interface state"
)]
pub(crate) struct App {
    pub repository_root: PathBuf,
    pub repository_name: String,
    pub view: View,
    pub focus: Focus,
    pub diff_layout: DiffLayout,
    pub theme: Theme,
    pub theme_name: ThemeName,
    pub appearance_choice: AppearanceChoice,
    pub appearance: Appearance,
    pub status: RepoStatus,
    pub history: Vec<Commit>,
    pub history_branch: Option<HistoryBranch>,
    pub pull_request: Option<PullRequest>,
    pub github_repositories: Vec<GitHubRepository>,
    pub local_github_repository: Option<GitHubRepository>,
    pub pull_request_repository: Option<GitHubRepository>,
    pub pull_request_warnings: Vec<String>,
    /// Why the last lookup failed. The pull-request pane renders app state
    /// rather than a document, so a failure needs somewhere to live that
    /// outlasts the toast announcing it.
    pub pull_request_error: Option<String>,
    pub pull_request_exact_number: Option<u64>,
    pub pull_request_from_cache: bool,
    pub history_branches: Vec<HistoryBranch>,
    pub history_branches_loading: bool,
    pub history_branches_loaded: bool,
    pub pull_request_lookup: TextBuffer,
    pub pull_request_lookup_active: bool,
    pub pull_request_section: PullRequestSection,
    pub pull_request_file_view: PullRequestFileView,
    pub pull_request_files: Vec<PullRequestFile>,
    pub pull_request_tree: Vec<PullRequestTreeEntry>,
    pub pull_request_total_files: usize,
    pub pull_request_files_truncated: bool,
    pub pull_request_file_cursor: usize,
    pub pull_request_tree_cursor: usize,
    pub collapsed_pull_request_directories: HashSet<PathBuf>,
    pub pull_request_checks: Vec<PullRequestCheck>,
    /// `None` keeps the content pane on the pull request itself; selecting a
    /// check replaces it with that run's steps and log.
    pub pull_request_check_cursor: Option<usize>,
    pub pull_request_checks_loading: bool,
    pub pull_request_checks_error: Option<String>,
    pub pull_request_checks_from_cache: bool,
    pub pull_request_prefetched_logs: HashSet<String>,
    pub pull_request_conversation: PullRequestConversation,
    pub pull_request_conversation_loading: bool,
    pub pull_request_conversation_refresh_again: bool,
    pub pull_request_conversation_error: Option<String>,
    pub pull_request_check_log: Option<CheckRunLog>,
    pub pull_request_check_log_loading: bool,
    pub pull_request_check_log_error: Option<String>,
    pub expanded_check_steps: HashSet<usize>,
    pub pull_request_step_cursor: usize,
    /// Set when the step selection moves, and cleared by the draw that acts on
    /// it. Scrolling the selection into view on every frame instead would pin
    /// the pane to the selected step and make its own output unreadable.
    pub pull_request_step_reveal: bool,
    pub pull_request_content_rows: Vec<PullRequestContentRow>,
    pub pull_request_content_rows_key: Option<(bool, usize, u64, u64, u64, u64, u64)>,
    pub pull_request_content_generation: u64,
    /// Whether the last draw left the content pane scrolled to its end. The
    /// renderer owns the row count, so it reports this back for the one decision
    /// that needs it: whether a growing log should keep following.
    pub content_at_bottom: bool,
    pub pull_request_progress: Option<PullRequestProgress>,
    pub auxiliary_preview: Option<AuxiliaryPreview>,
    pub document: DiffDocument,
    pub selected_change_section: Option<ChangeSection>,
    pub collapsed_change_sections: HashSet<ChangeSection>,
    pub selected_preview_file: Option<PathBuf>,
    pub preview_file_cursor: usize,
    pub collapsed_preview_files: HashSet<PathBuf>,
    pub expanded_preview_files: HashSet<PathBuf>,
    pub change_cursor: usize,
    pub history_cursor: usize,
    pub sidebar_offset: usize,
    pub content_scroll: usize,
    pub horizontal_scroll: usize,
    pub sidebar_width: u16,
    pub sidebar_hidden: bool,
    pub diff_split_percent: u16,
    pub expanded_diff: bool,
    pub files_collapsed: bool,
    pub collapse_preference_set: bool,
    pub resize_target: Option<ResizeTarget>,
    pub filter: String,
    pub modal: Option<Modal>,
    pub toast: Option<Toast>,
    pub mouse_capture: bool,
    pub webhooks_listening: bool,
    pub busy: Option<String>,
    pub operation_frame: usize,
    pub refreshing: bool,
    pub document_loading: bool,
    pub history_loading: bool,
    pub history_complete: bool,
    pub pull_request_loading: bool,
    pub last_refresh: Option<Instant>,
    pub geometry: UiGeometry,
    pub status_generation: u64,
    pub changes_diff_version: u64,
    pub diff_generation: u64,
    pub history_generation: u64,
    pub pull_request_generation: u64,
    /// Repository discovery answers on its own counter. Sharing the lookup's
    /// would let opening the picker discard a pull request already on its way,
    /// leaving its loading flag set with no reply ever able to clear it.
    pub repository_generation: u64,
    pub pull_request_workspace_generation: Option<u64>,
    pub pull_request_documents: HashMap<PathBuf, DiffDocument>,
    pub pull_request_document_order: VecDeque<PathBuf>,
    pub pull_request_document_bytes: usize,
    pub pull_request_prefetched_paths: HashSet<PathBuf>,
    pub pull_request_loading_path: Option<PathBuf>,
    /// The path whose patch currently occupies `document` in single-file view.
    /// Tracking it explicitly keeps the cache authoritative about which files
    /// already have a patch, wherever that patch happens to be held.
    pub pull_request_single_file: Option<PathBuf>,
    pub pull_request_prefetching: bool,
    pub pull_request_prefetch_retrying: bool,
    pub pull_request_checks_generation: u64,
    pub pull_request_conversation_generation: u64,
    pub pull_request_check_log_generation: u64,
    pub pull_request_check_log_target: Option<String>,
    pub local_diff_request: Option<LocalDiffRequest>,
    pub local_diff_workspace_generation: Option<u64>,
    pub local_diff_index: Option<DiffIndex>,
    pub local_diff_documents: HashMap<PathBuf, DiffDocument>,
    pub local_diff_loading_path: Option<PathBuf>,
    pub local_diff_pending_paths: VecDeque<PathBuf>,
    pub local_diff_single_loaded: bool,
    pub branch_generation: u64,
    pub history_branch_generation: u64,
    pub stash_generation: u64,
    pub operation_id: u64,
    pub refresh_again: bool,
    pub history_refresh_again: bool,
    pub preview_due: Option<Instant>,
    pub pull_request_poll_due: Option<Instant>,
    pub pull_request_checks_read_at: Option<Instant>,
    pub pull_request_detail_read_at: Option<Instant>,
    pub pull_request_log_read_at: Option<Instant>,
    pub pending_g: Option<Instant>,
    pub last_resize_tap: Option<(ResizeTarget, Instant)>,
}

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "the constructor explicitly initializes every independent state field"
    )]
    pub(crate) fn new(root: impl AsRef<Path>, name: impl Into<String>) -> Self {
        Self {
            repository_root: root.as_ref().to_path_buf(),
            repository_name: name.into(),
            view: View::Changes,
            focus: Focus::Sidebar,
            diff_layout: DiffLayout::SideBySide,
            theme: Theme::default(),
            theme_name: ThemeName::default(),
            appearance_choice: AppearanceChoice::default(),
            appearance: Appearance::Dark,
            status: RepoStatus::default(),
            history: Vec::new(),
            history_branch: None,
            pull_request: None,
            github_repositories: Vec::new(),
            local_github_repository: None,
            pull_request_repository: None,
            pull_request_warnings: Vec::new(),
            pull_request_error: None,
            pull_request_exact_number: None,
            pull_request_from_cache: false,
            history_branches: Vec::new(),
            history_branches_loading: false,
            history_branches_loaded: false,
            pull_request_lookup: TextBuffer::default(),
            pull_request_lookup_active: false,
            pull_request_section: PullRequestSection::Overview,
            pull_request_file_view: PullRequestFileView::AllFiles,
            pull_request_files: Vec::new(),
            pull_request_tree: Vec::new(),
            pull_request_total_files: 0,
            pull_request_files_truncated: false,
            pull_request_file_cursor: 0,
            pull_request_tree_cursor: 0,
            collapsed_pull_request_directories: HashSet::new(),
            pull_request_checks: Vec::new(),
            pull_request_check_cursor: None,
            pull_request_checks_loading: false,
            pull_request_checks_error: None,
            pull_request_checks_from_cache: false,
            pull_request_prefetched_logs: HashSet::new(),
            pull_request_conversation: PullRequestConversation::default(),
            pull_request_conversation_loading: false,
            pull_request_conversation_refresh_again: false,
            pull_request_conversation_error: None,
            pull_request_check_log: None,
            pull_request_check_log_loading: false,
            pull_request_check_log_error: None,
            expanded_check_steps: HashSet::new(),
            pull_request_step_cursor: 0,
            pull_request_step_reveal: false,
            pull_request_content_rows: Vec::new(),
            pull_request_content_rows_key: None,
            pull_request_content_generation: 0,
            content_at_bottom: true,
            pull_request_progress: None,
            auxiliary_preview: None,
            document: DiffDocument::empty("Working Tree", "Loading changes…"),
            selected_change_section: Some(ChangeSection::Unstaged),
            collapsed_change_sections: HashSet::new(),
            selected_preview_file: None,
            preview_file_cursor: 0,
            collapsed_preview_files: HashSet::new(),
            expanded_preview_files: HashSet::new(),
            change_cursor: 0,
            history_cursor: 0,
            sidebar_offset: 0,
            content_scroll: 0,
            horizontal_scroll: 0,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_hidden: false,
            diff_split_percent: DEFAULT_DIFF_SPLIT_PERCENT,
            expanded_diff: false,
            files_collapsed: false,
            collapse_preference_set: false,
            resize_target: None,
            filter: String::new(),
            modal: None,
            toast: None,
            mouse_capture: true,
            webhooks_listening: false,
            busy: None,
            operation_frame: 0,
            refreshing: false,
            document_loading: false,
            history_loading: false,
            history_complete: false,
            pull_request_loading: false,
            last_refresh: None,
            geometry: UiGeometry::default(),
            status_generation: 0,
            changes_diff_version: 0,
            diff_generation: 0,
            history_generation: 0,
            pull_request_generation: 0,
            repository_generation: 0,
            pull_request_workspace_generation: None,
            pull_request_documents: HashMap::new(),
            pull_request_document_order: VecDeque::new(),
            pull_request_document_bytes: 0,
            pull_request_prefetched_paths: HashSet::new(),
            pull_request_loading_path: None,
            pull_request_single_file: None,
            pull_request_prefetching: false,
            pull_request_prefetch_retrying: false,
            pull_request_checks_generation: 0,
            pull_request_conversation_generation: 0,
            pull_request_check_log_generation: 0,
            pull_request_check_log_target: None,
            local_diff_request: None,
            local_diff_workspace_generation: None,
            local_diff_index: None,
            local_diff_documents: HashMap::new(),
            local_diff_loading_path: None,
            local_diff_pending_paths: VecDeque::new(),
            local_diff_single_loaded: false,
            branch_generation: 0,
            history_branch_generation: 0,
            stash_generation: 0,
            operation_id: 0,
            refresh_again: false,
            history_refresh_again: false,
            preview_due: None,
            pull_request_poll_due: None,
            pull_request_checks_read_at: None,
            pull_request_detail_read_at: None,
            pull_request_log_read_at: None,
            pending_g: None,
            last_resize_tap: None,
        }
    }

    pub(crate) fn set_theme_selection(&mut self, name: ThemeName, choice: AppearanceChoice) {
        let appearance = choice.resolve();
        self.theme = Theme::new(name, appearance);
        self.theme_name = name;
        self.appearance_choice = choice;
        self.appearance = appearance;
    }

    pub(crate) fn initial_effects(&mut self) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        self.request_refresh(&mut effects);
        self.request_history(true, &mut effects);
        self.request_history_branches(&mut effects);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadLocalGitHubRepository,
        )));
        effects
    }

    pub(crate) fn visible_change_indices(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        self.status
            .changes
            .iter()
            .enumerate()
            .filter(|(_, change)| {
                query.is_empty() || change.display_path().to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn visible_commit_indices(&self) -> Vec<usize> {
        let query = self.filter.to_lowercase();
        self.history
            .iter()
            .enumerate()
            .filter(|(_, commit)| {
                query.is_empty()
                    || commit.subject.to_lowercase().contains(&query)
                    || commit.author.to_lowercase().contains(&query)
                    || commit.id.starts_with(&query)
                    || commit
                        .decorations
                        .iter()
                        .any(|decoration| decoration.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn history_branch_label(&self) -> String {
        self.history_branch.as_ref().map_or_else(
            || {
                if self.status.branch.head.is_empty() {
                    "HEAD".to_owned()
                } else {
                    self.status.branch.head.clone()
                }
            },
            |branch| branch.name.clone(),
        )
    }

    fn history_revision(&self) -> String {
        self.history_branch
            .as_ref()
            .map_or_else(|| "HEAD".to_owned(), |branch| branch.reference.clone())
    }

    pub(crate) const fn selected_pull_request(&self) -> Option<&PullRequest> {
        self.pull_request.as_ref()
    }

    pub(crate) fn selected_pull_request_file(&self) -> Option<&PullRequestFile> {
        self.pull_request_files.get(self.pull_request_file_cursor)
    }

    pub(crate) fn selected_pull_request_check(&self) -> Option<&PullRequestCheck> {
        self.pull_request_check_cursor
            .and_then(|cursor| self.pull_request_checks.get(cursor))
    }

    fn rebuild_pull_request_tree(&mut self) {
        let mut entries = Vec::with_capacity(self.pull_request_files.len().saturating_mul(2));
        let mut root = PullRequestTreeNode::default();
        for (index, file) in self.pull_request_files.iter().enumerate() {
            root.insert(&file.path, index);
        }
        root.append_entries(0, &self.collapsed_pull_request_directories, &mut entries);
        self.pull_request_tree = entries;
    }

    pub(crate) fn pull_request_tree_entries(&mut self) -> &[PullRequestTreeEntry] {
        if self.pull_request_tree.is_empty() && !self.pull_request_files.is_empty() {
            self.rebuild_pull_request_tree();
        }
        &self.pull_request_tree
    }

    pub(crate) fn pull_request_directory_collapsed(&self, path: &Path) -> bool {
        self.collapsed_pull_request_directories.contains(path)
    }

    fn sync_pull_request_tree_cursor_to_file(&mut self) {
        self.rebuild_pull_request_tree();
        self.pull_request_tree_cursor = self
            .pull_request_tree
            .iter()
            .position(|entry| {
                matches!(
                    entry,
                    PullRequestTreeEntry::File { index, .. }
                        if *index == self.pull_request_file_cursor
                )
            })
            .unwrap_or_default();
    }

    fn select_pull_request_tree_entry(&mut self, cursor: usize, now: Instant) {
        let entries = self.pull_request_tree_entries();
        let length = entries.len();
        let Some(entry) = entries.get(cursor.min(length.saturating_sub(1))).cloned() else {
            self.pull_request_tree_cursor = 0;
            return;
        };
        self.pull_request_tree_cursor = cursor.min(length - 1);
        if let PullRequestTreeEntry::File { index, .. } = entry {
            let changed_file = index != self.pull_request_file_cursor;
            let entering_single_file =
                self.pull_request_file_view != PullRequestFileView::SingleFile;
            if changed_file || entering_single_file {
                self.pull_request_file_cursor = index;
                self.pull_request_file_view = PullRequestFileView::SingleFile;
                self.content_scroll = 0;
                self.horizontal_scroll = 0;
                self.schedule_preview(now);
            }
        }
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the caller has no use for the value afterwards"
    )]
    fn toggle_pull_request_directory(&mut self, path: PathBuf) {
        if !self.collapsed_pull_request_directories.remove(&path) {
            let _ = self.collapsed_pull_request_directories.insert(path.clone());
        }
        self.rebuild_pull_request_tree();
        self.pull_request_tree_cursor = self
            .pull_request_tree
            .iter()
            .position(|entry| {
                matches!(entry, PullRequestTreeEntry::Directory { path: entry_path, .. } if entry_path == &path)
            })
            .unwrap_or_default();
    }

    fn toggle_selected_pull_request_directory(&mut self) -> bool {
        if self.view != View::PullRequests
            || self.pull_request_section != PullRequestSection::Files
            || self.focus != Focus::Sidebar
        {
            return false;
        }
        let cursor = self.pull_request_tree_cursor;
        let Some(PullRequestTreeEntry::Directory { path, .. }) =
            self.pull_request_tree_entries().get(cursor).cloned()
        else {
            return false;
        };
        self.toggle_pull_request_directory(path);
        true
    }

    fn navigate_pull_request_tree_horizontal(&mut self, expand: bool, now: Instant) {
        if self.pull_request_tree_entries().is_empty() {
            return;
        }
        let entries = &self.pull_request_tree;
        let Some(entry) = entries.get(self.pull_request_tree_cursor).cloned() else {
            return;
        };
        match entry {
            PullRequestTreeEntry::Directory { path, depth, .. } => {
                let collapsed = self.pull_request_directory_collapsed(&path);
                if (expand && collapsed) || (!expand && !collapsed) {
                    self.toggle_pull_request_directory(path);
                } else if expand {
                    let child = self.pull_request_tree_cursor.saturating_add(1);
                    if entries
                        .get(child)
                        .is_some_and(|entry| entry.depth() > depth)
                    {
                        self.select_pull_request_tree_entry(child, now);
                    }
                } else {
                    let parent_cursor =
                        entries
                            .get(..self.pull_request_tree_cursor)
                            .and_then(|parents| {
                                parents.iter().rposition(|entry| {
                                    entry
                                        .directory_depth()
                                        .is_some_and(|parent_depth| parent_depth < depth)
                                })
                            });
                    if let Some(cursor) = parent_cursor {
                        self.pull_request_tree_cursor = cursor;
                    }
                }
            }
            PullRequestTreeEntry::File { depth, .. } if !expand => {
                let parent_cursor =
                    entries
                        .get(..self.pull_request_tree_cursor)
                        .and_then(|parents| {
                            parents.iter().rposition(|entry| {
                                entry
                                    .directory_depth()
                                    .is_some_and(|parent_depth| parent_depth < depth)
                            })
                        });
                if let Some(cursor) = parent_cursor {
                    self.pull_request_tree_cursor = cursor;
                }
            }
            PullRequestTreeEntry::File { .. } => {}
        }
    }

    pub(crate) fn local_diff_load_progress(&self) -> Option<(usize, usize)> {
        self.local_diff_index.as_ref().map(|index| {
            let loaded = if index.files.len() == 1 && self.local_diff_single_loaded {
                1
            } else {
                index
                    .files
                    .iter()
                    .filter(|file| self.local_diff_documents.contains_key(&file.path))
                    .count()
            };
            (loaded, index.files.len())
        })
    }

    pub(crate) fn local_diff_line_counts(&self) -> Option<(usize, usize)> {
        self.local_diff_index.as_ref().map(|index| {
            let counts = index.line_counts();
            (counts.additions, counts.deletions)
        })
    }

    pub(crate) fn selected_change(&self) -> Option<&Change> {
        let visible = self.visible_change_indices();
        visible
            .get(self.change_cursor)
            .and_then(|index| self.status.changes.get(*index))
    }

    pub(crate) fn selected_section_changes(&self) -> Vec<Change> {
        let Some(section) = self.selected_change_section else {
            return Vec::new();
        };
        self.visible_change_indices()
            .into_iter()
            .filter_map(|index| self.status.changes.get(index))
            .filter(|change| section.matches(change))
            .cloned()
            .collect()
    }

    pub(crate) fn change_rows(&self) -> Vec<ChangeRow> {
        let visible = self.visible_change_indices();
        let mut rows = Vec::new();
        for section in ChangeSection::ALL {
            let members = visible
                .iter()
                .enumerate()
                .filter_map(|(cursor, index)| {
                    self.status
                        .changes
                        .get(*index)
                        .filter(|change| section.matches(change))
                        .map(|_| (*index, cursor))
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let collapsed = self.collapsed_change_sections.contains(&section);
            rows.push(ChangeRow::Section {
                section,
                count: members.len(),
                collapsed,
            });
            if !collapsed {
                rows.extend(
                    members
                        .into_iter()
                        .map(|(index, cursor)| ChangeRow::Change {
                            section,
                            index,
                            cursor,
                        }),
                );
            }
        }
        rows
    }

    fn change_targets(&self) -> Vec<ChangeTarget> {
        self.change_rows()
            .into_iter()
            .map(|row| match row {
                ChangeRow::Section { section, .. } => ChangeTarget::Section(section),
                ChangeRow::Change { cursor, .. } => ChangeTarget::Change(cursor),
            })
            .collect()
    }

    fn selected_change_target(&self) -> Option<ChangeTarget> {
        self.selected_change_section
            .map(ChangeTarget::Section)
            .or(Some(ChangeTarget::Change(self.change_cursor)))
            .filter(|target| self.change_targets().contains(target))
    }

    pub(crate) fn preview_file_selected(&self, path: &str) -> bool {
        self.selected_preview_file
            .as_deref()
            .is_some_and(|selected| selected.to_string_lossy() == path)
    }

    pub(crate) fn preview_files_collapsible(&self) -> bool {
        if let Some(index) = self.local_diff_index.as_ref() {
            return index.files.len() > 1;
        }
        if self.view == View::PullRequests {
            return self.pull_request_file_view == PullRequestFileView::AllFiles
                && self.pull_request_files.len() > 1;
        }
        self.document
            .lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .take(2)
            .count()
            > 1
    }

    pub(crate) fn preview_file_collapsed(&self, path: &str) -> bool {
        if !self.preview_files_collapsible() {
            return false;
        }
        if self.files_collapsed {
            !self.expanded_preview_files.contains(Path::new(path))
        } else {
            self.collapsed_preview_files.contains(Path::new(path))
        }
    }

    fn toggle_all_preview_files(&mut self, effects: &mut Vec<AppEffect>) {
        if !self.preview_files_collapsible() {
            return;
        }
        self.files_collapsed = !self.preview_files_all_collapsed();
        self.collapse_preference_set = true;
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        self.content_scroll = 0;
        self.rebuild_indexed_preview_document();
        if self.files_collapsed {
            self.local_diff_pending_paths.clear();
        } else {
            for path in self.preview_file_paths() {
                self.request_local_diff_file(path, effects);
            }
        }
    }

    fn toggle_preview_file(&mut self, path: PathBuf, effects: &mut Vec<AppEffect>) {
        if !self.preview_files_collapsible() {
            return;
        }
        let was_collapsed = self.preview_file_collapsed(&path.to_string_lossy());
        let overrides = if self.files_collapsed {
            &mut self.expanded_preview_files
        } else {
            &mut self.collapsed_preview_files
        };
        if !overrides.remove(&path) {
            let _ = overrides.insert(path.clone());
        }
        self.selected_preview_file = Some(path.clone());
        self.preview_file_cursor = self
            .preview_file_paths()
            .iter()
            .position(|candidate| candidate == &path)
            .unwrap_or_default();
        let is_collapsed = self.preview_file_collapsed(&path.to_string_lossy());
        self.rebuild_indexed_preview_document();
        if was_collapsed && !is_collapsed {
            if self.view == View::PullRequests
                && self.pull_request_file_view == PullRequestFileView::AllFiles
            {
                self.request_pull_request_diff_file(path, false, effects);
            } else {
                self.request_local_diff_file(path, effects);
            }
        }
    }

    pub(crate) fn preview_files_all_collapsed(&self) -> bool {
        let paths = self.preview_file_paths();
        paths.len() > 1
            && paths
                .iter()
                .all(|path| self.preview_file_collapsed(&path.to_string_lossy()))
    }

    fn preview_file_paths(&self) -> Vec<PathBuf> {
        self.document
            .lines
            .iter()
            .filter(|line| line.kind == DiffLineKind::FileHeader)
            .filter_map(|line| line.spans.first())
            .map(|span| PathBuf::from(span.text.split("  · ").next().unwrap_or(span.text.as_str())))
            .collect()
    }

    fn navigate_preview_file(&mut self, amount: isize) {
        let paths = self.preview_file_paths();
        if paths.is_empty() {
            return;
        }
        let current = self
            .selected_preview_file
            .as_ref()
            .and_then(|selected| paths.iter().position(|path| path == selected))
            .unwrap_or_else(|| self.preview_file_cursor.min(paths.len() - 1));
        self.preview_file_cursor = if amount < 0 {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            (current + count(amount)).min(paths.len() - 1)
        };
        self.selected_preview_file = paths.get(self.preview_file_cursor).cloned();
        if let Some(line_index) = self.document.lines.iter().position(|line| {
            line.kind == DiffLineKind::FileHeader
                && line.spans.first().is_some_and(|span| {
                    span.text.split("  · ").next().is_some_and(|path| {
                        paths
                            .get(self.preview_file_cursor)
                            .is_some_and(|selected| Path::new(path) == selected)
                    })
                })
        }) {
            self.content_scroll = line_index;
        }
    }

    const fn select_change_target(&mut self, target: ChangeTarget) {
        match target {
            ChangeTarget::Section(section) => self.selected_change_section = Some(section),
            ChangeTarget::Change(cursor) => {
                self.selected_change_section = None;
                self.change_cursor = cursor;
            }
        }
    }

    fn toggle_change_section(&mut self, section: ChangeSection) {
        if !self.collapsed_change_sections.remove(&section) {
            let _ = self.collapsed_change_sections.insert(section);
        }
        self.selected_change_section = Some(section);
    }

    fn toggle_selected_change_section(&mut self) -> bool {
        let Some(section) = self.selected_change_section else {
            return false;
        };
        self.toggle_change_section(section);
        true
    }

    fn navigate_change_section_horizontal(&mut self, expand: bool, now: Instant) {
        let Some(target) = self.selected_change_target() else {
            return;
        };
        match target {
            ChangeTarget::Section(section) => {
                let collapsed = self.collapsed_change_sections.contains(&section);
                if (expand && collapsed) || (!expand && !collapsed) {
                    self.toggle_change_section(section);
                } else if expand {
                    let targets = self.change_targets();
                    if let Some(index) = targets
                        .iter()
                        .position(|candidate| *candidate == target)
                        .and_then(|index| targets.get(index.saturating_add(1)))
                        .copied()
                        .filter(|candidate| matches!(candidate, ChangeTarget::Change(_)))
                    {
                        self.select_change_target(index);
                        self.schedule_preview(now);
                    }
                }
            }
            ChangeTarget::Change(_) if !expand => {
                let Some(change) = self.selected_change() else {
                    return;
                };
                if let Some(section) = ChangeSection::ALL
                    .into_iter()
                    .find(|section| section.matches(change))
                {
                    self.selected_change_section = Some(section);
                    self.schedule_preview(now);
                }
            }
            ChangeTarget::Change(_) => {}
        }
    }

    pub(crate) fn selected_commit(&self) -> Option<&Commit> {
        let visible = self.visible_commit_indices();
        visible
            .get(self.history_cursor)
            .and_then(|index| self.history.get(*index))
    }

    #[expect(
        clippy::unused_self,
        reason = "the method belongs to the app surface even without state"
    )]
    pub(crate) fn palette_commands(&self, query: &str) -> Vec<PaletteCommand> {
        let words: Vec<_> = query
            .split_ascii_whitespace()
            .map(str::to_lowercase)
            .collect();
        PaletteCommand::ALL
            .into_iter()
            .filter(|command| {
                let label = command.label().to_lowercase();
                words.iter().all(|word| label.contains(word))
            })
            .collect()
    }

    pub(crate) fn filtered_branches(items: &[Branch], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, branch)| query.is_empty() || branch.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_history_branches(items: &[HistoryBranch], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, branch)| query.is_empty() || branch.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_stashes(items: &[Stash], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, stash)| {
                query.is_empty()
                    || stash.reference.to_lowercase().contains(&query)
                    || stash.message.to_lowercase().contains(&query)
                    || stash.branch.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn filtered_github_repositories(
        items: &[GitHubRepository],
        query: &str,
    ) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, repository)| {
                query.is_empty()
                    || repository.display_name().to_lowercase().contains(&query)
                    || repository
                        .remotes
                        .iter()
                        .any(|remote| remote.to_lowercase().contains(&query))
            })
            .map(|(index, _)| index)
            .collect()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Vec<AppEffect> {
        if let Some(modal) = self.modal.take() {
            return self.handle_modal_key(modal, key, now);
        }
        if self.pull_request_lookup_active {
            return self.handle_pull_request_lookup_key(key, now);
        }

        let mut effects = Vec::new();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('p') => {
                    self.modal = Some(Modal::CommandPalette {
                        query: TextBuffer::default(),
                        selected: 0,
                    });
                }
                KeyCode::Char('r') => self.request_active_refresh(&mut effects),
                KeyCode::Char('d') => self.scroll_content_half(true),
                KeyCode::Char('u') => self.scroll_content_half(false),
                _ => {}
            }
            return effects;
        }

        match key.code {
            KeyCode::Char('q') => effects.push(AppEffect::Quit),
            KeyCode::Char('?') => self.modal = Some(Modal::Help { scroll: 0 }),
            KeyCode::Char(':') => {
                self.modal = Some(Modal::CommandPalette {
                    query: TextBuffer::default(),
                    selected: 0,
                });
            }
            KeyCode::Char('1') => self.switch_view(View::Changes, &mut effects),
            KeyCode::Char('2') => self.switch_view(View::History, &mut effects),
            KeyCode::Char('3') => self.switch_view(View::PullRequests, &mut effects),
            KeyCode::Tab | KeyCode::BackTab if !self.sidebar_hidden => {
                self.toggle_focus();
            }
            KeyCode::Char('r') => self.request_active_refresh(&mut effects),
            KeyCode::Char('/') if self.view == View::PullRequests => {
                self.pull_request_lookup_active = true;
                self.focus = Focus::Sidebar;
            }
            KeyCode::Char('/') => {
                self.modal = Some(Modal::Prompt {
                    title: "Filter".to_owned(),
                    input: TextBuffer::new(self.filter.clone()),
                    kind: PromptKind::Filter {
                        previous: self.filter.clone(),
                    },
                });
            }
            KeyCode::Char('v') => self.toggle_diff_layout(),
            KeyCode::Char('e' | 'E') if self.check_log_visible() => {
                self.toggle_all_check_steps();
            }
            KeyCode::Char('e' | 'E') => self.toggle_all_preview_files(&mut effects),
            KeyCode::Char('t' | 'T') => {
                self.expanded_diff = !self.expanded_diff;
                self.content_scroll = 0;
                self.request_preview(&mut effects);
            }
            KeyCode::Char('b') if self.view == View::History => {
                self.open_history_branches(&mut effects);
            }
            KeyCode::Char('b' | 'B') => self.open_branches(&mut effects),
            KeyCode::Char('d') if self.view == View::Changes => {
                self.open_compare_branches(&mut effects);
            }
            KeyCode::Char('S') if self.view == View::Changes => self.open_stashes(&mut effects),
            KeyCode::Char('o') if self.view == View::PullRequests => {
                self.open_pull_request_repositories(&mut effects);
            }
            KeyCode::Char('c') if self.view == View::Changes => {
                self.modal = Some(Modal::Commit {
                    input: TextBuffer::default(),
                    amend: false,
                });
            }
            KeyCode::Char('a') if self.view == View::Changes => {
                self.queue_operation(GitOperation::StageAll, &mut effects);
            }
            KeyCode::Char('U') if self.view == View::Changes => {
                self.queue_operation(GitOperation::UnstageAll, &mut effects);
            }
            KeyCode::Char('s' | ' ')
                if self.view == View::Changes
                    && self.focus == Focus::Sidebar
                    && self.selected_change_section.is_none() =>
            {
                self.toggle_stage_selected(&mut effects);
            }
            KeyCode::Char(' ') if self.view == View::Changes && self.focus == Focus::Sidebar => {
                let _ = self.toggle_selected_change_section();
            }
            KeyCode::Char(' ')
                if self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files
                    && self.focus == Focus::Sidebar =>
            {
                let _ = self.toggle_selected_pull_request_directory();
            }
            KeyCode::Char(' ') if self.check_log_visible() => {
                self.toggle_check_step(self.pull_request_step_cursor);
            }
            KeyCode::Char(' ')
                if self.focus == Focus::Content
                    && (self.local_diff_index.is_some()
                        || (self.view == View::PullRequests
                            && self.pull_request_file_view == PullRequestFileView::AllFiles)) =>
            {
                if let Some(path) = self.selected_preview_file.clone() {
                    self.toggle_preview_file(path, &mut effects);
                }
            }
            KeyCode::Char('u')
                if self.view == View::Changes && self.selected_change_section.is_none() =>
            {
                self.unstage_selected(&mut effects);
            }
            KeyCode::Char('x')
                if self.view == View::Changes && self.selected_change_section.is_none() =>
            {
                self.confirm_discard();
            }
            KeyCode::Char('P') if self.view == View::PullRequests => {
                self.select_pull_request_section(PullRequestSection::Overview, &mut effects);
            }
            KeyCode::Char('F') if self.view == View::PullRequests => {
                self.select_pull_request_section(PullRequestSection::Files, &mut effects);
            }
            KeyCode::Char('C') if self.view == View::History => self.confirm_cherry_pick(),
            KeyCode::Char('R') if self.view == View::History => self.confirm_revert(),
            KeyCode::Char('n') if self.view == View::History => self.prompt_branch_at_commit(),
            KeyCode::Char('f' | 'p') if self.view == View::PullRequests => {
                self.show_toast(
                    "Fetch and push live in Changes · Shift+P and Shift+F switch section"
                        .to_owned(),
                    ToastLevel::Error,
                    now,
                );
            }
            KeyCode::Char('f') => self.queue_operation(GitOperation::Fetch, &mut effects),
            KeyCode::Char('p') => self.queue_operation(GitOperation::Push, &mut effects),
            KeyCode::Char('l')
                if self.focus == Focus::Sidebar
                    && !(self.view == View::PullRequests
                        && self.pull_request_section == PullRequestSection::Files) =>
            {
                self.queue_operation(GitOperation::Pull, &mut effects);
            }
            KeyCode::Char('y') => self.queue_operation(GitOperation::Sync, &mut effects),
            KeyCode::Enter if self.check_log_visible() && self.focus == Focus::Content => {
                self.toggle_check_step(self.pull_request_step_cursor);
            }
            KeyCode::Enter if !self.sidebar_hidden => {
                if !(self.toggle_selected_pull_request_directory()
                    || self.view == View::Changes
                        && self.focus == Focus::Sidebar
                        && self.toggle_selected_change_section())
                {
                    self.toggle_focus();
                }
            }
            KeyCode::Esc => {
                if self.auxiliary_preview.take().is_some() {
                    self.request_preview(&mut effects);
                } else if self.view == View::PullRequests
                    && (self.pull_request_exact_number.is_some() || self.pull_request.is_some())
                {
                    self.invalidate_preview();
                    self.pull_request_exact_number = None;
                    self.pull_request = None;
                    self.reset_pull_request_runtime();
                    self.pull_request_warnings.clear();
                    self.pull_request_error = None;
                    self.pull_request_progress = None;
                    self.pull_request_poll_due = None;
                    self.pull_request_lookup = TextBuffer::default();
                    self.pull_request_lookup_active = true;
                    self.document = DiffDocument::empty(
                        "Open Pull Request",
                        "Enter a pull-request number and press Enter",
                    );
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                    self.normalize_selection();
                    self.schedule_preview(now);
                } else {
                    self.focus = Focus::Sidebar;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.navigate(-1, now),
            KeyCode::Down | KeyCode::Char('j') => self.navigate(1, now),
            KeyCode::PageUp => self.page(-1, now),
            KeyCode::PageDown => self.page(1, now),
            KeyCode::Home => self.go_to_edge(false, now),
            KeyCode::End | KeyCode::Char('G') => self.go_to_edge(true, now),
            KeyCode::Char('g') => {
                if self
                    .pending_g
                    .is_some_and(|pressed| now.duration_since(pressed) < Duration::from_millis(500))
                {
                    self.go_to_edge(false, now);
                    self.pending_g = None;
                } else {
                    self.pending_g = Some(now);
                }
            }
            KeyCode::Char('z') => self.toggle_sidebar(),
            KeyCode::Char('m') => effects.push(self.toggle_mouse_capture(now)),
            KeyCode::Char('O') if self.view == View::PullRequests => {
                self.open_selection_on_github(&mut effects, now);
            }
            KeyCode::Char('[') if self.check_log_visible() => {
                let _ = self.move_check_step_cursor(-1);
            }
            KeyCode::Char(']') if self.check_log_visible() => {
                let _ = self.move_check_step_cursor(1);
            }
            KeyCode::Char('[' | ']')
                if self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Overview => {}
            KeyCode::Char('[') => self.jump_hunk(false),
            KeyCode::Char(']') => self.jump_hunk(true),
            KeyCode::Left | KeyCode::Char('h')
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files =>
            {
                self.navigate_pull_request_tree_horizontal(false, now);
            }
            KeyCode::Right | KeyCode::Char('l')
                if self.focus == Focus::Sidebar
                    && self.view == View::PullRequests
                    && self.pull_request_section == PullRequestSection::Files =>
            {
                self.navigate_pull_request_tree_horizontal(true, now);
            }
            KeyCode::Left if self.focus == Focus::Sidebar && self.view == View::Changes => {
                self.navigate_change_section_horizontal(false, now);
            }
            KeyCode::Right if self.focus == Focus::Sidebar && self.view == View::Changes => {
                self.navigate_change_section_horizontal(true, now);
            }
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Content => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_sub(4);
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Content => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_add(4);
            }
            _ => {}
        }
        effects
    }

    pub(crate) fn handle_paste(&mut self, text: &str) {
        if self.pull_request_lookup_active {
            let remaining =
                MAX_PULL_REQUEST_NUMBER_DIGITS.saturating_sub(self.pull_request_lookup.value.len());
            let digits = text
                .chars()
                .filter(char::is_ascii_digit)
                .take(remaining)
                .collect::<String>();
            self.pull_request_lookup.insert_str(&digits);
            return;
        }
        if let Some(
            Modal::Commit { input, .. }
            | Modal::Prompt { input, .. }
            | Modal::CommandPalette { query: input, .. }
            | Modal::Branches { query: input, .. }
            | Modal::HistoryBranches { query: input, .. }
            | Modal::CompareBranches { query: input, .. }
            | Modal::Stashes { query: input, .. }
            | Modal::PullRequestRepositories { query: input, .. },
        ) = self.modal.as_mut()
        {
            input.insert_str(text);
        }
        self.apply_live_modal_filter();
    }

    #[expect(
        clippy::excessive_nesting,
        reason = "the key handler mirrors the shape of the input it decodes"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_mouse(&mut self, event: MouseEvent, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        if self.modal.is_some() || event.modifiers.contains(KeyModifiers::SHIFT) {
            return effects;
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(target) = self
                    .geometry
                    .link_hits
                    .iter()
                    .find(|hit| hit.area.contains((event.column, event.row).into()))
                    .map(|hit| hit.target.clone())
                {
                    effects.push(AppEffect::Open(target));
                } else if self
                    .geometry
                    .sidebar_divider
                    .contains((event.column, event.row).into())
                {
                    self.begin_resize(ResizeTarget::Sidebar, event.column, now);
                } else if self
                    .geometry
                    .diff_divider
                    .is_some_and(|divider| divider.contains((event.column, event.row).into()))
                {
                    self.begin_resize(ResizeTarget::Diff, event.column, now);
                } else {
                    self.last_resize_tap = None;
                    if self
                        .geometry
                        .changes_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::Changes, &mut effects);
                    } else if self
                        .geometry
                        .history_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::History, &mut effects);
                    } else if self
                        .geometry
                        .pull_requests_tab
                        .contains((event.column, event.row).into())
                    {
                        self.switch_view(View::PullRequests, &mut effects);
                    } else if let Some(action) = self
                        .geometry
                        .scm_action_hits
                        .iter()
                        .find(|hit| hit.area.contains((event.column, event.row).into()))
                        .map(|hit| hit.action.clone())
                    {
                        self.handle_scm_action(action, &mut effects);
                    } else if self
                        .geometry
                        .sidebar
                        .contains((event.column, event.row).into())
                    {
                        self.focus = Focus::Sidebar;
                        if let Some(hit) = self
                            .geometry
                            .sidebar_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.target.clone())
                        {
                            match hit {
                                SidebarHit::ChangeSection(section) => {
                                    self.auxiliary_preview = None;
                                    self.toggle_change_section(section);
                                    self.schedule_preview(now);
                                }
                                SidebarHit::Change(index) => {
                                    if let Some(cursor) = self
                                        .visible_change_indices()
                                        .iter()
                                        .position(|visible| *visible == index)
                                    {
                                        self.auxiliary_preview = None;
                                        self.selected_change_section = None;
                                        self.change_cursor = cursor;
                                        self.schedule_preview(now);
                                    }
                                }
                                SidebarHit::Commit(index) => {
                                    if let Some(cursor) = self
                                        .visible_commit_indices()
                                        .iter()
                                        .position(|visible| *visible == index)
                                    {
                                        self.history_cursor = cursor;
                                        self.schedule_preview(now);
                                    }
                                }
                                SidebarHit::PullRequestFiles => self.select_pull_request_section(
                                    PullRequestSection::Files,
                                    &mut effects,
                                ),
                                SidebarHit::PullRequestOverview => self
                                    .select_pull_request_section(
                                        PullRequestSection::Overview,
                                        &mut effects,
                                    ),
                                SidebarHit::PullRequestRefresh => {
                                    self.refresh_pull_request_live(now, true, &mut effects);
                                }
                                SidebarHit::PullRequestConversation => {
                                    self.select_pull_request_check(None, &mut effects);
                                }
                                SidebarHit::PullRequestConversationRefresh => {
                                    self.request_pull_request_conversation(true, &mut effects);
                                }
                                SidebarHit::PullRequestChooseRepository => {
                                    self.open_pull_request_repositories(&mut effects);
                                }
                                SidebarHit::PullRequestLookup => {
                                    self.pull_request_lookup_active = true;
                                }
                                SidebarHit::PullRequestDirectory(path) => {
                                    self.toggle_pull_request_directory(path);
                                }
                                SidebarHit::PullRequestFile(index) => {
                                    if let Some(cursor) =
                                        self.pull_request_tree_entries().iter().position(|entry| {
                                            matches!(
                                                entry,
                                                PullRequestTreeEntry::File {
                                                    index: entry_index,
                                                    ..
                                                } if *entry_index == index
                                            )
                                        })
                                    {
                                        self.select_pull_request_tree_entry(cursor, now);
                                    }
                                }
                                SidebarHit::PullRequestCheck(index) => {
                                    if index < self.pull_request_checks.len() {
                                        self.select_pull_request_check(Some(index), &mut effects);
                                    }
                                }
                            }
                        }
                    } else if self
                        .geometry
                        .content
                        .contains((event.column, event.row).into())
                    {
                        self.focus = Focus::Content;
                        if let Some(step) = self
                            .geometry
                            .content_step_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.step)
                        {
                            self.toggle_check_step(step);
                        } else if let Some(path) = self
                            .geometry
                            .content_file_hits
                            .iter()
                            .find(|hit| hit.area.contains((event.column, event.row).into()))
                            .map(|hit| hit.path.clone())
                        {
                            self.toggle_preview_file(path, &mut effects);
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.last_resize_tap = None;
                match self.resize_target {
                    Some(ResizeTarget::Sidebar) => self.resize_sidebar(event.column),
                    Some(ResizeTarget::Diff) => self.resize_diff(event.column),
                    None => {}
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.resize_target = None,
            MouseEventKind::ScrollDown => {
                if self
                    .geometry
                    .sidebar
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Sidebar;
                    self.navigate(1, now);
                } else {
                    self.focus = Focus::Content;
                    self.content_scroll = self.content_scroll.saturating_add(2);
                }
            }
            MouseEventKind::ScrollUp => {
                if self
                    .geometry
                    .sidebar
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Sidebar;
                    self.navigate(-1, now);
                } else {
                    self.focus = Focus::Content;
                    self.content_scroll = self.content_scroll.saturating_sub(2);
                }
            }
            MouseEventKind::ScrollLeft => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_sub(3);
            }
            MouseEventKind::ScrollRight => {
                self.horizontal_scroll = self.horizontal_scroll.saturating_add(3);
            }
            _ => {}
        }
        effects
    }

    pub(crate) fn tick(&mut self, now: Instant) -> (Vec<AppEffect>, bool) {
        let toast_expired = self
            .toast
            .as_ref()
            .is_some_and(|toast| now >= toast.expires_at);
        let mut changed = toast_expired;
        if toast_expired {
            self.toast = None;
        }
        if self
            .pending_g
            .is_some_and(|pressed| now.duration_since(pressed) >= Duration::from_millis(500))
        {
            self.pending_g = None;
        }

        let mut effects = Vec::new();
        if self.preview_due.is_some_and(|due| now >= due) {
            self.preview_due = None;
            self.request_preview(&mut effects);
            changed = true;
        }
        if self.pull_request_poll_due.is_some_and(|due| now >= due) {
            self.refresh_pull_request_live(now, false, &mut effects);
            changed = true;
        }
        if self.busy.is_some() {
            self.operation_frame = self.operation_frame.wrapping_add(1) % OPERATION_SPINNER.len();
            changed = true;
        }
        (effects, changed)
    }

    pub(crate) fn operation_spinner(&self) -> &'static str {
        OPERATION_SPINNER
            .get(self.operation_frame % OPERATION_SPINNER.len())
            .copied()
            .unwrap_or("◐")
    }

    /// A GitHub webhook was forwarded to this session. The payload is only a
    /// hint that something changed, so the poller runs immediately rather than
    /// trying to apply the delivery itself.
    pub(crate) fn webhook_delivered(&mut self, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        if self.pull_request.is_some() {
            self.refresh_pull_request_live(now, true, &mut effects);
        }
        effects
    }

    /// Whether any pull-request read is on its way. The view shows one reload
    /// mark for all of them, because the reader cares that it is refreshing,
    /// not which of four endpoints is answering.
    pub(crate) const fn pull_request_refreshing(&self) -> bool {
        self.pull_request_loading
            || self.pull_request_checks_loading
            || self.pull_request_conversation_loading
            || self.pull_request_check_log_loading
    }

    /// Whether the pull request itself was answered from disk rather than the
    /// network. Check state is deliberately held for only thirty seconds, so
    /// including it here made the answer almost always false and the label
    /// never appeared at all.
    pub(crate) const fn pull_request_served_from_cache(&self) -> bool {
        self.pull_request.is_some()
            && self.pull_request_from_cache
            && self.pull_request_conversation.from_cache
            && !self.pull_request_refreshing()
    }

    /// A forwarded delivery bypasses every floor, so when one can arrive the
    /// poll interval is the fallback rather than the promise.
    pub(crate) fn live_refresh_label(&self) -> String {
        let interval = self.pull_request_poll_interval().as_secs();
        if self.webhooks_listening {
            format!("webhooks · every {interval}s")
        } else {
            format!("every {interval}s")
        }
    }

    /// Watch a running pull request closely and a settled one loosely. The
    /// interval also stretches when the reader is somewhere else, so a loaded
    /// pull request stays fresh without spending requests on an unseen pane.
    fn pull_request_poll_interval(&self) -> Duration {
        if self.view != View::PullRequests {
            return PULL_REQUEST_BACKGROUND_POLL;
        }
        if self
            .pull_request_checks
            .iter()
            .any(|check| check.status.is_running())
        {
            PULL_REQUEST_ACTIVE_POLL
        } else {
            PULL_REQUEST_IDLE_POLL
        }
    }

    fn schedule_pull_request_poll(&mut self, now: Instant) {
        self.pull_request_poll_due = self
            .pull_request
            .is_some()
            .then(|| now + self.pull_request_poll_interval());
    }

    /// Run whichever live reads are due. `force` is a webhook delivery saying
    /// something definitely changed, so every stream reads at once.
    ///
    /// Each read is independent, so a single failing endpoint never stalls the
    /// others, and every one of them coalesces if a previous poll is still in
    /// flight.
    fn refresh_pull_request_live(
        &mut self,
        now: Instant,
        force: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        self.schedule_pull_request_poll(now);
        let Some(number) = self.pull_request_exact_number else {
            return;
        };
        if self.pull_request.is_none() {
            return;
        }
        let due = |last: Option<Instant>, interval: Duration| {
            force || last.is_none_or(|last| now.duration_since(last) >= interval)
        };

        if due(
            self.pull_request_checks_read_at,
            self.pull_request_poll_interval(),
        ) {
            let issued = effects.len();
            self.request_pull_request_checks(true, effects);
            if effects.len() > issued {
                self.pull_request_checks_read_at = Some(now);
            }
        }
        if due(self.pull_request_detail_read_at, PULL_REQUEST_DETAIL_POLL) {
            let issued = effects.len();
            self.request_pull_request_lookup(number, true, true, effects);
            self.request_pull_request_conversation(true, effects);
            if effects.len() > issued {
                self.pull_request_detail_read_at = Some(now);
            }
        }
        let running = self
            .selected_pull_request_check()
            .is_some_and(|check| check.status.is_running());
        if running && due(self.pull_request_log_read_at, PULL_REQUEST_LOG_POLL) {
            let issued = effects.len();
            self.request_check_run_log(true, effects);
            if effects.len() > issued {
                self.pull_request_log_read_at = Some(now);
            }
        }
    }

    pub(crate) fn filesystem_changed(&mut self, effects: &mut Vec<AppEffect>) {
        self.changes_diff_version = self.changes_diff_version.wrapping_add(1);
        if self.view == View::Changes && self.auxiliary_preview.is_none() {
            self.invalidate_preview();
            self.local_diff_loading_path = None;
        }
        self.request_refresh(effects);
    }

    /// The repository heartbeat. Pull-request liveness is separate because it
    /// paces itself against GitHub rather than the local working tree.
    pub(crate) fn periodic_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        self.request_refresh(effects);
    }

    #[expect(
        clippy::excessive_nesting,
        reason = "the key handler mirrors the shape of the input it decodes"
    )]
    #[expect(
        clippy::expect_used,
        reason = "the value was assigned on the line above"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    pub(crate) fn handle_worker_event(
        &mut self,
        event: WorkerEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            WorkerEvent::Status { generation, result } => {
                if generation != self.status_generation {
                    return effects;
                }
                self.refreshing = false;
                match result {
                    Ok(status) => {
                        let selected = self
                            .selected_change_section
                            .is_none()
                            .then(|| self.selected_change().cloned())
                            .flatten();
                        let branch_was_known =
                            !self.status.branch.head.is_empty() || self.status.branch.oid.is_some();
                        let branch_changed = self.status.branch.head != status.branch.head
                            || self.status.branch.oid != status.branch.oid;
                        self.status = status;
                        self.restore_change_selection(selected.as_ref());
                        self.last_refresh = Some(now);
                        if branch_changed
                            && self.history_branch.is_none()
                            && (branch_was_known || !self.history_loading)
                        {
                            self.request_history(true, &mut effects);
                        }
                        if self.view == View::Changes {
                            self.preview_due = None;
                            self.request_preview(&mut effects);
                        }
                    }
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
                }
                if self.refresh_again {
                    self.refresh_again = false;
                    self.request_refresh(&mut effects);
                }
            }
            WorkerEvent::LocalDiffIndex { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.document_loading = false;
                self.local_diff_loading_path = None;
                match result {
                    Ok(index) => {
                        let selected_path = self.selected_preview_file.clone().filter(|selected| {
                            index.files.iter().any(|file| &file.path == selected)
                        });
                        self.local_diff_workspace_generation = Some(generation);
                        self.local_diff_documents.clear();
                        self.collapsed_preview_files.clear();
                        self.expanded_preview_files.clear();
                        if index.files.len() > 1
                            && !self.files_collapsed
                            && !self.collapse_preference_set
                        {
                            self.collapsed_preview_files
                                .extend(index.files.iter().map(|file| file.path.clone()));
                        }
                        self.selected_preview_file = selected_path
                            .or_else(|| index.files.first().map(|file| file.path.clone()));
                        self.preview_file_cursor = self
                            .selected_preview_file
                            .as_ref()
                            .and_then(|selected| {
                                index.files.iter().position(|file| &file.path == selected)
                            })
                            .unwrap_or_default();
                        let first_path = self.selected_preview_file.clone();
                        self.local_diff_index = Some(index);
                        self.rebuild_local_diff_document();
                        self.content_scroll = 0;
                        self.horizontal_scroll = 0;
                        let mut paths = self
                            .local_diff_index
                            .as_ref()
                            .map(|index| {
                                index
                                    .files
                                    .iter()
                                    .filter(|file| {
                                        !self.preview_file_collapsed(&file.path.to_string_lossy())
                                    })
                                    .map(|file| file.path.clone())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        if let Some(path) = first_path
                            && !paths.contains(&path)
                        {
                            paths.insert(0, path);
                        }
                        for path in paths {
                            self.request_local_diff_file(path, &mut effects);
                        }
                    }
                    Err(error) => {
                        self.reset_local_diff_runtime();
                        self.document = DiffDocument::empty("Preview Error", error.clone());
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::LocalDiffFile {
                generation,
                workspace_generation,
                path,
                result,
            } => {
                if generation != self.diff_generation
                    || self.local_diff_workspace_generation != Some(workspace_generation)
                    || self.local_diff_loading_path.as_ref() != Some(&path)
                    || !self
                        .local_diff_index
                        .as_ref()
                        .is_some_and(|index| index.files.iter().any(|file| file.path == path))
                {
                    return effects;
                }
                self.local_diff_loading_path = None;
                match result {
                    Ok(document) => {
                        if self
                            .local_diff_index
                            .as_ref()
                            .is_some_and(|index| index.files.len() == 1)
                        {
                            self.document = document;
                            self.local_diff_single_loaded = true;
                        } else {
                            drop(self.local_diff_documents.insert(path, document));
                            self.rebuild_local_diff_document();
                        }
                    }
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
                }
                self.request_next_local_diff_file(&mut effects);
            }
            WorkerEvent::PullRequestIndex { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.pull_request_progress = None;
                match result {
                    Ok(index) => {
                        self.apply_pull_request_index(index);
                        self.pull_request_workspace_generation = Some(generation);
                        self.sidebar_offset = 0;
                        self.content_scroll = 0;
                        self.horizontal_scroll = 0;
                        self.document_loading = false;
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(error) => {
                        self.document_loading = false;
                        self.pull_request_workspace_generation = None;
                        self.document = DiffDocument::empty("Preview Error", error.clone());
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::PullRequestDiff { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.document_loading = false;
                self.pull_request_progress = None;
                let requested_path = self.pull_request_loading_path.take();
                match result {
                    Ok(document) => {
                        let path = requested_path.or_else(|| {
                            document
                                .pull_request_details
                                .as_ref()
                                .and_then(|details| details.selected_file.as_ref())
                                .map(PathBuf::from)
                        });
                        match self.pull_request_file_view {
                            PullRequestFileView::AllFiles => {
                                if let Some(path) = path {
                                    self.cache_pull_request_document(path, document);
                                    self.rebuild_pull_request_all_files_document();
                                } else {
                                    self.document = document;
                                }
                            }
                            PullRequestFileView::SingleFile => {
                                self.cache_current_pull_request_single_document();
                                self.document = document;
                                self.pull_request_single_file = path;
                                self.selected_preview_file = None;
                                self.preview_file_cursor = 0;
                                self.content_scroll = 0;
                                self.horizontal_scroll = 0;
                            }
                        }
                    }
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
                }
                self.request_pull_request_prefetch(&mut effects);
            }
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation,
                result,
            } => {
                if Some(workspace_generation) != self.pull_request_workspace_generation {
                    return effects;
                }
                self.pull_request_prefetching = false;
                match result {
                    Ok(documents) => {
                        self.pull_request_prefetch_retrying = false;
                        for (path, document) in documents {
                            if !self.pull_request_documents.contains_key(&path) {
                                self.cache_pull_request_document(path, document);
                            }
                        }
                        if self.pull_request_file_view == PullRequestFileView::AllFiles {
                            self.rebuild_pull_request_all_files_document();
                        }
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(_) if !self.pull_request_prefetch_retrying => {
                        self.pull_request_prefetch_retrying = true;
                        self.request_pull_request_prefetch(&mut effects);
                    }
                    Err(_) => {
                        self.pull_request_prefetch_retrying = false;
                    }
                }
            }
            WorkerEvent::PullRequestChecks { generation, result } => {
                if generation != self.pull_request_checks_generation {
                    return effects;
                }
                self.pull_request_checks_loading = false;
                match result {
                    Ok(snapshot) => {
                        let selected = self
                            .selected_pull_request_check()
                            .map(PullRequestCheck::identity);
                        let was_running = self
                            .selected_pull_request_check()
                            .is_some_and(|check| check.status.is_running());
                        self.pull_request_checks_from_cache = snapshot.from_cache;
                        self.pull_request_checks = snapshot.checks;
                        self.invalidate_pull_request_content_rows();
                        let cursor = selected.and_then(|selected| {
                            self.pull_request_checks
                                .iter()
                                .position(|check| check.identity() == selected)
                        });
                        let _ = self.set_check_cursor(cursor);
                        self.pull_request_checks_error = None;
                        if was_running {
                            self.request_check_run_log(true, &mut effects);
                        }
                        self.request_check_log_prefetch(&mut effects);
                    }
                    Err(error) => {
                        self.pull_request_checks_error = Some(error);
                        self.invalidate_pull_request_content_rows();
                    }
                }
            }
            WorkerEvent::CheckRunLog { generation, result } => {
                if generation != self.pull_request_check_log_generation {
                    return effects;
                }
                self.pull_request_check_log_loading = false;
                let following = self.content_at_bottom
                    && self
                        .selected_pull_request_check()
                        .is_some_and(|check| check.status.is_running());
                match result {
                    Ok(log) => {
                        if self.expanded_check_steps.is_empty()
                            && let Some(step) = log.failed_step().or_else(|| log.running_step())
                        {
                            let number = step.number;
                            let _ = self.expanded_check_steps.insert(number);
                            self.reveal_check_step(number);
                        }
                        if self.pull_request_step_cursor == 0
                            && let Some(step) = log.steps.first()
                        {
                            let number = step.number;
                            self.reveal_check_step(number);
                        }
                        self.pull_request_check_log = Some(log);
                        self.invalidate_pull_request_content_rows();
                        self.pull_request_check_log_error = None;
                        if following {
                            self.content_scroll = usize::MAX;
                        }
                    }
                    Err(error) => {
                        self.pull_request_check_log = None;
                        self.pull_request_check_log_error = Some(error);
                        self.invalidate_pull_request_content_rows();
                    }
                }
            }
            WorkerEvent::PullRequestConversation { generation, result } => {
                if generation != self.pull_request_conversation_generation {
                    return effects;
                }
                self.pull_request_conversation_loading = false;
                match result {
                    Ok(conversation) => {
                        self.pull_request_conversation = conversation;
                        self.pull_request_conversation_error = None;
                        self.invalidate_pull_request_content_rows();
                    }
                    Err(error) => {
                        self.pull_request_conversation_error = Some(error);
                        self.invalidate_pull_request_content_rows();
                    }
                }
                if self.pull_request_conversation_refresh_again {
                    self.pull_request_conversation_refresh_again = false;
                    self.request_pull_request_conversation(true, &mut effects);
                }
            }
            WorkerEvent::History {
                generation,
                skip,
                result,
            } => {
                if generation != self.history_generation {
                    return effects;
                }
                self.history_loading = false;
                match result {
                    Ok(commits) => {
                        let received = commits.len();
                        if skip == 0 {
                            self.history = commits;
                            self.history_cursor = self
                                .history_cursor
                                .min(self.visible_commit_indices().len().saturating_sub(1));
                        } else if skip == self.history.len() {
                            self.history.extend(commits);
                        }
                        self.history_complete = received < HISTORY_PAGE_SIZE;
                        if self.view == View::History {
                            self.schedule_preview(now);
                        }
                    }
                    Err(error) => {
                        if self.history_branch.take().is_some() {
                            self.show_toast(
                                format!(
                                    "Viewed branch is unavailable; returning to HEAD history: {error}"
                                ),
                                ToastLevel::Error,
                                now,
                            );
                            self.request_history(true, &mut effects);
                        } else {
                            self.show_toast(error, ToastLevel::Error, now);
                        }
                    }
                }
                if self.history_refresh_again {
                    self.history_refresh_again = false;
                    self.request_history(true, &mut effects);
                }
            }
            WorkerEvent::GitHubRepositories { generation, result } => {
                if generation != self.repository_generation {
                    return effects;
                }
                match result {
                    Ok((repositories, warnings)) => {
                        self.github_repositories = repositories;
                        self.pull_request_warnings = warnings;
                        if let Some(Modal::PullRequestRepositories {
                            items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            items.clone_from(&self.github_repositories);
                            *selected = self
                                .pull_request_repository
                                .as_ref()
                                .and_then(|current| {
                                    items.iter().position(|repository| {
                                        repository.url.eq_ignore_ascii_case(&current.url)
                                    })
                                })
                                .unwrap_or_default();
                            *loading = false;
                        }
                    }
                    Err(error) => {
                        if matches!(self.modal, Some(Modal::PullRequestRepositories { .. })) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::LocalGitHubRepository { result } => {
                if let Ok(repository) = result {
                    self.local_github_repository = repository;
                }
            }
            WorkerEvent::PullRequestLookup { generation, result } => {
                if generation != self.pull_request_generation {
                    return effects;
                }
                self.pull_request_loading = false;
                match result {
                    Ok(snapshot) => {
                        if !snapshot.repositories.is_empty() {
                            self.github_repositories = snapshot.repositories;
                        }
                        let previous = self.pull_request.replace(snapshot.pull_request);
                        self.invalidate_pull_request_content_rows();
                        let current = self.pull_request.as_ref().expect("just assigned");
                        let same = previous.as_ref().is_some_and(|previous| {
                            previous.number == current.number
                                && previous
                                    .base_repository
                                    .url
                                    .eq_ignore_ascii_case(&current.base_repository.url)
                        });
                        let head_moved =
                            previous.is_some_and(|previous| previous.head_oid != current.head_oid);
                        self.pull_request_repository = snapshot.selected_repository;
                        self.pull_request_warnings = snapshot.warnings;
                        self.pull_request_exact_number = snapshot.exact_number;
                        self.pull_request_from_cache = snapshot.from_cache;
                        if !same {
                            self.reset_pull_request_runtime();
                        } else if head_moved {
                            self.reset_pull_request_diff_runtime();
                        }
                        self.pull_request_progress = None;
                        self.pull_request_error = None;
                        self.schedule_pull_request_poll(now);
                        self.request_pull_request_checks(true, &mut effects);
                        self.request_pull_request_conversation(true, &mut effects);
                        if !same || head_moved {
                            self.preview_due = None;
                            self.request_preview(&mut effects);
                        }
                    }
                    Err(error) => {
                        self.pull_request_progress = None;
                        self.pull_request_error = Some(error.clone());
                        self.document = DiffDocument::empty("Pull Requests", error.clone());
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::PullRequestProgress {
                generation,
                diff,
                progress,
            } => {
                let current = if diff {
                    generation == self.diff_generation
                } else {
                    generation == self.pull_request_generation
                };
                if current {
                    self.pull_request_progress = Some(progress);
                }
            }
            WorkerEvent::Branches { generation, result } => {
                if generation != self.branch_generation {
                    return effects;
                }
                match result {
                    Ok(items) => {
                        if let Some(Modal::Branches {
                            items: modal_items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            *modal_items = items;
                            *loading = false;
                            *selected = 0;
                        }
                    }
                    Err(error) => {
                        self.modal = None;
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::HistoryBranches { generation, result } => {
                if generation != self.history_branch_generation {
                    return effects;
                }
                self.history_branches_loading = false;
                match result {
                    Ok(items) => {
                        self.history_branches_loaded = true;
                        self.history_branches = items;
                        match self.modal.as_mut() {
                            Some(Modal::HistoryBranches {
                                items: modal_items,
                                selected,
                                loading,
                                ..
                            }) => {
                                *selected = self
                                    .history_branches
                                    .iter()
                                    .position(|branch| {
                                        self.history_branch
                                            .as_ref()
                                            .map_or(branch.current, |selected| {
                                                selected.reference == branch.reference
                                            })
                                    })
                                    .unwrap_or_default();
                                modal_items.clone_from(&self.history_branches);
                                *loading = false;
                            }
                            Some(Modal::CompareBranches {
                                items: modal_items,
                                selected,
                                loading,
                                ..
                            }) => {
                                modal_items.clone_from(&self.history_branches);
                                *selected = 0;
                                *loading = false;
                            }
                            _ => {}
                        }
                    }
                    Err(error) => {
                        if matches!(
                            self.modal,
                            Some(Modal::HistoryBranches { .. } | Modal::CompareBranches { .. })
                        ) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::Stashes { generation, result } => {
                if generation != self.stash_generation {
                    return effects;
                }
                match result {
                    Ok(items) => {
                        if let Some(Modal::Stashes {
                            items: modal_items,
                            selected,
                            loading,
                            ..
                        }) = self.modal.as_mut()
                        {
                            *modal_items = items;
                            *selected = 0;
                            *loading = false;
                        }
                    }
                    Err(error) => {
                        if matches!(self.modal, Some(Modal::Stashes { .. })) {
                            self.modal = None;
                        }
                        self.show_toast(error, ToastLevel::Error, now);
                    }
                }
            }
            WorkerEvent::OperationFinished {
                id,
                label,
                changes_history,
                result,
            } => {
                if id != self.operation_id {
                    return effects;
                }
                self.busy = None;
                match result {
                    Ok(message) => {
                        self.show_toast(message, ToastLevel::Success, now);
                        self.request_refresh(&mut effects);
                        if changes_history {
                            self.request_history(true, &mut effects);
                        }
                    }
                    Err(error) => {
                        self.show_toast(format!("{label}: {error}"), ToastLevel::Error, now);
                        self.request_refresh(&mut effects);
                    }
                }
            }
        }
        effects
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the draw pass reads better as one top-to-bottom pass"
    )]
    fn handle_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match &mut modal {
            Modal::Help { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('?' | 'q') => {}
                KeyCode::Up | KeyCode::Char('k') => {
                    *scroll = scroll.saturating_sub(1);
                    self.modal = Some(modal);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *scroll = scroll.saturating_add(1);
                    self.modal = Some(modal);
                }
                KeyCode::PageUp => {
                    *scroll = scroll.saturating_sub(10);
                    self.modal = Some(modal);
                }
                KeyCode::PageDown => {
                    *scroll = scroll.saturating_add(10);
                    self.modal = Some(modal);
                }
                _ => self.modal = Some(modal),
            },
            Modal::Commit { input, amend } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                if key.code == KeyCode::Enter
                    && key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    let message = input.value.trim().to_owned();
                    self.queue_operation(
                        GitOperation::Commit {
                            message,
                            amend: *amend,
                        },
                        &mut effects,
                    );
                    return effects;
                }
                edit_text(input, key, true);
                self.modal = Some(modal);
            }
            Modal::Prompt { input, kind, .. } => {
                if key.code == KeyCode::Esc {
                    if let PromptKind::Filter { previous } = kind {
                        self.filter.clone_from(previous);
                        self.normalize_selection();
                        self.schedule_preview(now);
                    }
                    return effects;
                }
                if key.code == KeyCode::Enter {
                    match kind {
                        PromptKind::Filter { .. } => {
                            self.filter.clone_from(&input.value);
                            self.normalize_selection();
                            self.schedule_preview(now);
                        }
                        PromptKind::CreateBranch { start } => {
                            self.queue_operation(
                                GitOperation::CreateBranch {
                                    name: input.value.trim().to_owned(),
                                    start: start.clone(),
                                },
                                &mut effects,
                            );
                        }
                        PromptKind::RenameBranch { old } => {
                            self.queue_operation(
                                GitOperation::RenameBranch {
                                    old: old.clone(),
                                    new: input.value.trim().to_owned(),
                                },
                                &mut effects,
                            );
                        }
                        PromptKind::StashPush {
                            include_untracked,
                            staged,
                        } => {
                            self.queue_operation(
                                GitOperation::StashPush {
                                    message: input.value.trim().to_owned(),
                                    include_untracked: *include_untracked,
                                    staged: *staged,
                                },
                                &mut effects,
                            );
                        }
                    }
                    return effects;
                }
                edit_text(input, key, false);
                if matches!(kind, PromptKind::Filter { .. }) {
                    self.filter.clone_from(&input.value);
                    self.normalize_selection();
                    self.schedule_preview(now);
                }
                self.modal = Some(modal);
            }
            Modal::Confirm { operation, .. } => match key.code {
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    self.queue_operation(operation.clone(), &mut effects);
                }
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {}
                _ => self.modal = Some(modal),
            },
            Modal::Conflict { change } => match key.code {
                KeyCode::Char('o') => self.queue_operation(
                    GitOperation::ResolveConflict {
                        path: change.path.clone(),
                        choice: ConflictChoice::Ours,
                    },
                    &mut effects,
                ),
                KeyCode::Char('t') => self.queue_operation(
                    GitOperation::ResolveConflict {
                        path: change.path.clone(),
                        choice: ConflictChoice::Theirs,
                    },
                    &mut effects,
                ),
                KeyCode::Char('s') | KeyCode::Enter => self
                    .queue_operation(GitOperation::Stage(vec![change.path.clone()]), &mut effects),
                KeyCode::Esc => {}
                _ => self.modal = Some(modal),
            },
            Modal::Branches {
                items,
                selected,
                query,
                loading,
                ..
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_branches(items, &query.value);
                match key.code {
                    KeyCode::Up => *selected = previous_list_index(*selected, visible.len()),
                    KeyCode::Down => *selected = next_list_index(*selected, visible.len()),
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            if branch.current {
                                self.show_toast(
                                    format!("Already on {}", branch.name),
                                    ToastLevel::Info,
                                    now,
                                );
                            } else {
                                self.queue_operation(
                                    GitOperation::Checkout(branch.name.clone()),
                                    &mut effects,
                                );
                            }
                        }
                        return effects;
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.modal = Some(Modal::Prompt {
                            title: "Create Branch".to_owned(),
                            input: TextBuffer::default(),
                            kind: PromptKind::CreateBranch { start: None },
                        });
                        return effects;
                    }
                    KeyCode::F(2) | KeyCode::Char('r')
                        if !*loading
                            && (key.code == KeyCode::F(2)
                                || key.modifiers.contains(KeyModifiers::CONTROL)) =>
                    {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            self.modal = Some(Modal::Prompt {
                                title: "Rename Local Branch".to_owned(),
                                input: TextBuffer::new(branch.name.clone()),
                                kind: PromptKind::RenameBranch {
                                    old: branch.name.clone(),
                                },
                            });
                        }
                        return effects;
                    }
                    KeyCode::Delete if !*loading => {
                        if let Some(branch) =
                            visible.get(*selected).and_then(|index| items.get(*index))
                        {
                            if branch.current {
                                self.show_toast(
                                    "Cannot delete the current branch".to_owned(),
                                    ToastLevel::Error,
                                    now,
                                );
                            } else {
                                self.modal = Some(Modal::Confirm {
                                    title: "Delete Branch?".to_owned(),
                                    message: format!(
                                        "Delete local branch `{}`? Git will refuse if it is not merged.",
                                        branch.name
                                    ),
                                    operation: GitOperation::DeleteBranch(branch.name.clone()),
                                });
                            }
                            return effects;
                        }
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::HistoryBranches {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_history_branches(items, &query.value);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.select_history_branch(branch, &mut effects);
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::CompareBranches {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_history_branches(items, &query.value)
                    .into_iter()
                    .filter(|index| items.get(*index).is_some_and(|item| !item.current))
                    .collect::<Vec<_>>();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(branch) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.auxiliary_preview = Some(AuxiliaryPreview::Branch(branch));
                            self.focus = Focus::Content;
                            self.content_scroll = 0;
                            self.request_preview(&mut effects);
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::Stashes {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_stashes(items, &query.value);
                let selected_stash = visible
                    .get(*selected)
                    .and_then(|index| items.get(*index))
                    .cloned();
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(stash) = selected_stash {
                            self.auxiliary_preview = Some(AuxiliaryPreview::Stash(stash));
                            self.focus = Focus::Content;
                            self.content_scroll = 0;
                            self.request_preview(&mut effects);
                        }
                        return effects;
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(false, false);
                        return effects;
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(true, false);
                        return effects;
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.prompt_stash(false, true);
                        return effects;
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if let Some(stash) = selected_stash {
                            self.queue_operation(
                                GitOperation::StashApply(stash.reference),
                                &mut effects,
                            );
                        }
                        return effects;
                    }
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                        if let Some(stash) = selected_stash {
                            self.queue_operation(
                                GitOperation::StashPop(Some(stash.reference)),
                                &mut effects,
                            );
                        }
                        return effects;
                    }
                    KeyCode::Delete if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !items.is_empty() {
                            self.modal = Some(Modal::Confirm {
                                title: "Drop All Stashes?".to_owned(),
                                message: "Permanently delete every stash? This cannot be undone."
                                    .to_owned(),
                                operation: GitOperation::StashClear,
                            });
                        }
                        return effects;
                    }
                    KeyCode::Delete if !*loading => {
                        if let Some(stash) = selected_stash {
                            self.modal = Some(Modal::Confirm {
                                title: "Drop Stash?".to_owned(),
                                message: format!(
                                    "Permanently delete {} — {}?",
                                    stash.reference, stash.message
                                ),
                                operation: GitOperation::StashDrop(stash.reference),
                            });
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::PullRequestRepositories {
                items,
                selected,
                query,
                loading,
            } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let visible = Self::filtered_github_repositories(items, &query.value);
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, visible.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, visible.len());
                    }
                    KeyCode::Enter if !*loading => {
                        if let Some(repository) = visible
                            .get(*selected)
                            .and_then(|index| items.get(*index))
                            .cloned()
                        {
                            self.pull_request_repository = Some(repository);
                            if let Some(number) = self.pull_request_exact_number.or_else(|| {
                                self.pull_request_lookup.value.trim().parse::<u64>().ok()
                            }) {
                                self.request_pull_request_lookup(
                                    number,
                                    false,
                                    false,
                                    &mut effects,
                                );
                            }
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::CommandPalette { query, selected } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let commands = self.palette_commands(&query.value);
                match key.code {
                    KeyCode::Up => *selected = previous_list_index(*selected, commands.len()),
                    KeyCode::Down => *selected = next_list_index(*selected, commands.len()),
                    KeyCode::Enter => {
                        if let Some(command) = commands.get(*selected).copied() {
                            self.execute_palette(command, &mut effects, now);
                        }
                        return effects;
                    }
                    _ => {
                        edit_text(query, key, false);
                        *selected = 0;
                    }
                }
                self.modal = Some(modal);
            }
            Modal::Themes { selected, original } => {
                match key.code {
                    KeyCode::Esc => {
                        self.apply_theme(*original);
                        return effects;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, ThemeName::ALL.len());
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, ThemeName::ALL.len());
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(name) = ThemeName::ALL.get(*selected).copied() {
                            self.apply_theme(name);
                            self.show_toast(
                                format!("Theme changed to {}", name.label()),
                                ToastLevel::Success,
                                now,
                            );
                        }
                        return effects;
                    }
                    _ => {}
                }
                self.modal = Some(modal);
            }
            Modal::Appearances { selected } => {
                match key.code {
                    KeyCode::Esc => return effects,
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = previous_list_index(*selected, AppearanceChoice::ALL.len());
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        *selected = next_list_index(*selected, AppearanceChoice::ALL.len());
                    }
                    KeyCode::Enter => {
                        if let Some(choice) = AppearanceChoice::ALL.get(*selected).copied() {
                            self.set_theme_selection(self.theme_name, choice);
                            self.show_toast(
                                format!("Appearance changed to {}", choice.label()),
                                ToastLevel::Success,
                                now,
                            );
                        }
                        return effects;
                    }
                    _ => {}
                }
                self.modal = Some(modal);
            }
        }
        effects
    }

    fn execute_palette(
        &mut self,
        command: PaletteCommand,
        effects: &mut Vec<AppEffect>,
        now: Instant,
    ) {
        match command {
            PaletteCommand::Refresh => self.request_active_refresh(effects),
            PaletteCommand::StageAll => self.queue_operation(GitOperation::StageAll, effects),
            PaletteCommand::UnstageAll => {
                self.queue_operation(GitOperation::UnstageAll, effects);
            }
            PaletteCommand::Commit | PaletteCommand::Amend => {
                self.modal = Some(Modal::Commit {
                    input: TextBuffer::default(),
                    amend: command == PaletteCommand::Amend,
                });
            }
            PaletteCommand::Fetch => self.queue_operation(GitOperation::Fetch, effects),
            PaletteCommand::Pull => self.queue_operation(GitOperation::Pull, effects),
            PaletteCommand::Push => self.queue_operation(GitOperation::Push, effects),
            PaletteCommand::Sync => self.queue_operation(GitOperation::Sync, effects),
            PaletteCommand::Stash => self.prompt_stash(false, false),
            PaletteCommand::StashStaged => self.prompt_stash(false, true),
            PaletteCommand::StashIncludeUntracked => self.prompt_stash(true, false),
            PaletteCommand::StashPop => self.queue_operation(GitOperation::StashPop(None), effects),
            PaletteCommand::ManageStashes => self.open_stashes(effects),
            PaletteCommand::Branches => self.open_branches(effects),
            PaletteCommand::CompareBranch => self.open_compare_branches(effects),
            PaletteCommand::RenameCurrentBranch => {
                if self.status.branch.detached || self.status.branch.head.is_empty() {
                    self.show_toast(
                        "Cannot rename a detached or unnamed branch".to_owned(),
                        ToastLevel::Error,
                        now,
                    );
                } else {
                    let old = self.status.branch.head.clone();
                    self.modal = Some(Modal::Prompt {
                        title: "Rename Current Local Branch".to_owned(),
                        input: TextBuffer::new(old.clone()),
                        kind: PromptKind::RenameBranch { old },
                    });
                }
            }
            PaletteCommand::ToggleDiffLayout => self.toggle_diff_layout(),
            PaletteCommand::ToggleAllFiles => self.toggle_all_preview_files(effects),
            PaletteCommand::ShowChanges => self.switch_view(View::Changes, effects),
            PaletteCommand::ShowHistory => self.switch_view(View::History, effects),
            PaletteCommand::ShowPullRequests => self.switch_view(View::PullRequests, effects),
            PaletteCommand::ChangeTheme => {
                self.modal = Some(Modal::Themes {
                    selected: ThemeName::ALL
                        .iter()
                        .position(|name| *name == self.theme_name)
                        .unwrap_or_default(),
                    original: self.theme_name,
                });
            }
            PaletteCommand::ChangeAppearance => {
                self.modal = Some(Modal::Appearances {
                    selected: AppearanceChoice::ALL
                        .iter()
                        .position(|choice| *choice == self.appearance_choice)
                        .unwrap_or_default(),
                });
            }
            PaletteCommand::Help => self.modal = Some(Modal::Help { scroll: 0 }),
            PaletteCommand::Quit => effects.push(AppEffect::Quit),
        }
    }

    fn apply_theme(&mut self, name: ThemeName) {
        self.theme_name = name;
        self.theme = Theme::new(name, self.appearance);
    }

    fn apply_live_modal_filter(&mut self) {
        if let Some(Modal::Prompt {
            input,
            kind: PromptKind::Filter { .. },
            ..
        }) = self.modal.as_ref()
        {
            self.filter.clone_from(&input.value);
            self.normalize_selection();
            self.preview_due = Some(Instant::now() + PREVIEW_DEBOUNCE);
        }
    }

    fn begin_resize(&mut self, target: ResizeTarget, column: u16, now: Instant) {
        let double_tap = self.last_resize_tap.is_some_and(|(previous, tapped)| {
            previous == target
                && now.saturating_duration_since(tapped) <= RESIZE_DOUBLE_TAP_INTERVAL
        });
        if double_tap {
            match target {
                ResizeTarget::Sidebar => {
                    let maximum = self
                        .geometry
                        .main
                        .width
                        .saturating_sub(MIN_CONTENT_WIDTH)
                        .max(MIN_SIDEBAR_WIDTH);
                    self.sidebar_width = DEFAULT_SIDEBAR_WIDTH.clamp(MIN_SIDEBAR_WIDTH, maximum);
                }
                ResizeTarget::Diff => self.diff_split_percent = DEFAULT_DIFF_SPLIT_PERCENT,
            }
            self.resize_target = None;
            self.last_resize_tap = None;
            return;
        }

        self.last_resize_tap = Some((target, now));
        self.resize_target = Some(target);
        match target {
            ResizeTarget::Sidebar => self.resize_sidebar(column),
            ResizeTarget::Diff => self.resize_diff(column),
        }
    }

    fn resize_sidebar(&mut self, column: u16) {
        let main = self.geometry.main;
        let maximum = main
            .width
            .saturating_sub(MIN_CONTENT_WIDTH)
            .max(MIN_SIDEBAR_WIDTH);
        self.sidebar_width = column
            .saturating_sub(main.x)
            .clamp(MIN_SIDEBAR_WIDTH, maximum);
    }

    #[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
    fn resize_diff(&mut self, column: u16) {
        let content = self.geometry.content;
        if content.width == 0 {
            return;
        }
        let relative = column.saturating_sub(content.x).min(content.width);
        self.diff_split_percent = (relative.saturating_mul(100) / content.width)
            .clamp(MIN_DIFF_SPLIT_PERCENT, MAX_DIFF_SPLIT_PERCENT);
    }

    fn switch_view(&mut self, view: View, _effects: &mut Vec<AppEffect>) {
        self.pull_request_lookup_active = false;
        if self.view == view {
            if view == View::PullRequests && self.pull_request.is_none() {
                self.pull_request_lookup_active = true;
            }
            return;
        }
        self.view = view;
        self.auxiliary_preview = None;
        self.reset_local_diff_runtime();
        self.selected_preview_file = None;
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        self.focus = Focus::Sidebar;
        self.sidebar_offset = 0;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.invalidate_preview();
        self.document = self.loading_document_for_view(view);
        self.schedule_pull_request_poll(Instant::now());
        if view == View::PullRequests && self.pull_request.is_none() {
            self.pull_request_lookup_active = true;
        } else {
            self.preview_due = Some(Instant::now() + PREVIEW_DEBOUNCE);
        }
    }

    fn loading_document_for_view(&self, view: View) -> DiffDocument {
        match view {
            View::Changes => DiffDocument::empty("Working Tree", "Loading selected changes…"),
            View::History => {
                let title = self.selected_commit().map_or_else(
                    || "Commit History".to_owned(),
                    |commit| format!("{} — {}", commit.short_id, commit.subject),
                );
                let message = if self.history_loading && self.history.is_empty() {
                    "Loading commit history…"
                } else if self.history.is_empty() {
                    "No commits in this repository"
                } else {
                    "Loading commit preview…"
                };
                DiffDocument::empty(title, message)
            }
            View::PullRequests => self.selected_pull_request().map_or_else(
                || {
                    DiffDocument::empty(
                        "Open Pull Request",
                        "Enter a pull-request number and press Enter",
                    )
                },
                |pull_request| {
                    pull_request_loading_document(
                        pull_request,
                        self.pull_request_progress
                            .map_or("Calculating pull-request diff…", PullRequestProgress::label),
                    )
                },
            ),
        }
    }

    const fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Content,
            Focus::Content => Focus::Sidebar,
        };
    }

    const fn toggle_sidebar(&mut self) {
        self.sidebar_hidden = !self.sidebar_hidden;
        self.focus = if self.sidebar_hidden {
            Focus::Content
        } else {
            Focus::Sidebar
        };
        self.resize_target = None;
        if self.sidebar_hidden {
            self.pull_request_lookup_active = false;
        }
    }

    const fn toggle_diff_layout(&mut self) {
        self.diff_layout = match self.diff_layout {
            DiffLayout::Unified => DiffLayout::SideBySide,
            DiffLayout::SideBySide => DiffLayout::Unified,
        };
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
    }

    fn navigate(&mut self, amount: isize, now: Instant) {
        if self.focus == Focus::Content
            && self.check_log_visible()
            && self.move_check_step_cursor(amount)
        {
            return;
        }
        if self.focus == Focus::Content {
            if self.preview_files_all_collapsed() {
                self.navigate_preview_file(amount);
            } else if amount < 0 {
                self.content_scroll = self.content_scroll.saturating_sub(amount.unsigned_abs());
            } else {
                self.content_scroll = self.content_scroll.saturating_add(count(amount));
            }
            return;
        }

        let preserve_auxiliary_preview = self.auxiliary_preview.is_some();
        match self.view {
            View::Changes => {
                let targets = self.change_targets();
                if targets.is_empty() {
                    self.selected_change_section = Some(ChangeSection::Unstaged);
                    self.change_cursor = 0;
                    return;
                }
                let current = self
                    .selected_change_target()
                    .and_then(|target| targets.iter().position(|candidate| *candidate == target))
                    .unwrap_or_default();
                let next = if amount < 0 {
                    current.saturating_sub(amount.unsigned_abs())
                } else {
                    (current + count(amount)).min(targets.len() - 1)
                };
                if let Some(target) = targets.get(next).copied() {
                    self.select_change_target(target);
                }
            }
            View::History => {
                let length = self.visible_commit_indices().len();
                if length == 0 {
                    self.history_cursor = 0;
                    return;
                }
                self.history_cursor = if amount < 0 {
                    self.history_cursor.saturating_sub(amount.unsigned_abs())
                } else {
                    (self.history_cursor + count(amount)).min(length - 1)
                };
            }
            View::PullRequests => {
                match self.pull_request_section {
                    PullRequestSection::Files => {
                        let length = self.pull_request_tree_entries().len();
                        if length == 0 {
                            self.pull_request_file_cursor = 0;
                            self.pull_request_tree_cursor = 0;
                            return;
                        }
                        let cursor = if amount < 0 {
                            self.pull_request_tree_cursor
                                .saturating_sub(amount.unsigned_abs())
                        } else {
                            (self.pull_request_tree_cursor + count(amount)).min(length - 1)
                        };
                        self.select_pull_request_tree_entry(cursor, now);
                    }
                    PullRequestSection::Overview => {
                        self.move_check_cursor(amount);
                        self.schedule_preview(now);
                    }
                }
                return;
            }
        }
        if !preserve_auxiliary_preview {
            self.schedule_preview(now);
        }
    }

    fn page(&mut self, direction: isize, now: Instant) {
        if self.focus == Focus::Content {
            let amount = self.geometry.content.height.saturating_sub(4).max(1) as usize;
            if direction < 0 {
                self.content_scroll = self.content_scroll.saturating_sub(amount);
            } else {
                self.content_scroll = self.content_scroll.saturating_add(amount);
            }
        } else {
            let amount = offset(self.geometry.sidebar.height.saturating_sub(4).max(1));
            self.navigate(direction * amount, now);
        }
    }

    fn go_to_edge(&mut self, end: bool, now: Instant) {
        if self.focus == Focus::Content && self.check_log_visible() {
            let steps = self.check_step_numbers();
            if let Some(step) = if end { steps.last() } else { steps.first() } {
                self.reveal_check_step(*step);
                return;
            }
        }
        if self.focus == Focus::Content {
            self.content_scroll = if end { usize::MAX } else { 0 };
            return;
        }
        let preserve_auxiliary_preview = self.auxiliary_preview.is_some();
        match self.view {
            View::Changes => {
                let targets = self.change_targets();
                if let Some(target) = if end { targets.last() } else { targets.first() } {
                    self.select_change_target(*target);
                }
            }
            View::History => {
                let length = self.visible_commit_indices().len();
                self.history_cursor = if end { length.saturating_sub(1) } else { 0 };
            }
            View::PullRequests => {
                match self.pull_request_section {
                    PullRequestSection::Files => {
                        let entries = self.pull_request_tree_entries();
                        let cursor = if end {
                            entries.len().saturating_sub(1)
                        } else {
                            0
                        };
                        self.select_pull_request_tree_entry(cursor, now);
                    }
                    PullRequestSection::Overview => {
                        let cursor = end
                            .then(|| self.pull_request_checks.len().checked_sub(1))
                            .flatten();
                        let _ = self.set_check_cursor(cursor);
                        self.schedule_preview(now);
                    }
                }
                return;
            }
        }
        if !preserve_auxiliary_preview {
            self.schedule_preview(now);
        }
    }

    #[expect(clippy::integer_division, reason = "layout maths works in whole cells")]
    fn scroll_content_half(&mut self, down: bool) {
        let amount = (self.geometry.content.height / 2).max(1) as usize;
        if down {
            self.content_scroll = self.content_scroll.saturating_add(amount);
        } else {
            self.content_scroll = self.content_scroll.saturating_sub(amount);
        }
    }

    fn jump_hunk(&mut self, forward: bool) {
        if forward {
            if let Some((index, _)) = self
                .document
                .lines
                .iter()
                .enumerate()
                .skip(self.content_scroll.saturating_add(1))
                .find(|(_, line)| line.kind == DiffLineKind::HunkHeader)
            {
                self.content_scroll = index;
            }
        } else if let Some((index, _)) = self
            .document
            .lines
            .iter()
            .enumerate()
            .take(self.content_scroll)
            .rev()
            .find(|(_, line)| line.kind == DiffLineKind::HunkHeader)
        {
            self.content_scroll = index;
        }
    }

    #[expect(
        clippy::unreachable,
        reason = "the branch is impossible for the states that reach it"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the caller has no use for the value afterwards"
    )]
    fn handle_scm_action(&mut self, action: ScmAction, effects: &mut Vec<AppEffect>) {
        match action {
            ScmAction::Stage(index) | ScmAction::Unstage(index) | ScmAction::Resolve(index) => {
                let Some(change) = self.status.changes.get(index).cloned() else {
                    return;
                };
                self.auxiliary_preview = None;
                if let Some(cursor) = self
                    .visible_change_indices()
                    .iter()
                    .position(|visible| *visible == index)
                {
                    self.selected_change_section = None;
                    self.change_cursor = cursor;
                }
                match action {
                    ScmAction::Stage(_) => {
                        self.queue_operation(GitOperation::Stage(vec![change.path]), effects);
                    }
                    ScmAction::Unstage(_) => {
                        self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
                    }
                    ScmAction::Resolve(_) => self.modal = Some(Modal::Conflict { change }),
                    _ => unreachable!(),
                }
            }
            ScmAction::StageSection(section) | ScmAction::UnstageSection(section) => {
                let paths = self
                    .status
                    .changes
                    .iter()
                    .filter(|change| section.matches(change))
                    .map(|change| change.path.clone())
                    .collect::<Vec<_>>();
                if paths.is_empty() {
                    return;
                }
                match action {
                    ScmAction::StageSection(_) => {
                        self.queue_operation(GitOperation::Stage(paths), effects);
                    }
                    ScmAction::UnstageSection(_) => {
                        self.queue_operation(GitOperation::Unstage(paths), effects);
                    }
                    _ => unreachable!(),
                }
            }
            ScmAction::StageAll => self.queue_operation(GitOperation::StageAll, effects),
            ScmAction::UnstageAll => self.queue_operation(GitOperation::UnstageAll, effects),
            ScmAction::Commit => {
                self.modal = Some(Modal::Commit {
                    input: TextBuffer::default(),
                    amend: false,
                });
            }
            ScmAction::Stashes => self.open_stashes(effects),
            ScmAction::CompareBranch => self.open_compare_branches(effects),
        }
    }

    fn prompt_stash(&mut self, include_untracked: bool, staged: bool) {
        let title = if staged {
            "Stash Staged Changes"
        } else if include_untracked {
            "Stash Changes Including Untracked"
        } else {
            "Stash Changes"
        };
        self.modal = Some(Modal::Prompt {
            title: title.to_owned(),
            input: TextBuffer::default(),
            kind: PromptKind::StashPush {
                include_untracked,
                staged,
            },
        });
    }

    fn open_stashes(&mut self, effects: &mut Vec<AppEffect>) {
        self.stash_generation = self.stash_generation.wrapping_add(1);
        self.modal = Some(Modal::Stashes {
            items: Vec::new(),
            selected: 0,
            query: TextBuffer::default(),
            loading: true,
        });
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadStashes {
            generation: self.stash_generation,
        })));
    }

    fn open_compare_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.modal = Some(Modal::CompareBranches {
            items: self.history_branches.clone(),
            selected: 0,
            query: TextBuffer::default(),
            loading: self.history_branches_loading,
        });
        if !self.history_branches_loaded && !self.history_branches_loading {
            self.request_history_branches(effects);
        }
    }

    fn toggle_stage_selected(&mut self, effects: &mut Vec<AppEffect>) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        match change.area {
            ChangeArea::Unstaged => {
                self.queue_operation(GitOperation::Stage(vec![change.path]), effects);
            }
            ChangeArea::Conflict => self.modal = Some(Modal::Conflict { change }),
            ChangeArea::Staged => {
                self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
            }
        }
    }

    fn unstage_selected(&mut self, effects: &mut Vec<AppEffect>) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Staged {
            self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
        }
    }

    fn confirm_discard(&mut self) {
        if self.selected_change_section.is_some() {
            return;
        }
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Conflict {
            self.modal = Some(Modal::Conflict { change });
            return;
        }
        self.modal = Some(Modal::Confirm {
            title: "Discard Change?".to_owned(),
            message: format!(
                "Permanently discard changes to `{}`? This cannot be undone.",
                change.display_path()
            ),
            operation: GitOperation::Discard(vec![change]),
        });
    }

    fn confirm_cherry_pick(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: "Cherry-pick Commit?".to_owned(),
            message: format!(
                "Apply {} — {} to the current branch?",
                commit.short_id, commit.subject
            ),
            operation: GitOperation::CherryPick(commit.id.clone()),
        });
    }

    fn confirm_revert(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Confirm {
            title: "Revert Commit?".to_owned(),
            message: format!(
                "Create a commit that reverts {} — {}?",
                commit.short_id, commit.subject
            ),
            operation: GitOperation::Revert(commit.id.clone()),
        });
    }

    fn prompt_branch_at_commit(&mut self) {
        let Some(commit) = self.selected_commit() else {
            return;
        };
        self.modal = Some(Modal::Prompt {
            title: format!("Create Branch at {}", commit.short_id),
            input: TextBuffer::default(),
            kind: PromptKind::CreateBranch {
                start: Some(commit.id.clone()),
            },
        });
    }

    fn open_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.branch_generation = self.branch_generation.wrapping_add(1);
        let generation = self.branch_generation;
        self.modal = Some(Modal::Branches {
            items: Vec::new(),
            selected: 0,
            query: TextBuffer::default(),
            loading: true,
        });
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadBranches {
            generation,
        })));
    }

    fn open_history_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.modal = Some(Modal::HistoryBranches {
            items: self.history_branches.clone(),
            selected: self
                .history_branches
                .iter()
                .position(|branch| {
                    self.history_branch
                        .as_ref()
                        .map_or(branch.current, |selected| {
                            selected.reference == branch.reference
                        })
                })
                .unwrap_or_default(),
            query: TextBuffer::default(),
            loading: self.history_branches_loading,
        });
        if !self.history_branches_loaded && !self.history_branches_loading {
            self.request_history_branches(effects);
        }
    }

    fn request_history_branches(&mut self, effects: &mut Vec<AppEffect>) {
        self.history_branch_generation = self.history_branch_generation.wrapping_add(1);
        self.history_branches_loading = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadHistoryBranches {
                generation: self.history_branch_generation,
            },
        )));
    }

    fn select_history_branch(&mut self, branch: HistoryBranch, effects: &mut Vec<AppEffect>) {
        self.history_branch = (!branch.current).then_some(branch);
        self.history.clear();
        self.history_cursor = 0;
        self.sidebar_offset = 0;
        self.history_complete = false;
        self.history_refresh_again = false;
        self.history_generation = self.history_generation.wrapping_add(1);
        self.history_loading = false;
        self.request_history(true, effects);
    }

    fn open_pull_request_repositories(&mut self, effects: &mut Vec<AppEffect>) {
        let selected = self
            .pull_request_repository
            .as_ref()
            .and_then(|selected| {
                self.github_repositories
                    .iter()
                    .position(|repository| repository.url == selected.url)
            })
            .unwrap_or_default();
        let loading = self.github_repositories.is_empty();
        self.modal = Some(Modal::PullRequestRepositories {
            items: self.github_repositories.clone(),
            selected,
            query: TextBuffer::default(),
            loading,
        });
        if loading {
            self.repository_generation = self.repository_generation.wrapping_add(1);
            effects.push(AppEffect::Git(Box::new(
                WorkerCommand::LoadGitHubRepositories {
                    generation: self.repository_generation,
                    refresh: false,
                },
            )));
        }
    }

    fn handle_pull_request_lookup_key(&mut self, key: KeyEvent, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match key.code {
            KeyCode::Char('z') if key.modifiers == KeyModifiers::NONE => self.toggle_sidebar(),
            KeyCode::Char('m') if key.modifiers == KeyModifiers::NONE => {
                effects.push(self.toggle_mouse_capture(now));
            }
            KeyCode::Esc => self.pull_request_lookup_active = false,
            KeyCode::Char('o') => {
                self.pull_request_lookup_active = false;
                self.open_pull_request_repositories(&mut effects);
            }
            KeyCode::Enter => {
                let value = self.pull_request_lookup.value.trim();
                match value.parse::<u64>() {
                    Ok(number) if number > 0 => {
                        self.pull_request_lookup_active = false;
                        self.request_pull_request_lookup(number, false, false, &mut effects);
                    }
                    _ => self.show_toast(
                        "Enter a positive numeric pull-request number".to_owned(),
                        ToastLevel::Error,
                        now,
                    ),
                }
            }
            KeyCode::Char(character)
                if character.is_ascii_digit()
                    && self.pull_request_lookup.value.len() < MAX_PULL_REQUEST_NUMBER_DIGITS
                    && !key.modifiers.intersects(
                        KeyModifiers::CONTROL
                            | KeyModifiers::ALT
                            | KeyModifiers::SUPER
                            | KeyModifiers::META
                            | KeyModifiers::HYPER,
                    ) =>
            {
                self.pull_request_lookup.insert(character);
            }
            KeyCode::Backspace => self.pull_request_lookup.backspace(),
            KeyCode::Delete => self.pull_request_lookup.delete(),
            KeyCode::Left => self.pull_request_lookup.move_left(),
            KeyCode::Right => self.pull_request_lookup.move_right(),
            KeyCode::Home => self.pull_request_lookup.home(),
            KeyCode::End => self.pull_request_lookup.end(),
            _ => {}
        }
        effects
    }

    fn reset_local_diff_runtime(&mut self) {
        self.local_diff_request = None;
        self.local_diff_workspace_generation = None;
        self.local_diff_index = None;
        self.local_diff_documents.clear();
        self.local_diff_loading_path = None;
        self.local_diff_pending_paths.clear();
        self.local_diff_single_loaded = false;
    }

    fn rebuild_indexed_preview_document(&mut self) {
        if self.view == View::PullRequests
            && self.pull_request_file_view == PullRequestFileView::AllFiles
        {
            self.rebuild_pull_request_all_files_document();
        } else if self.local_diff_index.is_some() {
            self.rebuild_local_diff_document();
        }
    }

    fn rebuild_local_diff_document(&mut self) {
        if let Some(index) = &self.local_diff_index {
            let visible = if index.files.len() <= 1 {
                index
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<HashSet<_>>()
            } else if self.files_collapsed {
                self.expanded_preview_files.clone()
            } else {
                index
                    .files
                    .iter()
                    .filter(|file| !self.collapsed_preview_files.contains(&file.path))
                    .map(|file| file.path.clone())
                    .collect()
            };
            self.document = index.document_with_visibility(&self.local_diff_documents, |path| {
                visible.contains(path)
            });
            if index.files.is_empty()
                && matches!(
                    self.local_diff_request.as_ref(),
                    Some(LocalDiffRequest::Changes { .. })
                )
            {
                self.document = DiffDocument::empty(
                    &index.title,
                    if self.status.changes.is_empty() {
                        "Working tree clean — no changes"
                    } else {
                        "No changes match the current filter"
                    },
                );
            }
        }
    }

    fn request_local_diff_file(&mut self, path: PathBuf, effects: &mut Vec<AppEffect>) {
        let Some(workspace_generation) = self.local_diff_workspace_generation else {
            return;
        };
        let indexed = self
            .local_diff_index
            .as_ref()
            .is_some_and(|index| index.files.iter().any(|file| file.path == path));
        if !indexed
            || self.local_diff_documents.contains_key(&path)
            || self.local_diff_loading_path.as_ref() == Some(&path)
            || self.local_diff_pending_paths.contains(&path)
        {
            return;
        }
        if self.local_diff_loading_path.is_some() {
            self.local_diff_pending_paths.push_back(path);
            return;
        }
        self.local_diff_loading_path = Some(path.clone());
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadLocalDiffFile {
            generation: self.diff_generation,
            workspace_generation,
            path,
        })));
    }

    fn request_next_local_diff_file(&mut self, effects: &mut Vec<AppEffect>) {
        while let Some(path) = self.local_diff_pending_paths.pop_front() {
            if !self.preview_file_collapsed(&path.to_string_lossy())
                && !self.local_diff_documents.contains_key(&path)
            {
                self.request_local_diff_file(path, effects);
                return;
            }
        }
    }

    /// Drop only the prepared diff. The section, cursors, checks and
    /// conversation stay exactly where the reader left them.
    fn reset_pull_request_diff_runtime(&mut self) {
        self.pull_request_workspace_generation = None;
        self.pull_request_documents.clear();
        self.pull_request_document_order.clear();
        self.pull_request_document_bytes = 0;
        self.pull_request_prefetched_paths.clear();
        self.pull_request_loading_path = None;
        self.pull_request_single_file = None;
        self.pull_request_prefetching = false;
        self.pull_request_prefetch_retrying = false;
    }

    fn reset_pull_request_runtime(&mut self) {
        self.reset_pull_request_diff_runtime();
        self.pull_request_section = PullRequestSection::Overview;
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_files.clear();
        self.pull_request_tree.clear();
        self.pull_request_total_files = 0;
        self.pull_request_files_truncated = false;
        self.pull_request_file_cursor = 0;
        self.pull_request_tree_cursor = 0;
        self.collapsed_pull_request_directories.clear();
        self.pull_request_checks.clear();
        self.pull_request_check_cursor = None;
        self.pull_request_checks_loading = false;
        self.pull_request_checks_error = None;
        self.pull_request_checks_generation = self.pull_request_checks_generation.wrapping_add(1);
        self.pull_request_prefetched_logs.clear();
        self.pull_request_conversation = PullRequestConversation::default();
        self.pull_request_conversation_loading = false;
        self.pull_request_conversation_refresh_again = false;
        self.pull_request_conversation_error = None;
        self.pull_request_conversation_generation =
            self.pull_request_conversation_generation.wrapping_add(1);
        self.pull_request_check_log = None;
        self.pull_request_check_log_loading = false;
        self.pull_request_check_log_error = None;
        self.pull_request_check_log_target = None;
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
        self.expanded_check_steps.clear();
        self.pull_request_step_cursor = 0;
        self.pull_request_content_rows.clear();
        self.pull_request_content_rows_key = None;
        self.pull_request_content_generation = self.pull_request_content_generation.wrapping_add(1);
        self.pull_request_checks_read_at = None;
        self.pull_request_detail_read_at = None;
        self.pull_request_log_read_at = None;
        self.sidebar_offset = 0;
    }

    fn apply_pull_request_index(&mut self, index: PullRequestDiffIndex) {
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_documents.clear();
        self.pull_request_loading_path = None;
        self.pull_request_single_file = None;
        self.pull_request_prefetching = false;
        self.pull_request_prefetch_retrying = false;
        self.pull_request_files = index.files;
        self.pull_request_total_files = index.total_files;
        self.pull_request_files_truncated = index.truncated;
        self.pull_request_file_cursor = self
            .pull_request_file_cursor
            .min(self.pull_request_files.len().saturating_sub(1));
        self.sync_pull_request_tree_cursor_to_file();
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        if self.pull_request_files.len() > 1
            && !self.files_collapsed
            && !self.collapse_preference_set
        {
            self.collapsed_preview_files
                .extend(self.pull_request_files.iter().map(|file| file.path.clone()));
        }
        self.selected_preview_file = self
            .selected_pull_request_file()
            .map(|file| file.path.clone());
        self.preview_file_cursor = self.pull_request_file_cursor;
        self.rebuild_pull_request_all_files_document();
    }

    fn rebuild_pull_request_all_files_document(&mut self) {
        let Some(pull_request) = self.pull_request.as_ref() else {
            return;
        };
        let title = format!(
            "PR #{} — All Files · {} changed",
            pull_request.number, self.pull_request_total_files
        );
        if self.pull_request_files.is_empty() {
            let mut document = DiffDocument::empty(
                title,
                if self.pull_request_files_truncated {
                    "The changed-file index was truncated before any paths were read"
                } else {
                    "This pull request has no changed files"
                },
            );
            document.pull_request_details = Some(pull_request_details(pull_request));
            self.document = document;
            return;
        }
        let index = DiffIndex {
            title,
            files: self
                .pull_request_files
                .iter()
                .map(|file| crate::git::diff::DiffFileIndexEntry {
                    path: file.path.clone(),
                    old_path: file.old_path.clone(),
                    status: pull_request_file_status_label(file.status).to_owned(),
                    counts: file.counts,
                })
                .collect(),
            truncated: self.pull_request_files_truncated,
            commit_details: None,
        };
        let visible = if index.files.len() <= 1 {
            index
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect::<HashSet<_>>()
        } else if self.files_collapsed {
            self.expanded_preview_files.clone()
        } else {
            index
                .files
                .iter()
                .filter(|file| !self.collapsed_preview_files.contains(&file.path))
                .map(|file| file.path.clone())
                .collect()
        };
        let mut document = index
            .document_with_visibility(&self.pull_request_documents, |path| visible.contains(path));
        document.pull_request_details = Some(pull_request_details(pull_request));
        self.document = document;
    }

    fn cache_current_pull_request_single_document(&mut self) {
        let Some(path) = self.pull_request_single_file.take() else {
            return;
        };
        if self.pull_request_documents.contains_key(&path) {
            return;
        }
        let document = std::mem::take(&mut self.document);
        self.cache_pull_request_document(path, document);
    }

    fn cache_pull_request_document(&mut self, path: PathBuf, document: DiffDocument) {
        if let Some(previous) = self.pull_request_documents.remove(&path) {
            self.pull_request_document_bytes = self
                .pull_request_document_bytes
                .saturating_sub(diff_document_size(&previous));
            self.pull_request_document_order
                .retain(|candidate| candidate != &path);
        }
        self.pull_request_document_bytes = self
            .pull_request_document_bytes
            .saturating_add(diff_document_size(&document));
        let _ = self.pull_request_prefetched_paths.insert(path.clone());
        self.pull_request_document_order.push_back(path.clone());
        drop(self.pull_request_documents.insert(path, document));
        self.prune_pull_request_documents(MAX_PULL_REQUEST_DOCUMENT_BYTES);
    }

    fn prune_pull_request_documents(&mut self, maximum_bytes: usize) {
        while self.pull_request_document_bytes > maximum_bytes
            && self.pull_request_documents.len() > 1
        {
            let Some(expired) = self.pull_request_document_order.pop_front() else {
                break;
            };
            if let Some(document) = self.pull_request_documents.remove(&expired) {
                self.pull_request_document_bytes = self
                    .pull_request_document_bytes
                    .saturating_sub(diff_document_size(&document));
            }
        }
    }

    fn take_pull_request_document(&mut self, path: &Path) -> Option<DiffDocument> {
        let document = self.pull_request_documents.remove(path)?;
        self.pull_request_document_bytes = self
            .pull_request_document_bytes
            .saturating_sub(diff_document_size(&document));
        self.pull_request_document_order
            .retain(|candidate| candidate != path);
        Some(document)
    }

    fn show_pull_request_all_files(&mut self) {
        self.cache_current_pull_request_single_document();
        self.pull_request_file_view = PullRequestFileView::AllFiles;
        self.pull_request_loading_path = None;
        self.document_loading = false;
        self.selected_preview_file = self
            .selected_pull_request_file()
            .map(|file| file.path.clone());
        self.preview_file_cursor = self.pull_request_file_cursor;
        self.rebuild_pull_request_all_files_document();
    }

    fn select_pull_request_section(
        &mut self,
        section: PullRequestSection,
        effects: &mut Vec<AppEffect>,
    ) {
        if section == PullRequestSection::Files {
            if self.pull_request_section == PullRequestSection::Files
                && self.pull_request_file_view == PullRequestFileView::AllFiles
            {
                return;
            }
            self.invalidate_preview();
            self.pull_request_section = PullRequestSection::Files;
            self.sidebar_offset = 0;
            self.content_scroll = 0;
            self.horizontal_scroll = 0;
            self.show_pull_request_all_files();
            self.request_preview(effects);
            return;
        }
        if self.pull_request_section == section {
            return;
        }
        self.invalidate_preview();
        self.pull_request_section = section;
        self.sidebar_offset = 0;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.request_pull_request_checks(false, effects);
        self.request_pull_request_conversation(false, effects);
        self.request_check_run_log(false, effects);
    }

    fn request_pull_request_diff_file(
        &mut self,
        path: PathBuf,
        show_loading: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if !self.pull_request_files.iter().any(|file| file.path == path) {
            return;
        }
        if self.pull_request_file_view == PullRequestFileView::SingleFile {
            if self.pull_request_documents.contains_key(&path) {
                self.cache_current_pull_request_single_document();
            }
            if let Some(document) = self.take_pull_request_document(&path) {
                self.document_loading = false;
                self.document = document;
                self.pull_request_single_file = Some(path);
                self.selected_preview_file = None;
                self.preview_file_cursor = 0;
                return;
            }
        } else if self.pull_request_documents.contains_key(&path) {
            self.document_loading = false;
            self.rebuild_pull_request_all_files_document();
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.pull_request_loading_path = Some(path.clone());
        self.document_loading = show_loading;
        self.pull_request_progress = None;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFile {
                generation: self.diff_generation,
                workspace_generation,
                path,
            },
        )));
    }

    /// A path still needs its patch unless it is already cached, already in
    /// flight, or currently occupying the single-file document.
    fn pull_request_file_needs_patch(&self, path: &Path) -> bool {
        !self.pull_request_documents.contains_key(path)
            && self.pull_request_loading_path.as_deref() != Some(path)
            && self.pull_request_single_file.as_deref() != Some(path)
    }

    /// Walk the index in batches until every file has a patch. Each batch is one
    /// Git invocation and lands as soon as it is parsed, so the diff fills in
    /// progressively instead of a file at a time on demand.
    fn request_pull_request_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pull_request_prefetching {
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        if self.pull_request_prefetched_paths.len() >= MAX_PREFETCHED_PULL_REQUEST_FILES {
            return;
        }
        let remaining = MAX_PREFETCHED_PULL_REQUEST_FILES
            .saturating_sub(self.pull_request_prefetched_paths.len());
        let paths: Vec<PathBuf> = self
            .pull_request_files
            .iter()
            .map(|file| file.path.clone())
            .filter(|path| self.pull_request_file_needs_patch(path))
            .filter(|path| !self.pull_request_prefetched_paths.contains(path))
            .take(PULL_REQUEST_PREFETCH_BATCH.min(remaining))
            .collect();
        if paths.is_empty() {
            return;
        }
        self.pull_request_prefetching = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation,
                paths,
            },
        )));
    }

    /// `refresh` separates a live poll from merely arriving in the section: the
    /// latter reuses what is already loaded rather than spending a request.
    fn request_pull_request_checks(&mut self, refresh: bool, effects: &mut Vec<AppEffect>) {
        if self.pull_request_checks_loading || (!refresh && !self.pull_request_checks.is_empty()) {
            return;
        }
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        self.pull_request_checks_generation = self.pull_request_checks_generation.wrapping_add(1);
        self.pull_request_checks_loading = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestChecks {
                generation: self.pull_request_checks_generation,
                pull_request: Box::new(pull_request),
                refresh,
            },
        )));
    }

    /// The overview sidebar is the pull request itself followed by its checks, so
    /// the cursor walks one row above index zero and stops there.
    fn move_check_cursor(&mut self, amount: isize) {
        if self.pull_request_checks.is_empty() {
            let _ = self.set_check_cursor(None);
            return;
        }
        let last = self.pull_request_checks.len() - 1;
        let row = self
            .pull_request_check_cursor
            .map_or(0, |cursor| cursor.saturating_add(1));
        let next = if amount < 0 {
            row.saturating_sub(amount.unsigned_abs())
        } else {
            row.saturating_add(count(amount)).min(last + 1)
        };
        let _ = self.set_check_cursor(next.checked_sub(1));
    }

    /// Every row in the overview sidebar shows a different document on the right,
    /// so a new selection always starts at the top of it.
    fn set_check_cursor(&mut self, cursor: Option<usize>) -> bool {
        if self.pull_request_check_cursor == cursor {
            return false;
        }
        self.pull_request_check_cursor = cursor;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.invalidate_check_run_log();
        self.invalidate_pull_request_content_rows();
        true
    }

    const fn invalidate_pull_request_content_rows(&mut self) {
        self.pull_request_content_generation = self.pull_request_content_generation.wrapping_add(1);
        self.pull_request_content_rows_key = None;
    }

    fn invalidate_check_run_log(&mut self) {
        self.pull_request_check_log = None;
        self.pull_request_check_log_error = None;
        self.pull_request_check_log_target = None;
        self.pull_request_check_log_loading = false;
        self.pull_request_log_read_at = None;
        self.expanded_check_steps.clear();
        self.pull_request_step_cursor = 0;
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
    }

    pub(crate) fn check_log_visible(&self) -> bool {
        self.view == View::PullRequests
            && self.pull_request_section == PullRequestSection::Overview
            && self.pull_request_check_cursor.is_some()
    }

    fn check_step_numbers(&self) -> Vec<usize> {
        self.pull_request_check_log
            .as_ref()
            .map(|log| log.steps.iter().map(|step| step.number).collect())
            .unwrap_or_default()
    }

    pub(crate) fn check_step_expanded(&self, step: usize) -> bool {
        self.expanded_check_steps.contains(&step)
    }

    fn toggle_check_step(&mut self, step: usize) {
        if !self.expanded_check_steps.remove(&step) {
            let _ = self.expanded_check_steps.insert(step);
        }
        self.reveal_check_step(step);
        self.invalidate_pull_request_content_rows();
    }

    fn toggle_all_check_steps(&mut self) {
        let steps = self.check_step_numbers();
        if self.expanded_check_steps.is_empty() {
            self.expanded_check_steps.extend(steps);
        } else {
            self.expanded_check_steps.clear();
        }
        self.content_scroll = 0;
        self.invalidate_pull_request_content_rows();
    }

    /// Move between steps the way `[` and `]` move between diff hunks, so a long
    /// log can be walked without scrolling through it.
    /// Move the step selection and ask the next draw to bring it into view.
    const fn reveal_check_step(&mut self, step: usize) {
        self.pull_request_step_cursor = step;
        self.pull_request_step_reveal = true;
    }

    fn move_check_step_cursor(&mut self, amount: isize) -> bool {
        let steps = self.check_step_numbers();
        if steps.is_empty() {
            return false;
        }
        let current = steps
            .iter()
            .position(|step| *step == self.pull_request_step_cursor)
            .unwrap_or_default();
        let next = if amount < 0 {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            current.saturating_add(count(amount)).min(steps.len() - 1)
        };
        let Some(step) = steps.get(next).copied() else {
            return false;
        };
        self.reveal_check_step(step);
        true
    }

    fn select_pull_request_check(&mut self, cursor: Option<usize>, effects: &mut Vec<AppEffect>) {
        if self.set_check_cursor(cursor) {
            self.request_check_run_log(false, effects);
        }
    }

    /// Fetch the selected check's steps and log. A selection change starts from a
    /// clean slate; a live refresh of the same run updates in place so the reader
    /// keeps their scroll position while a job is still writing output. A log
    /// already held for the selected run is only re-read when `refresh` asks for
    /// it, so redrawing or re-entering the section costs nothing.
    fn request_check_run_log(&mut self, refresh: bool, effects: &mut Vec<AppEffect>) {
        let (Some(pull_request), Some(check)) = (
            self.pull_request.clone(),
            self.selected_pull_request_check().cloned(),
        ) else {
            self.pull_request_check_log = None;
            self.pull_request_check_log_error = None;
            self.pull_request_check_log_loading = false;
            self.pull_request_check_log_target = None;
            self.pull_request_check_log_generation =
                self.pull_request_check_log_generation.wrapping_add(1);
            return;
        };
        let target = check.identity();
        if self.pull_request_check_log_target.as_ref() == Some(&target) {
            if self.pull_request_check_log_loading {
                return;
            }
            let held = self.pull_request_check_log.is_some()
                || self.pull_request_check_log_error.is_some();
            if held && !refresh {
                return;
            }
        } else {
            self.pull_request_check_log = None;
            self.pull_request_check_log_error = None;
            self.expanded_check_steps.clear();
            self.pull_request_check_log_target = Some(target);
            self.pull_request_log_read_at = None;
        }
        self.pull_request_check_log_generation =
            self.pull_request_check_log_generation.wrapping_add(1);
        self.pull_request_check_log_loading = true;
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadCheckRunLog {
            generation: self.pull_request_check_log_generation,
            pull_request: Box::new(pull_request),
            check: Box::new(check),
        })));
    }

    /// Warm every finished run's log once per pull request. Selecting a check
    /// then costs a disk read rather than a round trip, which is the difference
    /// between the list being browsable and being a series of waits.
    fn request_check_log_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        let settled: Vec<PullRequestCheck> = self
            .pull_request_checks
            .iter()
            .filter(|check| !check.status.is_running() && check.job_id().is_some())
            .filter(|check| {
                !self
                    .pull_request_prefetched_logs
                    .contains(&check.identity())
            })
            .take(32_usize.saturating_sub(self.pull_request_prefetched_logs.len()))
            .cloned()
            .collect();
        if settled.is_empty() {
            return;
        }
        self.pull_request_prefetched_logs
            .extend(settled.iter().map(PullRequestCheck::identity));
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::PrefetchCheckRunLogs {
                generation: 0,
                pull_request: Box::new(pull_request),
                checks: settled,
            },
        )));
    }

    fn request_pull_request_conversation(&mut self, refresh: bool, effects: &mut Vec<AppEffect>) {
        if self.pull_request_conversation_loading {
            self.pull_request_conversation_refresh_again |= refresh;
            return;
        }
        if !refresh && !self.pull_request_conversation.entries.is_empty() {
            return;
        }
        let Some(pull_request) = self.pull_request.clone() else {
            return;
        };
        self.pull_request_conversation_generation =
            self.pull_request_conversation_generation.wrapping_add(1);
        self.pull_request_conversation_loading = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestConversation {
                generation: self.pull_request_conversation_generation,
                pull_request: Box::new(pull_request),
            },
        )));
    }

    fn queue_operation(&mut self, operation: GitOperation, effects: &mut Vec<AppEffect>) {
        if self.busy.is_some() {
            return;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        self.operation_frame = 0;
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Operate {
            id: self.operation_id,
            operation,
        })));
    }

    fn request_active_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        if self.view == View::Changes && self.auxiliary_preview.is_none() {
            self.changes_diff_version = self.changes_diff_version.wrapping_add(1);
            self.invalidate_preview();
            self.local_diff_loading_path = None;
        }
        self.request_refresh(effects);
        if !self.history_branches_loading {
            self.request_history_branches(effects);
        }
        if self.view == View::PullRequests
            && let Some(number) = self
                .pull_request_exact_number
                .or_else(|| self.pull_request_lookup.value.trim().parse::<u64>().ok())
        {
            self.request_pull_request_lookup(number, true, false, effects);
        }
    }

    fn request_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        if self.refreshing {
            self.refresh_again = true;
            return;
        }
        self.status_generation = self.status_generation.wrapping_add(1);
        self.refreshing = true;
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Refresh {
            generation: self.status_generation,
        })));
    }

    /// A `silent` lookup is a background poll: it keeps the loaded pull request,
    /// its section, its cursors and its diff on screen, and only replaces them
    /// once fresher metadata actually arrives.
    fn request_pull_request_lookup(
        &mut self,
        number: u64,
        refresh: bool,
        silent: bool,
        effects: &mut Vec<AppEffect>,
    ) {
        if silent && self.pull_request_loading {
            return;
        }
        self.pull_request_generation = self.pull_request_generation.wrapping_add(1);
        self.pull_request_loading = true;
        self.pull_request_exact_number = Some(number);
        if !silent {
            self.pull_request_error = None;
            self.invalidate_preview();
            self.pull_request_progress = Some(PullRequestProgress::LoadingMetadata);
            self.pull_request_warnings.clear();
            self.pull_request = None;
            self.reset_pull_request_runtime();
            self.document = DiffDocument::empty(
                format!("Opening Pull Request #{number}"),
                PullRequestProgress::LoadingMetadata.label(),
            );
        }
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LookupPullRequest {
            generation: self.pull_request_generation,
            repositories: self.github_repositories.clone(),
            repository: self.pull_request_repository.clone().map(Box::new),
            number,
            refresh,
        })));
    }

    fn request_history(&mut self, reset: bool, effects: &mut Vec<AppEffect>) {
        if self.history_loading {
            self.history_refresh_again |= reset;
            return;
        }
        self.history_generation = self.history_generation.wrapping_add(1);
        self.history_loading = true;
        if reset {
            self.history_complete = false;
        }
        let skip = if reset { 0 } else { self.history.len() };
        effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadHistory {
            generation: self.history_generation,
            revision: self.history_revision(),
            skip,
            limit: HISTORY_PAGE_SIZE,
        })));
    }

    fn local_diff_request_for_view(&self) -> Option<LocalDiffRequest> {
        match self.view {
            View::Changes => {
                if let Some(preview) = self.auxiliary_preview.clone() {
                    return Some(match preview {
                        AuxiliaryPreview::Branch(branch) => LocalDiffRequest::Branch {
                            branch: Box::new(branch),
                            current: if self.status.branch.head.is_empty() {
                                "HEAD".to_owned()
                            } else {
                                self.status.branch.head.clone()
                            },
                            current_oid: self.status.branch.oid.clone(),
                            expanded: self.expanded_diff,
                        },
                        AuxiliaryPreview::Stash(stash) => LocalDiffRequest::Stash {
                            stash: Box::new(stash),
                            expanded: self.expanded_diff,
                        },
                    });
                }
                let changes = if self.selected_change_section.is_some() {
                    self.selected_section_changes()
                } else {
                    self.selected_change().cloned().into_iter().collect()
                };
                Some(LocalDiffRequest::Changes {
                    changes,
                    version: self.changes_diff_version,
                    expanded: self.expanded_diff,
                })
            }
            View::History => {
                self.selected_commit()
                    .cloned()
                    .map(|commit| LocalDiffRequest::Commit {
                        commit: Box::new(commit),
                        expanded: self.expanded_diff,
                    })
            }
            View::PullRequests => None,
        }
    }

    fn prepare_local_diff(&mut self, request: LocalDiffRequest, effects: &mut Vec<AppEffect>) {
        if self.local_diff_request.as_ref() == Some(&request)
            && (self.local_diff_workspace_generation.is_some() || self.document_loading)
        {
            return;
        }
        let title = match &request {
            LocalDiffRequest::Changes { changes, .. } => changes
                .first()
                .map_or_else(|| "Working Tree".to_owned(), Change::display_path),
            LocalDiffRequest::Commit { commit, .. } => {
                format!("{} — {}", commit.short_id, commit.subject)
            }
            LocalDiffRequest::Branch { branch, .. } => {
                format!("Compare HEAD with {}", branch.name)
            }
            LocalDiffRequest::Stash { stash, .. } => {
                format!("{} — {}", stash.reference, stash.message)
            }
        };
        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        self.reset_local_diff_runtime();
        self.local_diff_request = Some(request.clone());
        self.document_loading = true;
        self.selected_preview_file = None;
        self.preview_file_cursor = 0;
        self.collapsed_preview_files.clear();
        self.expanded_preview_files.clear();
        self.document = DiffDocument::empty(title, "Indexing changed files…");
        effects.push(AppEffect::Git(Box::new(WorkerCommand::PrepareLocalDiff {
            generation,
            request: Box::new(request),
        })));
    }

    #[expect(
        clippy::unreachable,
        reason = "the branch is impossible for the states that reach it"
    )]
    fn request_preview(&mut self, effects: &mut Vec<AppEffect>) {
        if let Some(request) = self.local_diff_request_for_view() {
            self.prepare_local_diff(request, effects);
            if self.view == View::History {
                let visible_len = self.visible_commit_indices().len();
                if self.history_cursor + 20 >= visible_len
                    && !self.history_loading
                    && !self.history_complete
                    && self.filter.is_empty()
                {
                    self.request_history(false, effects);
                }
            }
            return;
        }

        if self.view == View::History {
            self.reset_local_diff_runtime();
            self.document_loading = false;
            self.document = DiffDocument::empty(
                "Commit History",
                if self.history.is_empty() {
                    "No commits in this repository"
                } else {
                    "No commits match the current filter"
                },
            );
            return;
        }

        match self.view {
            View::Changes | View::History => unreachable!("local diff views returned above"),
            View::PullRequests => {
                let Some(pull_request) = self.selected_pull_request().cloned() else {
                    self.document_loading = false;
                    if self.pull_request_section == PullRequestSection::Files {
                        self.document = DiffDocument::empty(
                            "Open Pull Request",
                            if self.pull_request_loading {
                                "Fetching pull-request metadata…"
                            } else {
                                "Enter a pull-request number and press Enter"
                            },
                        );
                    }
                    return;
                };
                let preparing = self.prepare_pull_request_workspace(&pull_request, effects);
                if self.pull_request_section == PullRequestSection::Overview {
                    self.request_check_run_log(false, effects);
                    return;
                }
                if preparing {
                    self.document = pull_request_loading_document(
                        &pull_request,
                        PullRequestProgress::PreparingRepository.label(),
                    );
                    return;
                }

                match self.pull_request_file_view {
                    PullRequestFileView::AllFiles => {
                        self.show_pull_request_all_files();
                        self.request_pull_request_prefetch(effects);
                    }
                    PullRequestFileView::SingleFile => {
                        let Some(path) = self
                            .selected_pull_request_file()
                            .map(|file| file.path.clone())
                        else {
                            self.show_pull_request_all_files();
                            return;
                        };
                        self.request_pull_request_diff_file(path, true, effects);
                    }
                }
            }
        }
    }

    /// Queue the isolated diff workspace unless one is already prepared or in
    /// flight. Returns whether the caller still has to wait for its index.
    fn prepare_pull_request_workspace(
        &mut self,
        pull_request: &PullRequest,
        effects: &mut Vec<AppEffect>,
    ) -> bool {
        if self.pull_request_workspace_generation.is_some() {
            return false;
        }
        if self.document_loading && self.pull_request_progress.is_some() {
            return true;
        }
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.document_loading = true;
        self.pull_request_progress = Some(PullRequestProgress::PreparingRepository);
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::PreparePullRequest {
                generation: self.diff_generation,
                pull_request: Box::new(pull_request.clone()),
            },
        )));
        true
    }

    const fn invalidate_preview(&mut self) {
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.document_loading = false;
        self.preview_due = None;
    }

    fn schedule_preview(&mut self, now: Instant) {
        self.invalidate_preview();
        if self.view != View::PullRequests {
            self.reset_local_diff_runtime();
            self.document = self.loading_document_for_view(self.view);
        }
        self.preview_due = Some(now + PREVIEW_DEBOUNCE);
    }

    fn normalize_selection(&mut self) {
        let targets = self.change_targets();
        if self.selected_change_target().is_none()
            && let Some(first) = targets.first().copied()
        {
            self.select_change_target(first);
        }
        self.change_cursor = self
            .change_cursor
            .min(self.visible_change_indices().len().saturating_sub(1));
        self.history_cursor = self
            .history_cursor
            .min(self.visible_commit_indices().len().saturating_sub(1));
        self.sidebar_offset = 0;
    }

    fn restore_change_selection(&mut self, selected: Option<&Change>) {
        let visible = self.visible_change_indices();
        if let Some(selected) = selected
            && let Some(cursor) = visible.iter().position(|index| {
                self.status.changes.get(*index).is_some_and(|change| {
                    change.path == selected.path && change.area == selected.area
                })
            })
        {
            self.selected_change_section = None;
            self.change_cursor = cursor;
            return;
        }
        if self.selected_change_target().is_none() {
            let preferred = [ChangeSection::Unstaged, ChangeSection::Untracked]
                .into_iter()
                .find(|section| {
                    visible.iter().any(|index| {
                        self.status
                            .changes
                            .get(*index)
                            .is_some_and(|change| section.matches(change))
                    })
                });
            if let Some(section) = preferred {
                self.selected_change_section = Some(section);
            } else if let Some(target) = self.change_targets().first().copied() {
                self.select_change_target(target);
            }
        }
        self.change_cursor = self.change_cursor.min(visible.len().saturating_sub(1));
    }

    /// Reporting the mouse to the application is what stops a terminal from
    /// selecting text with it. Releasing it hands selection and copying back to
    /// the terminal, which is the only place either can happen.
    fn toggle_mouse_capture(&mut self, now: Instant) -> AppEffect {
        self.mouse_capture = !self.mouse_capture;
        self.show_toast(
            if self.mouse_capture {
                "Mouse on · quinjet handles clicks and the wheel".to_owned()
            } else {
                "Mouse off · select and copy with the terminal, m to restore".to_owned()
            },
            ToastLevel::Info,
            now,
        );
        AppEffect::SetMouseCapture(self.mouse_capture)
    }

    /// A check's own link is the run it describes; anywhere else in the pull
    /// request view the pull request itself is what the reader is looking at.
    fn github_url_for_selection(&self) -> Option<&str> {
        if self.view != View::PullRequests {
            return None;
        }
        let check = self
            .selected_pull_request_check()
            .map(|check| check.link.as_str())
            .filter(|link| !link.is_empty());
        check.or_else(|| {
            self.pull_request
                .as_ref()
                .map(|pull_request| pull_request.url.as_str())
                .filter(|url| !url.is_empty())
        })
    }

    fn open_selection_on_github(&mut self, effects: &mut Vec<AppEffect>, now: Instant) {
        match self.github_url_for_selection() {
            Some(url) => {
                let url = url.to_owned();
                self.show_toast(format!("Opening {url}"), ToastLevel::Info, now);
                effects.push(AppEffect::Open(OpenTarget::Browser(url)));
            }
            None => self.show_toast(
                "Nothing to open: look up a pull request first".to_owned(),
                ToastLevel::Error,
                now,
            ),
        }
    }

    pub(crate) fn repository_open_target(&self) -> Option<OpenTarget> {
        self.local_github_repository
            .as_ref()
            .or_else(|| {
                self.pull_request
                    .as_ref()
                    .map(|pull_request| &pull_request.base_repository)
            })
            .map(|repository| OpenTarget::Browser(repository.url.clone()))
    }

    pub(crate) fn branch_open_target(&self, branch: &str) -> Option<OpenTarget> {
        self.repository_web_url().map(|repository| {
            let mut url = repository.trim_end_matches('/').to_owned();
            url.push_str("/tree/");
            url.push_str(&encode_url_path(branch));
            OpenTarget::Browser(url)
        })
    }

    pub(crate) fn commit_open_target(&self, commit: &str) -> Option<OpenTarget> {
        self.repository_web_url().map(|repository| {
            let mut url = repository.trim_end_matches('/').to_owned();
            url.push_str("/commit/");
            url.push_str(&encode_url_path(commit));
            OpenTarget::Browser(url)
        })
    }

    pub(crate) fn pull_request_open_target(&self, number: u64) -> Option<OpenTarget> {
        if let Some(pull_request) = self
            .pull_request
            .as_ref()
            .filter(|pull_request| pull_request.number == number)
        {
            return Some(OpenTarget::Browser(pull_request.url.clone()));
        }
        self.repository_web_url().map(|repository| {
            OpenTarget::Browser(format!(
                "{}/pull/{number}",
                repository.trim_end_matches('/')
            ))
        })
    }

    pub(crate) fn workspace_open_target(&self) -> OpenTarget {
        OpenTarget::Path(self.repository_root.clone())
    }

    fn repository_web_url(&self) -> Option<&str> {
        self.local_github_repository
            .as_ref()
            .map(|repository| repository.url.as_str())
            .or_else(|| {
                self.pull_request
                    .as_ref()
                    .map(|pull_request| pull_request.base_repository.url.as_str())
            })
    }

    fn show_toast(&mut self, message: String, level: ToastLevel, now: Instant) {
        self.toast = Some(Toast {
            message,
            level,
            expires_at: now + TOAST_DURATION,
        });
    }
}

fn pull_request_loading_document(pull_request: &PullRequest, message: &str) -> DiffDocument {
    let mut document = DiffDocument::empty(
        format!(
            "PR #{} — {}  ·  {} → {}",
            pull_request.number,
            pull_request.title,
            pull_request.head_label(),
            pull_request.base_label(),
        ),
        message,
    );
    document.pull_request_details = Some(pull_request_details(pull_request));
    document
}

const fn previous_list_index(selected: usize, length: usize) -> usize {
    if selected == 0 {
        length.saturating_sub(1)
    } else {
        selected.saturating_sub(1)
    }
}

const fn next_list_index(selected: usize, length: usize) -> usize {
    if selected.saturating_add(1) >= length {
        0
    } else {
        selected.saturating_add(1)
    }
}

fn diff_document_size(document: &DiffDocument) -> usize {
    let lines = document.lines.iter().fold(0_usize, |total, line| {
        let spans = line.spans.iter().fold(0_usize, |span_total, span| {
            span_total.saturating_add(size_of_val(span) + span.text.capacity())
        });
        total
            .saturating_add(size_of_val(line))
            .saturating_add(spans)
    });
    size_of_val(document)
        .saturating_add(document.title.capacity())
        .saturating_add(lines)
}

fn pull_request_details(pull_request: &PullRequest) -> PullRequestDetails {
    PullRequestDetails {
        number: pull_request.number,
        title: pull_request.title.clone(),
        description: pull_request.description.clone(),
        author: pull_request.author.clone(),
        state: pull_request.state.clone(),
        is_draft: pull_request.is_draft,
        updated_at: pull_request.updated_at.clone(),
        url: pull_request.url.clone(),
        base_repository: pull_request.base_repository.display_name(),
        base_ref: pull_request.base_ref.clone(),
        base_remotes: pull_request.base_repository.remotes.clone(),
        head_repository: pull_request.head_repository.clone(),
        head_ref: pull_request.head_ref.clone(),
        head_remotes: pull_request.head_remotes.clone(),
        is_cross_repository: pull_request.is_cross_repository,
        changed_files: pull_request.changed_files,
        additions: pull_request.additions,
        deletions: pull_request.deletions,
        selected_file: None,
        selected_file_additions: 0,
        selected_file_deletions: 0,
    }
}

const fn pull_request_file_status_label(status: PullRequestFileStatus) -> &'static str {
    match status {
        PullRequestFileStatus::Added => "added",
        PullRequestFileStatus::Modified => "modified",
        PullRequestFileStatus::Deleted => "deleted",
        PullRequestFileStatus::Renamed => "renamed",
        PullRequestFileStatus::Copied => "copied",
        PullRequestFileStatus::TypeChanged => "type changed",
        PullRequestFileStatus::Unmerged => "unmerged",
        PullRequestFileStatus::Unknown => "changed",
    }
}

fn edit_text(input: &mut TextBuffer, key: KeyEvent, multiline: bool) {
    let word_modifier = key
        .modifiers
        .intersects(KeyModifiers::ALT | KeyModifiers::CONTROL);
    let command_modifier = key
        .modifiers
        .intersects(KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER);

    match key.code {
        KeyCode::Backspace if command_modifier => input.delete_to_line_start(),
        KeyCode::Delete if command_modifier => input.delete_to_line_end(),
        KeyCode::Backspace if word_modifier => input.delete_word_backward(),
        KeyCode::Delete if word_modifier => input.delete_word_forward(),
        KeyCode::Left if command_modifier => input.home(),
        KeyCode::Right if command_modifier => input.end(),
        KeyCode::Left if word_modifier => input.move_word_left(),
        KeyCode::Right if word_modifier => input.move_word_right(),
        KeyCode::Home if command_modifier => input.document_start(),
        KeyCode::End if command_modifier => input.document_end(),
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.document_start();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => input.end(),
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => input.move_left(),
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => input.move_right(),
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_word_backward();
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_to_line_start();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            input.delete_to_line_end();
        }
        KeyCode::Char(character)
            if !key.modifiers.intersects(
                KeyModifiers::CONTROL
                    | KeyModifiers::ALT
                    | KeyModifiers::SUPER
                    | KeyModifiers::META
                    | KeyModifiers::HYPER,
            ) =>
        {
            input.insert(character);
        }
        KeyCode::Backspace => input.backspace(),
        KeyCode::Delete => input.delete(),
        KeyCode::Left => input.move_left(),
        KeyCode::Right => input.move_right(),
        KeyCode::Home => input.home(),
        KeyCode::End => input.end(),
        KeyCode::Enter if multiline => input.insert('\n'),
        _ => {}
    }
}

fn previous_character(value: &str, cursor: usize) -> Option<(usize, char)> {
    value.get(..cursor)?.char_indices().next_back()
}

fn next_character(value: &str, cursor: usize) -> Option<(usize, char)> {
    let character = value.get(cursor..)?.chars().next()?;
    Some((cursor + character.len_utf8(), character))
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn encode_url_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else if write!(encoded, "%{byte:02X}").is_err() {
            return String::new();
        }
    }
    encoded
}

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests {
    use super::*;
    use crate::git::github::PullRequestCheckStatus;
    use crate::git::status::{BranchState, ChangeStatus};

    fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
        PullRequestCheck {
            name: name.to_owned(),
            workflow: "CI".to_owned(),
            state: format!("{status:?}").to_uppercase(),
            status,
            description: String::new(),
            link: "https://github.com/acme/widget/actions/runs/1/job/2".to_owned(),
            started_at: "2026-08-14T18:00:00Z".to_owned(),
            completed_at: String::new(),
        }
    }

    fn pull_request(number: u64, title: &str, repository: &str) -> PullRequest {
        PullRequest {
            number,
            title: title.to_owned(),
            description: "A detailed pull-request description".to_owned(),
            author: "octocat".to_owned(),
            state: "OPEN".to_owned(),
            is_draft: false,
            created_at: format!("2026-07-{number:02}T00:00:00Z"),
            updated_at: format!("2026-08-{number:02}T00:00:00Z"),
            url: format!("https://github.com/{repository}/pull/{number}"),
            base_ref: "main".to_owned(),
            base_oid: format!("base-{number}"),
            head_ref: format!("feature/{number}"),
            head_oid: format!("head-{number}"),
            base_repository: GitHubRepository {
                name_with_owner: repository.to_owned(),
                url: format!("https://github.com/{repository}"),
                remotes: vec!["upstream".to_owned()],
            },
            head_repository: Some("octocat/fork".to_owned()),
            head_remotes: vec!["origin".to_owned()],
            is_cross_repository: true,
            additions: usize::try_from(number).unwrap_or(usize::MAX),
            deletions: 1,
            changed_files: 2,
        }
    }

    fn app_with_changes() -> App {
        let mut app = App::new("/tmp/repo", "repo");
        app.status = RepoStatus {
            branch: BranchState::default(),
            changes: vec![
                Change {
                    path: PathBuf::from("src/main.rs"),
                    original_path: None,
                    area: ChangeArea::Unstaged,
                    status: ChangeStatus::Modified,
                },
                Change {
                    path: PathBuf::from("README.md"),
                    original_path: None,
                    area: ChangeArea::Staged,
                    status: ChangeStatus::Modified,
                },
            ],
        };
        app.selected_change_section = None;
        app
    }

    fn indexed_document(paths: &[&str]) -> DiffDocument {
        DiffIndex {
            title: "Diff".to_owned(),
            files: paths
                .iter()
                .map(|path| crate::git::diff::DiffFileIndexEntry {
                    path: PathBuf::from(path),
                    old_path: None,
                    status: "modified".to_owned(),
                    counts: None,
                })
                .collect(),
            truncated: false,
            commit_details: None,
        }
        .document(&HashMap::new())
    }

    #[test]
    fn text_buffer_edits_unicode_on_character_boundaries() {
        let mut buffer = TextBuffer::new("a🚀b");
        buffer.move_left();
        buffer.backspace();
        assert_eq!(buffer.value, "ab");
        buffer.insert('é');
        assert_eq!(buffer.value, "aéb");
    }

    #[test]
    fn text_buffer_supports_word_and_line_deletion() {
        let mut buffer = TextBuffer::new("first second\nthird word");
        buffer.delete_word_backward();
        assert_eq!(buffer.value, "first second\nthird ");
        buffer.delete_to_line_start();
        assert_eq!(buffer.value, "first second\n");
        buffer.document_start();
        buffer.delete_word_forward();
        assert_eq!(buffer.value, " second\n");
        buffer.document_end();
        buffer.delete_to_line_start();
        assert_eq!(buffer.value, " second\n");
    }

    #[test]
    fn pane_resize_is_clamped_to_usable_bounds() {
        let mut app = App::new("/tmp/repo", "repo");
        app.geometry.main = Rect::new(5, 3, 120, 30);
        app.geometry.content = Rect::new(48, 3, 77, 30);

        app.resize_sidebar(120);
        assert_eq!(app.sidebar_width, 88);
        app.resize_sidebar(6);
        assert_eq!(app.sidebar_width, 22);
        app.resize_diff(49);
        assert_eq!(app.diff_split_percent, 20);
        app.resize_diff(124);
        assert_eq!(app.diff_split_percent, 80);
    }

    #[test]
    fn double_tapping_each_divider_restores_its_default_size() {
        fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
            MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            }
        }

        let mut app = App::new("/tmp/repo", "repo");
        app.geometry.main = Rect::new(5, 3, 120, 30);
        app.geometry.sidebar_divider = Rect::new(82, 3, 1, 30);
        app.geometry.content = Rect::new(48, 3, 77, 30);
        app.geometry.diff_divider = Some(Rect::new(109, 3, 1, 30));
        app.sidebar_width = 77;
        app.diff_split_percent = 80;
        let now = Instant::now();

        app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 82, 10), now);
        app.handle_mouse(
            mouse(MouseEventKind::Up(MouseButton::Left), 82, 10),
            now + Duration::from_millis(20),
        );
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), 82, 10),
            now + Duration::from_millis(120),
        );
        assert_eq!(app.sidebar_width, DEFAULT_SIDEBAR_WIDTH);
        assert_eq!(app.resize_target, None);

        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), 109, 10),
            now + Duration::from_millis(600),
        );
        app.handle_mouse(
            mouse(MouseEventKind::Up(MouseButton::Left), 109, 10),
            now + Duration::from_millis(620),
        );
        app.handle_mouse(
            mouse(MouseEventKind::Down(MouseButton::Left), 109, 10),
            now + Duration::from_millis(720),
        );
        assert_eq!(app.diff_split_percent, DEFAULT_DIFF_SPLIT_PERCENT);
        assert_eq!(app.resize_target, None);
    }

    #[test]
    fn filters_changes_without_losing_underlying_index() {
        let mut app = app_with_changes();
        app.filter = "read".to_owned();
        assert_eq!(app.visible_change_indices(), vec![1]);
        assert_eq!(
            app.selected_change().unwrap().path,
            PathBuf::from("README.md")
        );
    }

    #[test]
    fn discard_on_a_conflict_opens_resolution_instead() {
        let mut app = app_with_changes();
        app.status.changes[0].area = ChangeArea::Conflict;
        app.status.changes[0].status = ChangeStatus::Conflicted;

        let effects = app.handle_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            Instant::now(),
        );

        assert!(effects.is_empty());
        assert!(matches!(app.modal, Some(Modal::Conflict { .. })));
    }

    #[test]
    fn enter_toggles_focus_between_sidebar_and_content() {
        let mut app = app_with_changes();
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert_eq!(app.focus, Focus::Content);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn reading_a_pull_request_never_pushes_your_own_branch() {
        let mut app = app_with_changes();
        let now = Instant::now();
        app.view = View::PullRequests;

        for character in ['p', 'f'] {
            let effects = app.handle_key(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                now,
            );
            assert!(
                effects.is_empty(),
                "{character} queued work from inside the pull-request view"
            );
            assert!(app.busy.is_none(), "{character} started a git operation");
        }

        app.view = View::Changes;
        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), now);
        assert!(app.busy.is_some(), "fetch still runs from the changes view");
    }

    #[test]
    fn shift_o_opens_the_run_being_read_rather_than_always_the_pull_request() {
        let mut app = app_with_changes();
        let now = Instant::now();
        app.view = View::PullRequests;

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
        assert!(effects.is_empty(), "nothing is open, so nothing to open");

        app.pull_request = Some(PullRequest {
            url: "https://github.com/o/r/pull/8".to_owned(),
            ..PullRequest::default()
        });
        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Open(OpenTarget::Browser(url))]
                if url == "https://github.com/o/r/pull/8"
        ));

        app.pull_request_checks = vec![PullRequestCheck {
            name: "build".to_owned(),
            workflow: "CI".to_owned(),
            state: "SUCCESS".to_owned(),
            status: PullRequestCheckStatus::Passed,
            description: String::new(),
            link: "https://github.com/o/r/actions/runs/1/job/2".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
        }];
        app.set_check_cursor(Some(0));
        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE), now);
        assert!(
            matches!(
                effects.as_slice(),
                [AppEffect::Open(OpenTarget::Browser(url))]
                    if url.contains("/actions/runs/1/job/2")
            ),
            "a selected check opens the run it names, not the pull request"
        );
    }

    #[test]
    fn the_mouse_can_be_released_so_the_terminal_can_select_text() {
        let mut app = app_with_changes();
        let now = Instant::now();
        assert!(app.mouse_capture);

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
        assert!(!app.mouse_capture);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::SetMouseCapture(false)]
        ));
        assert!(
            app.toast
                .as_ref()
                .is_some_and(|toast| toast.message.contains("select and copy")),
            "the reader is told what releasing the mouse just gave them"
        );

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
        assert!(app.mouse_capture);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::SetMouseCapture(true)]
        ));
    }

    #[test]
    fn the_mouse_is_released_even_while_the_number_field_has_focus() {
        let mut app = app_with_changes();
        let now = Instant::now();
        app.pull_request_lookup_active = true;

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), now);
        assert!(!app.mouse_capture);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::SetMouseCapture(false)]
        ));
        assert!(
            app.pull_request_lookup_active,
            "releasing the mouse does not close the field"
        );
    }

    #[test]
    fn z_hides_and_restores_sidebar_without_replacing_hunk_shortcuts() {
        let mut app = app_with_changes();
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
        assert!(app.sidebar_hidden);
        assert_eq!(app.focus, Focus::Content);
        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
        assert!(!app.sidebar_hidden);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn z_works_while_the_pull_request_number_input_has_focus() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.switch_view(View::PullRequests, &mut Vec::new());
        assert!(app.pull_request_lookup_active);

        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
        assert!(app.sidebar_hidden);
        assert_eq!(app.focus, Focus::Content);
        assert!(!app.pull_request_lookup_active);

        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE), now);
        assert!(!app.sidebar_hidden);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn change_sections_share_navigation_folding_and_membership() {
        let mut app = app_with_changes();
        let now = Instant::now();
        app.status.changes.push(Change {
            path: PathBuf::from("new.txt"),
            original_path: None,
            area: ChangeArea::Unstaged,
            status: ChangeStatus::Untracked,
        });
        app.selected_change_section = Some(ChangeSection::Staged);

        app.navigate(1, now);
        assert!(app.selected_change_section.is_none());
        assert_eq!(app.selected_change().unwrap().area, ChangeArea::Staged);
        app.navigate(1, now);
        assert_eq!(app.selected_change_section, Some(ChangeSection::Unstaged));
        assert_eq!(app.selected_section_changes().len(), 1);

        assert!(app.toggle_selected_change_section());
        assert!(
            app.collapsed_change_sections
                .contains(&ChangeSection::Unstaged)
        );
        assert!(!app.change_rows().iter().any(|row| matches!(
            row,
            ChangeRow::Change {
                section: ChangeSection::Unstaged,
                ..
            }
        )));

        app.navigate(1, now);
        assert_eq!(app.selected_change_section, Some(ChangeSection::Untracked));
        assert_eq!(app.selected_section_changes().len(), 1);
        assert_eq!(
            app.selected_section_changes()[0].path,
            PathBuf::from("new.txt")
        );
    }

    #[test]
    fn clicking_a_file_stage_action_queues_only_that_path() {
        let mut app = app_with_changes();
        app.geometry.scm_action_hits = vec![ScmActionHit {
            area: Rect::new(30, 8, 4, 1),
            action: ScmAction::Stage(0),
        }];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 31,
            row: 8,
            modifiers: KeyModifiers::NONE,
        };

        let effects = app.handle_mouse(click, Instant::now());

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::Operate {
                    operation: GitOperation::Stage(paths),
                    ..
                } if paths == &[PathBuf::from("src/main.rs")]
            )
        ));
    }

    #[test]
    fn clicking_link_text_opens_its_target_before_the_containing_row() {
        let mut app = app_with_changes();
        app.geometry.sidebar = Rect::new(0, 0, 40, 20);
        app.geometry.sidebar_hits = vec![SidebarHitArea {
            area: Rect::new(0, 4, 40, 1),
            target: SidebarHit::Change(0),
        }];
        app.geometry.link_hits = vec![LinkHit {
            area: Rect::new(30, 4, 7, 1),
            target: OpenTarget::Browser("https://github.com/acme/widget/commit/abc".to_owned()),
        }];

        let effects = app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 32,
                row: 4,
                modifiers: KeyModifiers::NONE,
            },
            Instant::now(),
        );

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Open(OpenTarget::Browser(url))]
                if url == "https://github.com/acme/widget/commit/abc"
        ));
        assert_eq!(app.selected_change_section, None);
        assert_eq!(app.change_cursor, 0);
    }

    #[test]
    fn github_reference_targets_encode_branch_paths() {
        let mut app = App::new("/tmp/repo", "repo");
        app.local_github_repository = Some(GitHubRepository {
            name_with_owner: "acme/widget".to_owned(),
            url: "https://github.com/acme/widget".to_owned(),
            remotes: vec!["origin".to_owned()],
        });

        assert_eq!(
            app.branch_open_target("feature/fix #42"),
            Some(OpenTarget::Browser(
                "https://github.com/acme/widget/tree/feature/fix%20%2342".to_owned()
            ))
        );
        assert_eq!(
            app.pull_request_open_target(42),
            Some(OpenTarget::Browser(
                "https://github.com/acme/widget/pull/42".to_owned()
            ))
        );
    }

    #[test]
    fn clicking_a_change_section_selects_and_collapses_only_that_section() {
        let mut app = app_with_changes();
        app.geometry.sidebar = Rect::new(0, 0, 40, 20);
        app.geometry.sidebar_hits = vec![SidebarHitArea {
            area: Rect::new(0, 3, 40, 1),
            target: SidebarHit::ChangeSection(ChangeSection::Unstaged),
        }];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 8,
            row: 3,
            modifiers: KeyModifiers::NONE,
        };

        app.handle_mouse(click, Instant::now());

        assert_eq!(app.selected_change_section, Some(ChangeSection::Unstaged));
        assert!(
            app.collapsed_change_sections
                .contains(&ChangeSection::Unstaged)
        );
        assert!(
            app.selected_section_changes()
                .iter()
                .all(|change| change.status != ChangeStatus::Untracked)
        );
    }

    #[test]
    fn clicking_a_file_header_toggles_only_that_file() {
        let mut app = App::new("/tmp/repo", "repo");
        app.document = indexed_document(&["src/main.rs", "src/lib.rs"]);
        app.geometry.content = Rect::new(20, 4, 80, 20);
        app.geometry.content_file_hits = vec![ContentFileHit {
            area: Rect::new(20, 8, 80, 1),
            path: PathBuf::from("src/main.rs"),
        }];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 24,
            row: 8,
            modifiers: KeyModifiers::NONE,
        };

        app.handle_mouse(click, Instant::now());
        assert!(
            app.collapsed_preview_files
                .contains(Path::new("src/main.rs"))
        );
        app.handle_mouse(click, Instant::now());
        assert!(
            !app.collapsed_preview_files
                .contains(Path::new("src/main.rs"))
        );
    }

    #[test]
    fn single_file_views_cannot_be_collapsed() {
        let mut app = App::new("/tmp/repo", "repo");
        app.document = indexed_document(&["src/main.rs"]);
        app.selected_preview_file = Some(PathBuf::from("src/main.rs"));
        let now = Instant::now();

        assert!(!app.preview_files_collapsible());
        assert!(!app.preview_file_collapsed("src/main.rs"));
        assert!(!app.preview_files_all_collapsed());

        app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
        app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());

        assert!(!app.files_collapsed);
        assert!(app.collapsed_preview_files.is_empty());
        assert!(app.expanded_preview_files.is_empty());
        assert!(!app.preview_file_collapsed("src/main.rs"));
    }

    #[test]
    fn preserves_selected_change_across_status_refresh_order() {
        let mut app = app_with_changes();
        app.change_cursor = 1;
        let selected = app.selected_change().cloned();
        app.status.changes.swap(0, 1);
        app.restore_change_selection(selected.as_ref());
        assert_eq!(
            app.selected_change().unwrap().path,
            PathBuf::from("README.md")
        );
    }

    #[test]
    fn startup_does_not_fetch_any_pull_request_data() {
        let mut app = App::new("/tmp/repo", "repo");

        let effects = app.initial_effects();

        assert!(matches!(
            effects.as_slice(),
            [
                AppEffect::Git(refresh),
                AppEffect::Git(history),
                AppEffect::Git(branches),
                AppEffect::Git(repository),
            ] if matches!(refresh.as_ref(), WorkerCommand::Refresh { .. })
                && matches!(history.as_ref(), WorkerCommand::LoadHistory { .. })
                && matches!(branches.as_ref(), WorkerCommand::LoadHistoryBranches { .. })
                && matches!(repository.as_ref(), WorkerCommand::LoadLocalGitHubRepository)
        ));
        assert!(!app.pull_request_loading);
        assert!(app.pull_request.is_none());
        assert_eq!(app.pull_request_generation, 0);
    }

    #[test]
    fn opening_pr_tab_only_focuses_the_number_field() {
        let mut app = App::new("/tmp/repo", "repo");

        let effects = app.handle_key(
            KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
            Instant::now(),
        );

        assert!(effects.is_empty());
        assert_eq!(app.view, View::PullRequests);
        assert!(app.pull_request_lookup_active);
        assert_eq!(app.document.title, "Open Pull Request");
    }

    #[test]
    fn repository_picker_is_also_discovered_only_on_explicit_request() {
        let mut app = App::new("/tmp/repo", "repo");
        app.switch_view(View::PullRequests, &mut Vec::new());

        let effects = app.handle_key(
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
            Instant::now(),
        );

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadGitHubRepositories {
                    generation: 1,
                    refresh: false,
                }
            )
        ));
        assert!(matches!(
            app.modal,
            Some(Modal::PullRequestRepositories { loading: true, .. })
        ));
    }

    #[test]
    fn numeric_lookup_discovers_the_repository_on_demand() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.switch_view(View::PullRequests, &mut Vec::new());
        for character in ['4', '2'] {
            app.handle_key(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                now,
            );
        }

        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LookupPullRequest {
                    number: 42,
                    repository: None,
                    ..
                }
            )
        ));
        assert_eq!(
            app.pull_request_progress,
            Some(PullRequestProgress::LoadingMetadata)
        );
        assert!(app.pull_request_loading);
    }

    #[test]
    fn opening_a_pull_request_prefetches_diffs_checks_and_conversation_from_overview() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_generation = 3;
        app.pull_request_loading = true;
        let request = pull_request(8, "Cross-fork update", "acme/widget");
        let repository = request.base_repository.clone();

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestLookup {
                generation: 3,
                result: Ok(crate::git::github::PullRequestSnapshot {
                    repositories: vec![repository.clone()],
                    selected_repository: Some(repository),
                    pull_request: request,
                    warnings: Vec::new(),
                    exact_number: Some(8),
                    from_cache: false,
                }),
            },
            Instant::now(),
        );

        assert_eq!(effects.len(), 3);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestConversation { pull_request, .. }
                    if pull_request.number == 8
            )
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(
                command.as_ref(),
                WorkerCommand::PreparePullRequest { pull_request, .. }
                    if pull_request.number == 8
            )
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestChecks { pull_request, .. }
                    if pull_request.number == 8
            )
        )));
        assert_eq!(
            app.pull_request_progress,
            Some(PullRequestProgress::PreparingRepository)
        );
        assert_eq!(
            app.pull_request_section,
            PullRequestSection::Overview,
            "an opened pull request lands on itself, not on its files"
        );
        let workspace_generation = app.diff_generation;
        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestIndex {
                generation: workspace_generation,
                result: Ok(PullRequestDiffIndex {
                    files: ["src/first.rs", "src/second.rs"]
                        .into_iter()
                        .map(|path| PullRequestFile {
                            path: PathBuf::from(path),
                            old_path: None,
                            status: PullRequestFileStatus::Modified,
                            counts: None,
                        })
                        .collect(),
                    total_files: 2,
                    truncated: false,
                }),
            },
            Instant::now(),
        );
        assert_eq!(app.pull_request_section, PullRequestSection::Overview);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch {
                    workspace_generation: generation,
                    paths,
                } if *generation == workspace_generation
                    && paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
            )
        ));
        assert_eq!(
            app.pull_request.as_ref().unwrap().description,
            "A detailed pull-request description"
        );
    }

    #[test]
    fn local_diff_index_prefetches_one_file_then_loads_only_an_expanded_path() {
        let mut app = App::new("/tmp/repo", "repo");
        app.diff_generation = 5;
        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffIndex {
                generation: 5,
                result: Ok(DiffIndex {
                    title: "Branch comparison".to_owned(),
                    files: ["src/first.rs", "src/second.rs"]
                        .into_iter()
                        .map(|path| crate::git::diff::DiffFileIndexEntry {
                            path: PathBuf::from(path),
                            old_path: None,
                            status: "modified".to_owned(),
                            counts: None,
                        })
                        .collect(),
                    truncated: false,
                    commit_details: None,
                }),
            },
            Instant::now(),
        );

        assert_eq!(app.document.file_count(), 2);
        assert!(app.preview_files_all_collapsed());
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile {
                    workspace_generation: 5,
                    path,
                    ..
                } if path == Path::new("src/first.rs")
            )
        ));

        let first_generation = app.diff_generation;
        app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: first_generation,
                workspace_generation: 5,
                path: PathBuf::from("src/first.rs"),
                result: Ok(DiffDocument::empty("first", "loaded")),
            },
            Instant::now(),
        );
        let mut effects = Vec::new();
        app.toggle_preview_file(PathBuf::from("src/second.rs"), &mut effects);

        assert_eq!(app.document.file_count(), 2);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile {
                    workspace_generation: 5,
                    path,
                    ..
                } if path == Path::new("src/second.rs")
            )
        ));
    }

    #[test]
    fn expanding_all_local_files_loads_every_visible_diff_in_order() {
        let mut app = App::new("/tmp/repo", "repo");
        app.diff_generation = 5;
        let now = Instant::now();
        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffIndex {
                generation: 5,
                result: Ok(DiffIndex {
                    title: "Commit details".to_owned(),
                    files: ["src/first.rs", "src/second.rs", "src/third.rs"]
                        .into_iter()
                        .map(|path| crate::git::diff::DiffFileIndexEntry {
                            path: PathBuf::from(path),
                            old_path: None,
                            status: "modified".to_owned(),
                            counts: None,
                        })
                        .collect(),
                    truncated: false,
                    commit_details: None,
                }),
            },
            now,
        );
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile { path, .. }
                    if path == Path::new("src/first.rs")
            )
        ));

        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: app.diff_generation,
                workspace_generation: 5,
                path: PathBuf::from("src/first.rs"),
                result: Ok(indexed_document(&["src/first.rs"])),
            },
            now,
        );
        assert!(effects.is_empty());

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile { path, .. }
                    if path == Path::new("src/second.rs")
            )
        ));

        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: app.diff_generation,
                workspace_generation: 5,
                path: PathBuf::from("src/second.rs"),
                result: Ok(indexed_document(&["src/second.rs"])),
            },
            now,
        );
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile { path, .. }
                    if path == Path::new("src/third.rs")
            )
        ));

        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: app.diff_generation,
                workspace_generation: 5,
                path: PathBuf::from("src/third.rs"),
                result: Ok(indexed_document(&["src/third.rs"])),
            },
            now,
        );
        assert!(effects.is_empty());
        assert_eq!(app.local_diff_documents.len(), 3);
        assert!(app.local_diff_pending_paths.is_empty());
    }

    #[test]
    fn an_existing_expand_all_preference_loads_every_file_in_a_new_index() {
        let mut app = App::new("/tmp/repo", "repo");
        app.diff_generation = 9;
        app.files_collapsed = false;
        app.collapse_preference_set = true;

        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffIndex {
                generation: 9,
                result: Ok(DiffIndex {
                    title: "Next commit".to_owned(),
                    files: ["one.rs", "two.rs", "three.rs"]
                        .into_iter()
                        .map(|path| crate::git::diff::DiffFileIndexEntry {
                            path: PathBuf::from(path),
                            old_path: None,
                            status: "modified".to_owned(),
                            counts: None,
                        })
                        .collect(),
                    truncated: false,
                    commit_details: None,
                }),
            },
            Instant::now(),
        );

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadLocalDiffFile { path, .. } if path == Path::new("one.rs")
            )
        ));
        assert_eq!(
            app.local_diff_pending_paths,
            VecDeque::from([PathBuf::from("two.rs"), PathBuf::from("three.rs")])
        );
        assert!(
            app.preview_file_paths()
                .iter()
                .all(|path| !app.preview_file_collapsed(&path.to_string_lossy()))
        );
    }

    #[test]
    fn local_diff_file_replies_must_match_the_prepared_workspace() {
        let mut app = App::new("/tmp/repo", "repo");
        app.diff_generation = 12;
        app.handle_worker_event(
            WorkerEvent::LocalDiffIndex {
                generation: 12,
                result: Ok(DiffIndex {
                    title: "Current".to_owned(),
                    files: vec![crate::git::diff::DiffFileIndexEntry {
                        path: PathBuf::from("same.rs"),
                        old_path: None,
                        status: "modified".to_owned(),
                        counts: None,
                    }],
                    truncated: false,
                    commit_details: None,
                }),
            },
            Instant::now(),
        );

        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: 12,
                workspace_generation: 11,
                path: PathBuf::from("same.rs"),
                result: Ok(DiffDocument::empty("Old commit", "stale")),
            },
            Instant::now(),
        );

        assert!(effects.is_empty());
        assert_eq!(app.local_diff_loading_path, Some(PathBuf::from("same.rs")));
        assert!(!app.local_diff_single_loaded);
        assert_ne!(app.document.title, "Old commit");
    }

    #[test]
    fn pull_request_folders_are_selectable_and_collapsible() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_section = PullRequestSection::Files;
        app.focus = Focus::Sidebar;
        app.pull_request_files = ["src/app.rs", "src/git/diff.rs", "tests/ui.rs"]
            .into_iter()
            .map(|path| PullRequestFile {
                path: PathBuf::from(path),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            })
            .collect();
        app.sync_pull_request_tree_cursor_to_file();

        let cursor = app.pull_request_tree_cursor;
        let entries = app.pull_request_tree_entries();
        assert!(matches!(
            entries.get(cursor),
            Some(PullRequestTreeEntry::File { index: 0, .. })
        ));

        app.handle_key(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            Instant::now(),
        );
        let cursor = app.pull_request_tree_cursor;
        assert!(matches!(
            app.pull_request_tree_entries().get(cursor),
            Some(PullRequestTreeEntry::Directory { path, .. }) if path == Path::new("src")
        ));

        app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Instant::now(),
        );
        assert!(app.pull_request_directory_collapsed(Path::new("src")));
        assert_eq!(
            app.pull_request_tree_entries()
                .iter()
                .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
                .count(),
            1
        );

        app.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            Instant::now(),
        );
        assert!(!app.pull_request_directory_collapsed(Path::new("src")));
        assert_eq!(
            app.pull_request_tree_entries()
                .iter()
                .filter(|entry| matches!(entry, PullRequestTreeEntry::File { .. }))
                .count(),
            3
        );

        app.geometry.sidebar = Rect::new(0, 0, 40, 10);
        app.geometry.sidebar_hits = vec![SidebarHitArea {
            area: Rect::new(0, 2, 40, 1),
            target: SidebarHit::PullRequestDirectory(PathBuf::from("src/git")),
        }];
        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 2,
                modifiers: KeyModifiers::SHIFT,
            },
            Instant::now(),
        );
        assert!(!app.pull_request_directory_collapsed(Path::new("src/git")));

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            Instant::now(),
        );
        assert!(app.pull_request_directory_collapsed(Path::new("src/git")));
        assert!(
            !app.pull_request_tree_entries()
                .iter()
                .any(|entry| { matches!(entry, PullRequestTreeEntry::File { index: 1, .. }) })
        );
    }

    #[test]
    fn pull_request_tree_compacts_single_child_directory_chains() {
        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request_files = [
            "apps/web/app/one.txt",
            "apps/web/modules/marketing/landing/two.txt",
        ]
        .into_iter()
        .map(|path| PullRequestFile {
            path: PathBuf::from(path),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        })
        .collect();

        let entries = app.pull_request_tree_entries();

        assert!(matches!(
            entries.first(),
            Some(PullRequestTreeEntry::Directory { path, label, depth: 0 })
                if path == Path::new("apps/web") && label == "apps/web"
        ));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            PullRequestTreeEntry::Directory { path, label, depth: 1 }
                if path == Path::new("apps/web/modules/marketing/landing")
                    && label == "modules/marketing/landing"
        )));

        app.toggle_pull_request_directory(PathBuf::from("apps/web"));
        assert_eq!(app.pull_request_tree_entries().len(), 1);
    }

    #[test]
    fn pull_request_defaults_to_all_files_then_files_tab_restores_it_from_single_file() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request_section = PullRequestSection::Files;
        app.focus = Focus::Sidebar;
        app.pull_request = Some(pull_request(8, "Large change", "acme/widget"));
        app.diff_generation = 10;
        let files = ["src/first.rs", "src/second.rs"]
            .into_iter()
            .map(|path| PullRequestFile {
                path: PathBuf::from(path),
                old_path: None,
                status: PullRequestFileStatus::Modified,
                counts: None,
            })
            .collect();
        let now = Instant::now();

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestIndex {
                generation: 10,
                result: Ok(PullRequestDiffIndex {
                    files,
                    total_files: 2,
                    truncated: false,
                }),
            },
            now,
        );

        assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
        assert_eq!(app.document.file_count(), 2);
        assert!(app.preview_files_all_collapsed());
        assert!(
            matches!(
                effects.as_slice(),
                [AppEffect::Git(command)] if matches!(
                    command.as_ref(),
                    WorkerCommand::LoadPullRequestFileBatch {
                        workspace_generation: 10,
                        paths,
                    } if paths == &[PathBuf::from("src/first.rs"), PathBuf::from("src/second.rs")]
                )
            ),
            "the whole index is fetched in one batch rather than a file at a time"
        );

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation: 10,
                result: Ok(vec![
                    (
                        PathBuf::from("src/first.rs"),
                        indexed_document(&["src/first.rs"]),
                    ),
                    (
                        PathBuf::from("src/second.rs"),
                        indexed_document(&["src/second.rs"]),
                    ),
                ]),
            },
            now,
        );
        assert!(effects.is_empty(), "no file is left to fetch");
        assert_eq!(app.document.file_count(), 2);

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_file_cursor, 1);
        assert_eq!(app.pull_request_file_view, PullRequestFileView::SingleFile);
        let (effects, _) = app.tick(now + PREVIEW_DEBOUNCE);
        assert!(
            effects.is_empty(),
            "a prefetched file opens without another Git round trip"
        );
        assert_eq!(app.document.file_count(), 1);
        assert!(!app.preview_files_collapsible());

        app.geometry.sidebar = Rect::new(0, 0, 20, 10);
        app.geometry.sidebar_hits = vec![SidebarHitArea {
            area: Rect::new(1, 1, 8, 1),
            target: SidebarHit::PullRequestFiles,
        }];
        let effects = app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 2,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
            now,
        );

        assert!(effects.is_empty(), "the prefetched first file is cached");
        assert_eq!(app.pull_request_file_view, PullRequestFileView::AllFiles);
        assert_eq!(app.document.file_count(), 2);
        assert!(app.preview_files_all_collapsed());
    }

    #[test]
    fn pull_request_prefetch_retries_once_after_a_failure() {
        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request_workspace_generation = Some(10);
        app.pull_request_files = vec![PullRequestFile {
            path: PathBuf::from("src/first.rs"),
            old_path: None,
            status: PullRequestFileStatus::Modified,
            counts: None,
        }];
        app.pull_request_prefetching = true;

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation: 10,
                result: Err("transient failure".to_owned()),
            },
            Instant::now(),
        );
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadPullRequestFileBatch {
                    workspace_generation: 10,
                    paths,
                } if paths == &[PathBuf::from("src/first.rs")]
            )
        ));
        assert!(app.pull_request_prefetch_retrying);

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation: 10,
                result: Err("persistent failure".to_owned()),
            },
            Instant::now(),
        );
        assert!(effects.is_empty());
        assert!(!app.pull_request_prefetching);
        assert!(!app.pull_request_prefetch_retrying);
    }

    #[test]
    fn a_live_poll_refreshes_a_pull_request_without_disturbing_the_reader() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
        app.pull_request_exact_number = Some(8);
        app.pull_request_section = PullRequestSection::Files;
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
        app.pull_request_check_cursor = Some(0);
        app.content_scroll = 40;

        let mut effects = Vec::new();
        app.refresh_pull_request_live(now, false, &mut effects);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command) if matches!(
                command.as_ref(),
                WorkerCommand::LookupPullRequest { number: 8, refresh: true, .. }
            )
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
        )));
        assert!(
            !effects.iter().any(|effect| matches!(
                effect,
                AppEffect::Git(command)
                    if matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. })
            )),
            "a finished run's log never changes, so a poll does not re-read it"
        );
        assert!(
            app.pull_request.is_some(),
            "the loaded pull request stays on screen while the poll runs"
        );
        assert_eq!(app.pull_request_section, PullRequestSection::Files);
        assert_eq!(app.content_scroll, 40);
        assert!(app.pull_request_progress.is_none());
    }

    #[test]
    fn conversation_refresh_replays_when_the_previous_read_finishes() {
        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request = Some(pull_request(8, "Conversation", "acme/widget"));
        let mut first = Vec::new();
        app.request_pull_request_conversation(true, &mut first);
        let generation = app.pull_request_conversation_generation;

        let mut coalesced = Vec::new();
        app.request_pull_request_conversation(true, &mut coalesced);
        assert!(coalesced.is_empty());
        assert!(app.pull_request_conversation_refresh_again);

        let replayed = app.handle_worker_event(
            WorkerEvent::PullRequestConversation {
                generation,
                result: Ok(PullRequestConversation::default()),
            },
            Instant::now(),
        );
        assert!(matches!(
            replayed.as_slice(),
            [AppEffect::Git(command)]
                if matches!(command.as_ref(), WorkerCommand::LoadPullRequestConversation { .. })
        ));
        assert!(app.pull_request_conversation_loading);
    }

    #[test]
    fn parsed_pull_request_documents_evict_oldest_entries_by_size() {
        let mut app = App::new("/tmp/repo", "repo");
        let mut document = indexed_document(&["src/file.rs"]);
        document.title = "x".repeat(1_024);
        let document_size = diff_document_size(&document);

        for name in ["first.rs", "second.rs", "third.rs"] {
            app.cache_pull_request_document(PathBuf::from(name), document.clone());
        }
        app.prune_pull_request_documents(document_size.saturating_mul(2));

        assert_eq!(app.pull_request_documents.len(), 2);
        assert!(
            !app.pull_request_documents
                .contains_key(Path::new("first.rs"))
        );
        assert!(
            app.pull_request_documents
                .contains_key(Path::new("second.rs"))
        );
        assert!(
            app.pull_request_documents
                .contains_key(Path::new("third.rs"))
        );
    }

    #[test]
    fn a_fast_tick_only_speeds_up_the_reads_that_change_that_fast() {
        let mut app = App::new("/tmp/repo", "repo");
        let start = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
        app.pull_request_exact_number = Some(8);
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
        app.pull_request_check_cursor = Some(0);

        let mut first = Vec::new();
        app.refresh_pull_request_live(start, false, &mut first);
        let command_count = |effects: &[AppEffect], name: &str| {
            effects
                .iter()
                .filter(|effect| match effect {
                    AppEffect::Git(command) => match name {
                        "checks" => {
                            matches!(
                                command.as_ref(),
                                WorkerCommand::LoadPullRequestChecks { .. }
                            )
                        }
                        "conversation" => matches!(
                            command.as_ref(),
                            WorkerCommand::LoadPullRequestConversation { .. }
                        ),
                        _ => matches!(command.as_ref(), WorkerCommand::LoadCheckRunLog { .. }),
                    },
                    AppEffect::SetMouseCapture(_) | AppEffect::Open(_) | AppEffect::Quit => false,
                })
                .count()
        };
        assert_eq!(command_count(&first, "checks"), 1);
        assert_eq!(command_count(&first, "conversation"), 1);
        assert_eq!(command_count(&first, "log"), 1);

        app.pull_request_checks_loading = false;
        app.pull_request_conversation_loading = false;
        app.pull_request_check_log_loading = false;
        app.pull_request_loading = false;
        let mut second = Vec::new();
        app.refresh_pull_request_live(start + PULL_REQUEST_ACTIVE_POLL, false, &mut second);

        assert_eq!(command_count(&second, "checks"), 1);
        assert_eq!(
            command_count(&second, "conversation"),
            0,
            "the conversation holds its own floor rather than following the tick"
        );
        assert_eq!(command_count(&second, "log"), 0);

        let forced = app.webhook_delivered(start + PULL_REQUEST_ACTIVE_POLL);
        assert_eq!(command_count(&forced, "conversation"), 1);
        assert_eq!(command_count(&forced, "log"), 1);
    }

    #[test]
    fn steps_are_selected_with_the_same_keys_as_every_other_list() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.focus = Focus::Content;
        app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
        app.pull_request_check_cursor = Some(0);
        let step = |number: usize| crate::git::github::CheckStep {
            number,
            name: format!("step {number}"),
            status: PullRequestCheckStatus::Passed,
            conclusion: "success".to_owned(),
            started_at: String::new(),
            completed_at: String::new(),
            lines: Vec::new(),
        };
        app.pull_request_check_log = Some(CheckRunLog {
            steps: vec![step(1), step(2), step(3)],
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        });
        app.pull_request_step_cursor = 1;

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 2);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 3);
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 3, "the last step is the end");
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 2);
        assert_eq!(
            app.content_scroll, 0,
            "selecting a step never scrolls the pane behind the reader's back"
        );

        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), now);
        assert!(app.check_step_expanded(2));

        app.geometry.content = Rect::new(0, 0, 80, 20);
        app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 2);
        assert!(app.content_scroll > 0);

        app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 3);
        app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), now);
        assert_eq!(app.pull_request_step_cursor, 1);
    }

    #[test]
    fn walking_the_check_list_with_keys_drops_the_log_it_walked_away_from() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.focus = Focus::Sidebar;
        app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
        app.pull_request_checks = vec![
            check("Build every workspace", PullRequestCheckStatus::Passed),
            check("No-comment policy", PullRequestCheckStatus::Passed),
        ];
        app.pull_request_check_cursor = Some(0);
        app.pull_request_check_log_target = Some(app.pull_request_checks[0].identity());
        app.pull_request_check_log = Some(CheckRunLog {
            steps: vec![crate::git::github::CheckStep {
                number: 1,
                name: "Build every workspace".to_owned(),
                status: PullRequestCheckStatus::Passed,
                conclusion: "success".to_owned(),
                started_at: String::new(),
                completed_at: String::new(),
                lines: Vec::new(),
            }],
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        });
        let stale = app.pull_request_check_log_generation;

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE), now);

        assert_eq!(app.pull_request_check_cursor, Some(1));
        assert!(
            app.pull_request_check_log.is_none(),
            "the header moved to another run, so its body cannot still be the old one"
        );
        assert_ne!(
            app.pull_request_check_log_generation, stale,
            "a reply already in flight for the previous run is invalidated"
        );
    }

    #[test]
    fn opening_the_repository_picker_does_not_cancel_a_pull_request_lookup() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;

        let mut effects = Vec::new();
        app.request_pull_request_lookup(42, false, false, &mut effects);
        let lookup = app.pull_request_generation;
        assert!(app.pull_request_loading);

        app.open_pull_request_repositories(&mut Vec::new());
        assert_eq!(
            app.pull_request_generation, lookup,
            "repository discovery answers on its own counter"
        );

        app.handle_worker_event(
            WorkerEvent::PullRequestLookup {
                generation: lookup,
                result: Ok(crate::git::github::PullRequestSnapshot {
                    repositories: Vec::new(),
                    selected_repository: None,
                    pull_request: pull_request(42, "Still wanted", "acme/widget"),
                    warnings: Vec::new(),
                    exact_number: Some(42),
                    from_cache: false,
                }),
            },
            now,
        );

        assert!(
            !app.pull_request_loading,
            "the lookup reply is still accepted, so polling is never blocked"
        );
        assert_eq!(
            app.pull_request.as_ref().map(|request| request.number),
            Some(42)
        );
    }

    #[test]
    fn moving_to_another_check_never_shows_the_previous_run_under_its_name() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Two checks", "acme/widget"));
        app.pull_request_checks = vec![
            check("Build every workspace", PullRequestCheckStatus::Passed),
            check("No-comment policy", PullRequestCheckStatus::Passed),
        ];

        let mut effects = Vec::new();
        app.select_pull_request_check(Some(0), &mut effects);
        assert_eq!(effects.len(), 1);
        let slow = app.pull_request_check_log_generation;
        assert!(app.pull_request_check_log_loading);

        let mut effects = Vec::new();
        app.select_pull_request_check(Some(1), &mut effects);
        assert!(
            matches!(
                effects.as_slice(),
                [AppEffect::Git(command)] if matches!(
                    command.as_ref(),
                    WorkerCommand::LoadCheckRunLog { check, .. }
                        if check.name == "No-comment policy"
                )
            ),
            "the newly selected run is requested rather than waited for"
        );
        assert_ne!(
            app.pull_request_check_log_generation, slow,
            "the in-flight read is invalidated by the move"
        );

        let effects = app.handle_worker_event(
            WorkerEvent::CheckRunLog {
                generation: slow,
                result: Ok(CheckRunLog {
                    steps: vec![crate::git::github::CheckStep {
                        number: 1,
                        name: "Build every workspace".to_owned(),
                        status: PullRequestCheckStatus::Passed,
                        conclusion: "success".to_owned(),
                        started_at: String::new(),
                        completed_at: String::new(),
                        lines: Vec::new(),
                    }],
                    loose_lines: Vec::new(),
                    truncated: false,
                    unavailable: None,
                    log_pending: false,
                }),
            },
            now,
        );

        assert!(effects.is_empty());
        assert!(
            app.pull_request_check_log.is_none(),
            "a reply for the previous check is discarded, not shown under the new one"
        );
        assert!(
            app.pull_request_check_log_loading,
            "the new read is still pending"
        );
    }

    #[test]
    fn a_running_log_follows_its_own_tail_unless_the_reader_scrolled_up() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Running", "acme/widget"));
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
        app.pull_request_check_cursor = Some(0);
        app.pull_request_check_log_generation = 3;
        let log = || CheckRunLog {
            steps: Vec::new(),
            loose_lines: Vec::new(),
            truncated: false,
            unavailable: None,
            log_pending: false,
        };

        app.content_at_bottom = true;
        app.content_scroll = 120;
        app.handle_worker_event(
            WorkerEvent::CheckRunLog {
                generation: 3,
                result: Ok(log()),
            },
            now,
        );
        assert_eq!(
            app.content_scroll,
            usize::MAX,
            "the draw clamps this to the new end"
        );

        app.content_at_bottom = false;
        app.content_scroll = 40;
        app.pull_request_check_log_generation = 4;
        app.handle_worker_event(
            WorkerEvent::CheckRunLog {
                generation: 4,
                result: Ok(log()),
            },
            now,
        );
        assert_eq!(app.content_scroll, 40);

        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
        app.content_at_bottom = true;
        app.content_scroll = 40;
        app.pull_request_check_log_generation = 5;
        app.handle_worker_event(
            WorkerEvent::CheckRunLog {
                generation: 5,
                result: Ok(log()),
            },
            now,
        );
        assert_eq!(app.content_scroll, 40);
    }

    #[test]
    fn a_settled_pull_request_polls_less_often_than_a_running_one() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];
        assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_IDLE_POLL);

        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Pending)];
        assert_eq!(app.pull_request_poll_interval(), PULL_REQUEST_ACTIVE_POLL);

        app.view = View::Changes;
        assert_eq!(
            app.pull_request_poll_interval(),
            PULL_REQUEST_BACKGROUND_POLL,
            "a pull request nobody is looking at still stays fresh, just cheaply"
        );
    }

    #[test]
    fn a_moved_head_reindexes_the_diff_and_keeps_everything_else() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request_generation = 4;
        app.pull_request_loading = true;
        app.pull_request = Some(pull_request(8, "Force pushed", "acme/widget"));
        app.pull_request_section = PullRequestSection::Files;
        app.pull_request_workspace_generation = Some(2);
        app.pull_request_documents
            .insert(PathBuf::from("src/one.rs"), DiffDocument::default());
        app.pull_request_check_cursor = Some(0);
        app.pull_request_checks = vec![check("build", PullRequestCheckStatus::Passed)];

        let mut moved = pull_request(8, "Force pushed", "acme/widget");
        moved.head_oid = "rewritten".to_owned();
        let repository = moved.base_repository.clone();
        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestLookup {
                generation: 4,
                result: Ok(crate::git::github::PullRequestSnapshot {
                    repositories: vec![repository.clone()],
                    selected_repository: Some(repository),
                    pull_request: moved,
                    warnings: Vec::new(),
                    exact_number: Some(8),
                    from_cache: false,
                }),
            },
            now,
        );

        assert!(app.pull_request_workspace_generation.is_none());
        assert!(app.pull_request_documents.is_empty());
        assert_eq!(
            app.pull_request_section,
            PullRequestSection::Files,
            "a force push replaces the diff, not the reader's place in the view"
        );
        assert_eq!(app.pull_request_check_cursor, Some(0));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::PreparePullRequest { .. })
        )));
    }

    #[test]
    fn a_forwarded_webhook_refreshes_immediately_instead_of_waiting_for_the_poll() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(8, "Checks", "acme/widget"));
        app.pull_request_exact_number = Some(8);
        app.pull_request_poll_due = Some(now + Duration::from_secs(3_600));

        let effects = app.webhook_delivered(now);

        assert!(effects.iter().any(|effect| matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::LoadPullRequestChecks { .. })
        )));
        assert!(
            app.pull_request_poll_due
                .is_some_and(|due| due <= now + PULL_REQUEST_IDLE_POLL),
            "the delivery also restarts the poll clock"
        );
    }

    #[test]
    fn history_reset_requested_during_a_load_runs_after_the_in_flight_page() {
        let mut app = App::new("/tmp/repo", "repo");
        app.history_generation = 4;
        app.history_loading = true;
        let mut effects = Vec::new();

        app.request_history(true, &mut effects);
        assert!(effects.is_empty());
        assert!(app.history_refresh_again);

        let effects = app.handle_worker_event(
            WorkerEvent::History {
                generation: 4,
                skip: 0,
                result: Ok(Vec::new()),
            },
            Instant::now(),
        );

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadHistory {
                    generation: 5,
                    skip: 0,
                    limit: HISTORY_PAGE_SIZE,
                    ..
                }
            )
        ));
        assert!(app.history_loading);
        assert!(!app.history_refresh_again);
    }

    #[test]
    fn scheduling_a_new_selection_immediately_invalidates_an_in_flight_preview() {
        let mut app = App::new("/tmp/repo", "repo");
        app.diff_generation = 7;
        app.document = DiffDocument::empty("Current", "keep me");
        app.local_diff_workspace_generation = Some(7);
        app.local_diff_index = Some(DiffIndex {
            title: "Previous commit".to_owned(),
            files: vec![crate::git::diff::DiffFileIndexEntry {
                path: PathBuf::from("stale.rs"),
                old_path: None,
                status: "modified".to_owned(),
                counts: None,
            }],
            truncated: false,
            commit_details: None,
        });

        let now = Instant::now();
        app.schedule_preview(now);
        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE), now);
        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffFile {
                generation: 7,
                workspace_generation: 7,
                path: PathBuf::from("stale.rs"),
                result: Ok(DiffDocument::empty("Stale", "replace me")),
            },
            Instant::now(),
        );

        assert!(effects.is_empty());
        assert_eq!(app.diff_generation, 8);
        assert!(app.local_diff_index.is_none());
        assert!(app.local_diff_workspace_generation.is_none());
        assert_eq!(app.document.title, "Working Tree");
        assert_eq!(app.document.lines[0].text(), "Loading selected changes…");
    }

    #[test]
    fn switching_views_replaces_stale_preview_before_async_work_completes() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
        app.view = View::PullRequests;
        app.document = DiffDocument::empty("PR #6", "stale PR contents");
        app.history.push(Commit {
            id: "a".repeat(40),
            short_id: "aaaaaaa".to_owned(),
            parent_ids: Vec::new(),
            author: String::new(),
            author_email: String::new(),
            authored_at: String::new(),
            committer: String::new(),
            committer_email: String::new(),
            committed_at: String::new(),
            relative_date: String::new(),
            subject: "Selected history commit".to_owned(),
            decorations: Vec::new(),
        });

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);

        assert!(effects.is_empty());
        assert_eq!(app.view, View::History);
        assert_eq!(app.document.title, "aaaaaaa — Selected history commit");
        assert_eq!(app.document.lines[0].text(), "Loading commit preview…");
        assert!(app.preview_due.is_some());
    }

    #[test]
    fn switching_views_invalidates_an_in_flight_pull_request_preview() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.view = View::PullRequests;
        app.pull_request = Some(pull_request(6, "Slow preview", "acme/widget"));
        app.diff_generation = 9;

        app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestDiff {
                generation: 9,
                result: Ok(DiffDocument::empty("PR #6", "stale PR contents")),
            },
            now,
        );

        assert!(effects.is_empty());
        assert_eq!(app.view, View::History);
        assert_eq!(app.document.title, "Commit History");
        assert_eq!(
            app.document.lines[0].text(),
            "No commits in this repository"
        );
        assert_ne!(app.document.lines[0].text(), "stale PR contents");
    }

    #[test]
    fn stale_pull_request_metadata_does_not_replace_the_active_lookup() {
        let mut app = App::new("/tmp/repo", "repo");
        app.pull_request_generation = 2;
        app.pull_request_loading = true;

        let effects = app.handle_worker_event(
            WorkerEvent::PullRequestLookup {
                generation: 1,
                result: Ok(crate::git::github::PullRequestSnapshot {
                    repositories: Vec::new(),
                    selected_repository: None,
                    pull_request: pull_request(1, "Stale", "acme/widget"),
                    warnings: Vec::new(),
                    exact_number: Some(1),
                    from_cache: false,
                }),
            },
            Instant::now(),
        );

        assert!(effects.is_empty());
        assert!(app.pull_request_loading);
        assert!(app.pull_request.is_none());
    }

    #[test]
    fn exact_pull_request_lookup_accepts_only_digits_and_keeps_repository_scope() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::PullRequests;
        let repository = pull_request(1, "One", "acme/widget").base_repository;
        app.github_repositories = vec![repository.clone()];
        app.pull_request_repository = Some(repository.clone());
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), now);
        for character in ['1', '2', 'x'] {
            app.handle_key(
                KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE),
                now,
            );
        }
        app.handle_paste("abc3def");
        assert_eq!(app.pull_request_lookup.value, "123");

        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LookupPullRequest {
                    number: 123,
                    repository: Some(selected),
                    ..
                } if selected.url == repository.url
            )
        ));
        assert!(!app.pull_request_lookup_active);
    }

    #[test]
    fn history_branch_picker_changes_only_the_viewed_revision() {
        let mut app = App::new("/tmp/repo", "repo");
        app.view = View::History;
        app.status.branch.head = "main".to_owned();
        let now = Instant::now();

        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadHistoryBranches { generation: 1 }
            )
        ));
        app.handle_worker_event(
            WorkerEvent::HistoryBranches {
                generation: 1,
                result: Ok(vec![
                    HistoryBranch {
                        name: "main".to_owned(),
                        reference: "refs/heads/main".to_owned(),
                        current: true,
                        remote: false,
                        relative_date: "now".to_owned(),
                        short_id: "aaaaaaa".to_owned(),
                    },
                    HistoryBranch {
                        name: "topic".to_owned(),
                        reference: "refs/heads/topic".to_owned(),
                        current: false,
                        remote: false,
                        relative_date: "now".to_owned(),
                        short_id: "bbbbbbb".to_owned(),
                    },
                ]),
            },
            now,
        );
        let Some(Modal::HistoryBranches { selected, .. }) = app.modal.as_mut() else {
            panic!("expected history branch picker");
        };
        *selected = 1;

        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert_eq!(app.status.branch.head, "main");
        assert_eq!(app.history_branch_label(), "topic");
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::LoadHistory { revision, skip: 0, .. }
                    if revision == "refs/heads/topic"
            )
        ));
    }

    #[test]
    fn collapse_preference_survives_documents_selections_and_views() {
        let mut app = app_with_changes();
        app.document = indexed_document(&["src/main.rs", "README.md"]);
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now);
        assert!(app.files_collapsed);
        app.toggle_preview_file(PathBuf::from("src/main.rs"), &mut Vec::new());
        assert!(
            app.files_collapsed,
            "a one-file override must not reset the preference"
        );
        assert!(!app.preview_file_collapsed("src/main.rs"));

        app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), now);
        assert!(app.files_collapsed);
        assert!(app.expanded_preview_files.is_empty());

        app.handle_key(KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE), now);
        app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE), now);
        assert!(app.files_collapsed);
    }

    #[test]
    fn branch_dialog_renames_the_selected_local_branch() {
        let mut app = App::new("/tmp/repo", "repo");
        app.modal = Some(Modal::Branches {
            items: vec![Branch {
                name: "topic".to_owned(),
                current: false,
                upstream: Some("origin/topic".to_owned()),
                relative_date: "now".to_owned(),
                short_id: "abc1234".to_owned(),
            }],
            selected: 0,
            query: TextBuffer::default(),
            loading: false,
        });

        let effects = app.handle_key(
            KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE),
            Instant::now(),
        );
        assert!(effects.is_empty());
        let Some(Modal::Prompt { input, kind, .. }) = app.modal.as_mut() else {
            panic!("expected rename prompt");
        };
        assert_eq!(input.value, "topic");
        assert!(matches!(kind, PromptKind::RenameBranch { old } if old == "topic"));
        *input = TextBuffer::new("feature/topic");

        let effects = app.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            Instant::now(),
        );
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)]
                if matches!(command.as_ref(), WorkerCommand::Operate {
                    operation: GitOperation::RenameBranch { old, new }, ..
                } if old == "topic" && new == "feature/topic")
        ));
        assert_eq!(app.busy.as_deref(), Some("Renaming branch"));
    }

    #[test]
    fn long_running_git_operations_animate_until_completion() {
        let mut app = App::new("/tmp/repo", "repo");
        let mut effects = Vec::new();
        app.queue_operation(GitOperation::Pull, &mut effects);
        let initial = app.operation_spinner();

        let (_, changed) = app.tick(Instant::now());

        assert!(changed);
        assert_ne!(app.operation_spinner(), initial);
        assert_eq!(app.busy.as_deref(), Some("Pulling changes"));
    }

    #[expect(
        clippy::ref_patterns,
        reason = "matches! can only borrow the value under test"
    )]
    #[test]
    fn compare_branch_picker_queues_a_head_diff_without_checkout() {
        let mut app = app_with_changes();
        app.history_branches_loaded = true;
        app.history_branches = vec![
            HistoryBranch {
                name: "main".to_owned(),
                reference: "refs/heads/main".to_owned(),
                current: true,
                remote: false,
                relative_date: "now".to_owned(),
                short_id: "aaaaaaa".to_owned(),
            },
            HistoryBranch {
                name: "topic".to_owned(),
                reference: "refs/heads/topic".to_owned(),
                current: false,
                remote: false,
                relative_date: "now".to_owned(),
                short_id: "bbbbbbb".to_owned(),
            },
        ];
        let now = Instant::now();

        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), now)
                .is_empty()
        );
        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::PrepareLocalDiff { request, .. }
                    if matches!(request.as_ref(), LocalDiffRequest::Branch { branch, .. } if branch.name == "topic")
            )
        ));
        assert!(matches!(
            app.auxiliary_preview,
            Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
        ));
        assert_eq!(app.focus, Focus::Content);
    }

    #[expect(
        clippy::ref_patterns,
        reason = "matches! can only borrow the value under test"
    )]
    #[test]
    fn background_status_and_collapse_do_not_restart_a_branch_comparison() {
        let mut app = app_with_changes();
        app.history_branches_loaded = true;
        app.history_branches = vec![HistoryBranch {
            name: "topic".to_owned(),
            reference: "refs/heads/topic".to_owned(),
            current: false,
            remote: false,
            relative_date: "now".to_owned(),
            short_id: "bbbbbbb".to_owned(),
        }];
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), now);
        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)]
                if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
        ));
        let index_generation = app.diff_generation;
        let effects = app.handle_worker_event(
            WorkerEvent::LocalDiffIndex {
                generation: index_generation,
                result: Ok(DiffIndex {
                    title: "topic → HEAD — branch comparison".to_owned(),
                    files: ["src/main.rs", "src/lib.rs"]
                        .into_iter()
                        .map(|path| crate::git::diff::DiffFileIndexEntry {
                            path: PathBuf::from(path),
                            old_path: None,
                            status: "modified".to_owned(),
                            counts: None,
                        })
                        .collect(),
                    truncated: false,
                    commit_details: None,
                }),
            },
            now,
        );
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)]
                if matches!(command.as_ref(), WorkerCommand::LoadLocalDiffFile { .. })
        ));
        let stable_generation = app.diff_generation;

        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now)
                .is_empty()
        );
        assert!(
            !app.files_collapsed,
            "first press expands the initial index"
        );
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Char('E'), KeyModifiers::SHIFT), now)
                .is_empty()
        );
        assert!(app.files_collapsed, "second press collapses every file");
        app.focus = Focus::Sidebar;
        assert!(
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now)
                .is_empty()
        );
        assert_eq!(app.diff_generation, stable_generation);
        assert!(matches!(
            app.auxiliary_preview,
            Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
        ));

        app.status_generation = 4;
        let effects = app.handle_worker_event(
            WorkerEvent::Status {
                generation: 4,
                result: Ok(app.status.clone()),
            },
            now + Duration::from_secs(10),
        );

        assert!(effects.iter().all(|effect| !matches!(
            effect,
            AppEffect::Git(command)
                if matches!(command.as_ref(), WorkerCommand::PrepareLocalDiff { .. })
        )));
        assert_eq!(app.diff_generation, stable_generation);
        assert!(matches!(
            app.auxiliary_preview,
            Some(AuxiliaryPreview::Branch(ref branch)) if branch.name == "topic"
        ));
        assert_eq!(app.document.file_count(), 2);
    }

    #[test]
    fn stash_manager_creates_a_named_stash_flow() {
        let mut app = app_with_changes();
        let now = Instant::now();
        let effects = app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT), now);
        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)]
                if matches!(command.as_ref(), WorkerCommand::LoadStashes { .. })
        ));

        app.handle_key(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            now,
        );
        let Some(Modal::Prompt { input, .. }) = app.modal.as_mut() else {
            panic!("expected stash message prompt");
        };
        *input = TextBuffer::new("checkpoint");
        let effects = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert!(matches!(
            effects.as_slice(),
            [AppEffect::Git(command)] if matches!(
                command.as_ref(),
                WorkerCommand::Operate {
                    operation: GitOperation::StashPush {
                        message,
                        include_untracked: false,
                        staged: false,
                    },
                    ..
                } if message == "checkpoint"
            )
        ));
    }

    #[test]
    fn command_palette_can_rename_the_current_branch_but_not_detached_head() {
        let mut app = App::new("/tmp/repo", "repo");
        app.status.branch.head = "main".to_owned();
        let now = Instant::now();

        app.execute_palette(PaletteCommand::RenameCurrentBranch, &mut Vec::new(), now);
        assert!(matches!(
            app.modal,
            Some(Modal::Prompt {
                kind: PromptKind::RenameBranch { old },
                ..
            }) if old == "main"
        ));

        app.modal = None;
        app.status.branch.detached = true;
        app.execute_palette(PaletteCommand::RenameCurrentBranch, &mut Vec::new(), now);
        assert!(app.modal.is_none());
        assert_eq!(app.toast.as_ref().unwrap().level, ToastLevel::Error);
    }

    #[test]
    fn command_palette_navigation_wraps() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.modal = Some(Modal::CommandPalette {
            query: TextBuffer::default(),
            selected: PaletteCommand::ALL.len().saturating_sub(1),
        });

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        assert!(matches!(
            app.modal,
            Some(Modal::CommandPalette { selected: 0, .. })
        ));

        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
        assert!(matches!(
            app.modal,
            Some(Modal::CommandPalette { selected, .. })
                if selected == PaletteCommand::ALL.len().saturating_sub(1)
        ));
    }

    #[test]
    fn list_navigation_wraps_and_handles_empty_lists() {
        assert_eq!(previous_list_index(0, 3), 2);
        assert_eq!(next_list_index(2, 3), 0);
        assert_eq!(previous_list_index(0, 0), 0);
        assert_eq!(next_list_index(0, 0), 0);
    }

    #[test]
    fn command_palette_switches_theme_and_appearance_in_place() {
        let mut app = App::new("/tmp/repo", "repo");
        let now = Instant::now();
        app.set_theme_selection(ThemeName::Catppuccin, AppearanceChoice::Dark);
        let original_background = app.theme.background;

        assert_eq!(
            app.palette_commands("change theme"),
            vec![PaletteCommand::ChangeTheme]
        );
        app.execute_palette(PaletteCommand::ChangeTheme, &mut Vec::new(), now);
        assert!(matches!(
            app.modal,
            Some(Modal::Themes {
                selected: 1,
                original: ThemeName::Catppuccin,
            })
        ));
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);

        assert_eq!(app.theme_name, ThemeName::Monokai);
        assert_ne!(app.theme.background, original_background);
        assert!(matches!(
            app.modal,
            Some(Modal::Themes { selected: 11, .. })
        ));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), now);

        assert_eq!(app.theme_name, ThemeName::Catppuccin);
        assert_eq!(app.theme.background, original_background);
        assert!(app.modal.is_none());

        app.execute_palette(PaletteCommand::ChangeTheme, &mut Vec::new(), now);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), now);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert_eq!(app.theme_name, ThemeName::Dracula);
        assert_eq!(app.appearance_choice, AppearanceChoice::Dark);
        assert_eq!(app.appearance, Appearance::Dark);
        assert_ne!(app.theme.background, original_background);
        assert!(app.modal.is_none());

        app.execute_palette(PaletteCommand::ChangeAppearance, &mut Vec::new(), now);
        assert!(matches!(
            app.modal,
            Some(Modal::Appearances { selected: 2 })
        ));
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);

        assert_eq!(app.theme_name, ThemeName::Dracula);
        assert_eq!(app.appearance_choice, AppearanceChoice::Light);
        assert_eq!(app.appearance, Appearance::Light);
        assert!(app.modal.is_none());
    }
}
