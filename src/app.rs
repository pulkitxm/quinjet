use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ffi::OsString;
use std::fmt::Write;
use std::hash::Hash;
use std::mem::size_of_val;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::convert::{count, offset};
use crate::git::diff::{DiffDocument, DiffIndex, DiffLineCounts, DiffLineKind, PullRequestDetails};
use crate::git::github::{
    CheckRunLog, GitHubRepository, PullRequest, PullRequestCheck, PullRequestCheckStatus,
    PullRequestCommentMode, PullRequestConversation, PullRequestDiffIndex, PullRequestEdit,
    PullRequestFile, PullRequestFileStatus, PullRequestLockReason, PullRequestMergeMethod,
    PullRequestMergeMode, PullRequestOperation, PullRequestProgress, PullRequestReviewDecision,
    PullRequestReviewKind, PullRequestReviewOperation, PullRequestReviewSide,
    PullRequestReviewSnapshot, PullRequestReviewThread, PullRequestReviewThreadSubject,
    PullRequestStack, PullRequestStackMember, PullRequestUpdateMethod, RecentPullRequest,
};
use crate::git::history::Commit;
use crate::git::status::{Change, ChangeArea, ChangeStatus, RepoStatus};
use crate::git::worker::{WorkerCommand, WorkerEvent};
use crate::git::{
    Branch, ConflictChoice, GitOperation, HistoryBranch, LocalDiffRequest, ProjectGroup, Stash,
    Worktree,
};
use crate::integration::{Client, HostAction};
use crate::ssh::SshContext;
use crate::tabs::{TabId, TabInfo};
use crate::theme::{Appearance, AppearanceChoice, Theme, ThemeName, ThemeSelection};

const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(45);
const RESIZE_DOUBLE_TAP_INTERVAL: Duration = Duration::from_millis(450);
const TOAST_DURATION: Duration = Duration::from_secs(4);
const HISTORY_PAGE_SIZE: usize = 300;
const PULL_REQUEST_PREFETCH_BATCH: usize = 32;
const PULL_REQUEST_PREFETCH_BYTE_BUDGET: usize = 6 * 1024 * 1024;
const PULL_REQUEST_PATCH_FALLBACK_ESTIMATE: usize = 512 * 1024;
const PULL_REQUEST_PATCH_LINE_ESTIMATE: usize = 80;
const MAX_PREFETCHED_PULL_REQUEST_FILES: usize = 4_096;
const MAX_PULL_REQUEST_DOCUMENT_BYTES: usize = 32 * 1024 * 1024;
#[doc = " Poll cadences for an open pull request. A run in progress changes state in"]
#[doc = " seconds and is worth watching closely; a settled pull request only needs to"]
#[doc = " notice new comments; a pull request nobody is looking at needs less again."]
const PULL_REQUEST_ACTIVE_POLL: Duration = Duration::from_secs(5);
const PULL_REQUEST_IDLE_POLL: Duration = Duration::from_secs(20);
const PULL_REQUEST_BACKGROUND_POLL: Duration = Duration::from_secs(120);
#[doc = " Each live stream costs its own GitHub requests, so the tick cadence is a"]
#[doc = " ceiling rather than a schedule: check state is the only thing worth reading"]
#[doc = " as often as the tick fires. Metadata, the conversation and a growing log all"]
#[doc = " change on human or build timescales and hold their own floor."]
const PULL_REQUEST_DETAIL_POLL: Duration = Duration::from_secs(20);
#[doc = " A running job's log grows continuously, so this is a tail interval rather"]
#[doc = " than a staleness bound."]
const PULL_REQUEST_LOG_POLL: Duration = Duration::from_secs(8);
const MAX_PULL_REQUEST_NUMBER_DIGITS: usize = 20;
const DEFAULT_SIDEBAR_WIDTH: u16 = 42;
const MIN_SIDEBAR_WIDTH: u16 = 22;
const MIN_CONTENT_WIDTH: u16 = 32;
const DEFAULT_DIFF_SPLIT_PERCENT: u16 = 50;
const MIN_DIFF_SPLIT_PERCENT: u16 = 20;
const MAX_DIFF_SPLIT_PERCENT: u16 = 80;
const OPERATION_SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];

const fn initial_recent_pull_requests() -> Vec<RecentPullRequest> {
    Vec::new()
}

fn recent_pull_requests_for(
    recent: Vec<RecentPullRequest>,
    repository: &GitHubRepository,
) -> Vec<RecentPullRequest> {
    recent
        .into_iter()
        .filter(|entry| entry.repository.url.eq_ignore_ascii_case(&repository.url))
        .collect()
}

fn toggle_membership<T: Eq + Hash>(items: &mut HashSet<T>, item: T) {
    if !items.remove(&item) {
        items.extend([item]);
    }
}

#[cfg(not(test))]
fn cached_recent_pull_requests_for(repository: &GitHubRepository) -> Vec<RecentPullRequest> {
    recent_pull_requests_for(crate::git::github::recent_pull_requests(), repository)
}

#[cfg(test)]
const fn cached_recent_pull_requests_for(_repository: &GitHubRepository) -> Vec<RecentPullRequest> {
    Vec::new()
}

#[cfg(not(test))]
fn updated_recent_pull_requests(
    _existing: &[RecentPullRequest],
    pull_request: &PullRequest,
) -> Vec<RecentPullRequest> {
    recent_pull_requests_for(
        crate::git::github::record_recent_pull_request(pull_request),
        &pull_request.base_repository,
    )
}

#[cfg(test)]
fn updated_recent_pull_requests(
    existing: &[RecentPullRequest],
    pull_request: &PullRequest,
) -> Vec<RecentPullRequest> {
    let current = RecentPullRequest::from(pull_request);
    let mut recent = existing.to_vec();
    recent.retain(|entry| {
        entry
            .repository
            .url
            .eq_ignore_ascii_case(&current.repository.url)
            && entry.number != current.number
    });
    recent.insert(0, current);
    recent_pull_requests_for(recent, &pull_request.base_repository)
}

mod changes;
mod dialogs;
mod geometry;
mod initialization;
mod interaction;
mod keyboard;
mod keyboard_stack;
mod links;
mod live;
mod local_diff;
mod modal;
mod modal_actions;
mod modal_events;
mod modal_forms;
mod modal_help;
mod modal_pickers;
mod modal_review;
mod mouse;
mod mouse_sidebar;
mod operations;
mod palette;
mod projects;
mod pull_request_actions;
mod pull_request_checks;
mod pull_request_diff;
mod pull_request_review;
mod pull_request_stack;
mod refresh;
mod repository_tabs;
mod scm;
mod selection;
mod stack_inspector;
mod support;
mod view;
mod view_state;
mod worker_content;
mod worker_events;
mod worker_repository;
mod worker_stack;

pub(crate) use geometry::*;
pub(crate) use modal::*;
pub(crate) use projects::{ProjectOpenMode, ProjectRow};
pub(crate) use pull_request_actions::*;
pub(crate) use stack_inspector::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use support::*;
pub(crate) use view::*;
pub(crate) use view_state::*;

#[derive(Debug)]
pub(crate) enum AppEffect {
    Git(Box<WorkerCommand>),
    Copy(String),
    SetMouseCapture(bool),
    Host(HostAction),
    Open(OpenTarget),
    SwitchRepository(PathBuf),
    OpenRepositoryTabPicker,
    OpenRepositoryTab(PathBuf),
    CancelRepositoryTabPicker,
    SwitchSshMachine(crate::ssh::SshSwitch),
    ActivateRepositoryTab(TabId),
    ReorderRepositoryTab { source: TabId, target: TabId },
    CloseRepositoryTab(TabId),
    CloseOtherRepositoryTabs(TabId),
    CloseAllRepositoryTabs,
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
    pub view_states: ViewStates,
    pub focus: Focus,
    pub diff_layout: DiffLayout,
    pub theme: Theme,
    pub theme_selection: ThemeSelection,
    pub appearance_choice: AppearanceChoice,
    pub appearance: Appearance,
    pub status: RepoStatus,
    pub history: Vec<Commit>,
    pub worktrees: Vec<Worktree>,
    pub project_groups: Vec<ProjectGroup>,
    pub collapsed_project_groups: HashSet<PathBuf>,
    pub ssh_context: Option<SshContext>,
    pub history_branch: Option<HistoryBranch>,
    pub pull_request: Option<PullRequest>,
    pub github_repositories: Vec<GitHubRepository>,
    pub local_github_repository: Option<GitHubRepository>,
    pub pull_request_repository: Option<GitHubRepository>,
    pub pull_request_warnings: Vec<String>,
    #[doc = " Why the last lookup failed. The pull-request pane renders app state"]
    #[doc = " rather than a document, so a failure needs somewhere to live that"]
    #[doc = " outlasts the toast announcing it."]
    pub pull_request_error: Option<String>,
    pub pull_request_exact_number: Option<u64>,
    pub pull_request_from_cache: bool,
    pub pull_request_stack: Option<PullRequestStack>,
    pub pull_request_stack_loading: bool,
    pub pull_request_stack_error: Option<String>,
    pub pull_request_lookup_refresh: bool,
    pub pull_request_stack_anchor: Option<usize>,
    pub pull_request_stack_cursor: Option<usize>,
    pub stack_inspector: StackInspector,
    pub stack_inspector_content_rows: Vec<PullRequestContentRow>,
    pub stack_inspector_content_rows_key: Option<(StackMemberSection, usize, u64, i64)>,
    pub stack_inspector_content_width: usize,
    pub stack_inspector_content_links: Vec<PullRequestContentLink>,
    pub history_branches: Vec<HistoryBranch>,
    pub history_branches_loading: bool,
    pub history_branches_loaded: bool,
    pub pull_request_lookup: TextBuffer,
    pub pull_request_lookup_active: bool,
    pub recent_pull_requests: Vec<RecentPullRequest>,
    pub recent_pull_request_cursor: usize,
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
    #[doc = " `None` keeps the content pane on the pull request itself; selecting a"]
    #[doc = " check replaces it with that run's steps and log."]
    pub pull_request_check_cursor: Option<usize>,
    pub selected_check_section: Option<CheckStatusSection>,
    pub collapsed_check_sections: HashSet<CheckStatusSection>,
    pub pull_request_checks_loading: bool,
    pub pull_request_checks_error: Option<String>,
    pub pull_request_checks_from_cache: bool,
    pub pull_request_prefetched_logs: HashSet<String>,
    pub pull_request_conversation: PullRequestConversation,
    pub pull_request_conversation_loading: bool,
    pub pull_request_conversation_refresh_again: bool,
    pub pull_request_conversation_error: Option<String>,
    pub pull_request_review: PullRequestReviewSnapshot,
    pub pull_request_review_loading: bool,
    pub pull_request_review_mutating: bool,
    pub pull_request_review_error: Option<String>,
    pub pull_request_review_cursor: Option<PullRequestReviewAnchor>,
    pub pull_request_review_line_threads: HashMap<usize, String>,
    pub pull_request_check_log: Option<CheckRunLog>,
    pub pull_request_check_log_loading: bool,
    pub pull_request_check_log_error: Option<String>,
    pub expanded_check_steps: HashSet<usize>,
    pub pull_request_step_cursor: usize,
    #[doc = " Set when the step selection moves, and cleared by the draw that acts on"]
    #[doc = " it. Scrolling the selection into view on every frame instead would pin"]
    #[doc = " the pane to the selected step and make its own output unreadable."]
    pub pull_request_step_reveal: bool,
    pub pull_request_content_rows: Vec<PullRequestContentRow>,
    pub pull_request_content_rows_key: Option<(bool, usize, u64, i64)>,
    pub pull_request_content_width: usize,
    pub pull_request_content_links: Vec<PullRequestContentLink>,
    pub pull_request_content_generation: u64,
    #[doc = " The relative-time bucket the rendered rows were built in. The draw reads"]
    #[doc = " it instead of the clock, so a frame is a function of state alone and only"]
    #[doc = " a tick can age the timestamps out of their cache."]
    pub relative_time_generation: i64,
    #[doc = " Whether the last draw left the content pane scrolled to its end. The"]
    #[doc = " renderer owns the row count, so it reports this back for the one decision"]
    #[doc = " that needs it: whether a growing log should keep following."]
    pub content_at_bottom: bool,
    pub pull_request_progress: Option<PullRequestProgress>,
    pub auxiliary_preview: Option<AuxiliaryPreview>,
    pub document: DiffDocument,
    pub document_layout_generation: u64,
    pub unified_diff_rows: Vec<usize>,
    pub side_by_side_diff_rows: Vec<SideBySideRow>,
    pub diff_rows_key: Option<(u64, bool)>,
    pub selected_change_section: Option<ChangeSection>,
    pub collapsed_change_sections: HashSet<ChangeSection>,
    pub checked_change_paths: HashSet<PathBuf>,
    pub scm_menu_open: bool,
    pub scm_menu_selected: usize,
    pub pr_menu_open: bool,
    pub pr_menu_selected: usize,
    pub preferred_merge_method: PullRequestMergeMethod,
    pub selected_preview_file: Option<PathBuf>,
    pub preview_file_cursor: usize,
    pub collapsed_preview_files: HashSet<PathBuf>,
    pub expanded_preview_files: HashSet<PathBuf>,
    #[doc = " The file whose header the next draw should park at the top of the content"]
    #[doc = " pane. Only the renderer knows how many rows a folded document occupies, so"]
    #[doc = " collapsing, expanding and file navigation name the file and let the draw"]
    #[doc = " turn it into a scroll offset."]
    pub content_file_anchor: Option<PathBuf>,
    pub change_cursor: usize,
    pub history_cursor: usize,
    pub sidebar_offset: usize,
    pub sidebar_free_scroll: bool,
    pub sidebar_last_cursor: Option<usize>,
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
    pub modal_scroll: usize,
    pub modal_free_scroll: bool,
    pub toast: Option<Toast>,
    pub mouse_capture: bool,
    pub mouse_capture_preference: bool,
    pub link_hover: Option<(u16, u16)>,
    pub text_selection: Option<TextSelection>,
    pub rendered_cells: Vec<Vec<char>>,
    pub webhooks_listening: bool,
    pub host_client: Option<Client>,
    pub busy: Option<String>,
    pub operation_frame: usize,
    pub refreshing: bool,
    pub document_loading: bool,
    pub history_loading: bool,
    pub history_complete: bool,
    pub pull_request_loading: bool,
    pub last_refresh: Option<Instant>,
    pub geometry: UiGeometry,
    pub repository_tabs: Vec<TabInfo>,
    pub repository_tab_drag: Option<RepositoryTabDrag>,
    pub repository_tab_menu: Option<RepositoryTabMenu>,
    pub tab_active: bool,
    pub status_generation: u64,
    pub changes_diff_version: u64,
    pub diff_generation: u64,
    pub history_generation: u64,
    pub pull_request_generation: u64,
    #[doc = " Repository discovery answers on its own counter. Sharing the lookup's"]
    #[doc = " would let opening the picker discard a pull request already on its way,"]
    #[doc = " leaving its loading flag set with no reply ever able to clear it."]
    pub repository_generation: u64,
    pub pull_request_workspace_generation: Option<u64>,
    pub pull_request_diff_source: Option<PullRequestDiffSource>,
    pub pull_request_documents: HashMap<PathBuf, DiffDocument>,
    pub pull_request_document_order: VecDeque<PathBuf>,
    pub pull_request_document_bytes: usize,
    pub pull_request_prefetched_paths: HashSet<PathBuf>,
    pub pull_request_loading_path: Option<PathBuf>,
    #[doc = " The path whose patch currently occupies `document` in single-file view."]
    #[doc = " Tracking it explicitly keeps the cache authoritative about which files"]
    #[doc = " already have a patch, wherever that patch happens to be held."]
    pub pull_request_single_file: Option<PathBuf>,
    pub pull_request_prefetching: bool,
    pub pull_request_prefetch_retrying: bool,
    pub pull_request_checks_generation: u64,
    pub pull_request_conversation_generation: u64,
    pub pull_request_review_generation: u64,
    pub pull_request_check_log_generation: u64,
    pub pull_request_check_log_target: Option<String>,
    pub local_diff_request: Option<LocalDiffRequest>,
    pub local_diff_change_section: Option<ChangeSection>,
    pub local_diff_preserving_document: bool,
    pub local_diff_preserved_paths: HashSet<PathBuf>,
    pub local_diff_workspace_generation: Option<u64>,
    pub local_diff_index: Option<DiffIndex>,
    pub local_diff_documents: HashMap<PathBuf, DiffDocument>,
    pub local_diff_loading_path: Option<PathBuf>,
    pub local_diff_pending_paths: VecDeque<PathBuf>,
    pub local_diff_single_loaded: bool,
    pub branch_generation: u64,
    pub history_branch_generation: u64,
    pub stash_generation: u64,
    pub worktree_generation: u64,
    pub project_generation: u64,
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

#[cfg(test)]
#[expect(
    unused_results,
    reason = "test helpers return values the assertions do not use"
)]
mod tests;
