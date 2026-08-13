use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::git::diff::{DiffDocument, DiffLineKind};
use crate::git::history::Commit;
use crate::git::status::{Change, ChangeArea, RepoStatus};
use crate::git::worker::{WorkerCommand, WorkerEvent};
use crate::git::{Branch, ConflictChoice, GitOperation};

const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(45);
const TOAST_DURATION: Duration = Duration::from_secs(4);
const HISTORY_PAGE_SIZE: usize = 300;
const DEFAULT_SIDEBAR_WIDTH: u16 = 42;
const MIN_SIDEBAR_WIDTH: u16 = 22;
const MIN_CONTENT_WIDTH: u16 = 32;
const DEFAULT_DIFF_SPLIT_PERCENT: u16 = 50;
const MIN_DIFF_SPLIT_PERCENT: u16 = 20;
const MAX_DIFF_SPLIT_PERCENT: u16 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Changes,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLayout {
    Unified,
    SideBySide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeTarget {
    Sidebar,
    Diff,
}

#[derive(Debug, Clone, Default)]
pub struct TextBuffer {
    pub value: String,
    pub cursor: usize,
}

impl TextBuffer {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self { value, cursor }
    }

    pub fn insert(&mut self, character: char) {
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub fn insert_str(&mut self, text: &str) {
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or_default();
        self.value.drain(previous..self.cursor);
        self.cursor = previous;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let length = self.value[self.cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or_default();
        self.value.drain(self.cursor..self.cursor + length);
    }

    pub fn move_left(&mut self) {
        self.cursor = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
            .unwrap_or_default();
    }

    pub fn move_right(&mut self) {
        if let Some(character) = self.value[self.cursor..].chars().next() {
            self.cursor += character.len_utf8();
        }
    }

    pub fn home(&mut self) {
        let line_start = self.value[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.cursor = line_start;
    }

    pub fn end(&mut self) {
        self.cursor = self.value[self.cursor..]
            .find('\n')
            .map_or(self.value.len(), |index| self.cursor + index);
    }

    pub fn document_start(&mut self) {
        self.cursor = 0;
    }

    pub fn document_end(&mut self) {
        self.cursor = self.value.len();
    }

    pub fn delete_word_backward(&mut self) {
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
        if start == self.cursor {
            if let Some((index, _)) = previous_character(&self.value, start) {
                start = index;
            }
        }
        self.value.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn delete_word_forward(&mut self) {
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
        if end == self.cursor {
            if let Some((next, _)) = next_character(&self.value, end) {
                end = next;
            }
        }
        self.value.drain(self.cursor..end);
    }

    pub fn delete_to_line_start(&mut self) {
        let start = self.value[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        self.value.drain(start..self.cursor);
        self.cursor = start;
    }

    pub fn delete_to_line_end(&mut self) {
        let end = self.value[self.cursor..]
            .find('\n')
            .map_or(self.value.len(), |index| self.cursor + index);
        self.value.drain(self.cursor..end);
    }

    pub fn move_word_left(&mut self) {
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

    pub fn move_word_right(&mut self) {
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
pub enum PromptKind {
    Filter { previous: String },
    CreateBranch { start: Option<String> },
}

#[derive(Debug, Clone)]
pub enum Modal {
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
    CommandPalette {
        query: TextBuffer,
        selected: usize,
    },
    Conflict {
        change: Change,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteCommand {
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
    StashPop,
    Branches,
    ToggleDiffLayout,
    ShowChanges,
    ShowHistory,
    Help,
    Quit,
}

impl PaletteCommand {
    pub const ALL: [Self; 17] = [
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
        Self::StashPop,
        Self::Branches,
        Self::ToggleDiffLayout,
        Self::ShowChanges,
        Self::ShowHistory,
        Self::Help,
        Self::Quit,
    ];

    pub const fn label(self) -> &'static str {
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
            Self::Stash => "Stash Including Untracked",
            Self::StashPop => "Pop Latest Stash",
            Self::Branches => "Switch Branch…",
            Self::ToggleDiffLayout => "Toggle Unified / Side-by-Side Diff",
            Self::ShowChanges => "Show Changes",
            Self::ShowHistory => "Show Commit History",
            Self::Help => "Keyboard Shortcuts",
            Self::Quit => "Quit Quinjet",
        }
    }
}

#[derive(Debug, Clone)]
pub enum SidebarHit {
    Change(usize),
    Commit(usize),
}

#[derive(Debug, Clone, Default)]
pub struct UiGeometry {
    pub changes_tab: Rect,
    pub history_tab: Rect,
    pub main: Rect,
    pub sidebar: Rect,
    pub sidebar_divider: Rect,
    pub content: Rect,
    pub diff_divider: Option<Rect>,
    pub sidebar_hits: Vec<(u16, SidebarHit)>,
}

#[derive(Debug)]
pub enum AppEffect {
    Git(Box<WorkerCommand>),
    Quit,
}

pub struct App {
    pub repository_root: PathBuf,
    pub repository_name: String,
    pub view: View,
    pub focus: Focus,
    pub diff_layout: DiffLayout,
    pub status: RepoStatus,
    pub history: Vec<Commit>,
    pub document: DiffDocument,
    pub change_cursor: usize,
    pub history_cursor: usize,
    pub sidebar_offset: usize,
    pub content_scroll: usize,
    pub horizontal_scroll: usize,
    pub sidebar_width: u16,
    pub diff_split_percent: u16,
    pub expanded_diff: bool,
    pub resize_target: Option<ResizeTarget>,
    pub filter: String,
    pub modal: Option<Modal>,
    pub toast: Option<Toast>,
    pub busy: Option<String>,
    pub refreshing: bool,
    pub document_loading: bool,
    pub history_loading: bool,
    pub history_complete: bool,
    pub last_refresh: Option<Instant>,
    pub geometry: UiGeometry,
    status_generation: u64,
    diff_generation: u64,
    history_generation: u64,
    branch_generation: u64,
    operation_id: u64,
    refresh_again: bool,
    preview_due: Option<Instant>,
    pending_g: Option<Instant>,
}

impl App {
    pub fn new(root: impl AsRef<Path>, name: impl Into<String>) -> Self {
        Self {
            repository_root: root.as_ref().to_path_buf(),
            repository_name: name.into(),
            view: View::Changes,
            focus: Focus::Sidebar,
            diff_layout: DiffLayout::SideBySide,
            status: RepoStatus::default(),
            history: Vec::new(),
            document: DiffDocument::empty("Working Tree", "Loading changes…"),
            change_cursor: 0,
            history_cursor: 0,
            sidebar_offset: 0,
            content_scroll: 0,
            horizontal_scroll: 0,
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            diff_split_percent: DEFAULT_DIFF_SPLIT_PERCENT,
            expanded_diff: false,
            resize_target: None,
            filter: String::new(),
            modal: None,
            toast: None,
            busy: None,
            refreshing: false,
            document_loading: false,
            history_loading: false,
            history_complete: false,
            last_refresh: None,
            geometry: UiGeometry::default(),
            status_generation: 0,
            diff_generation: 0,
            history_generation: 0,
            branch_generation: 0,
            operation_id: 0,
            refresh_again: false,
            preview_due: None,
            pending_g: None,
        }
    }

    pub fn initial_effects(&mut self) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        self.request_refresh(&mut effects);
        self.request_history(true, &mut effects);
        effects
    }

    pub fn visible_change_indices(&self) -> Vec<usize> {
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

    pub fn visible_commit_indices(&self) -> Vec<usize> {
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

    pub fn selected_change(&self) -> Option<&Change> {
        let visible = self.visible_change_indices();
        visible
            .get(self.change_cursor)
            .and_then(|index| self.status.changes.get(*index))
    }

    pub fn selected_commit(&self) -> Option<&Commit> {
        let visible = self.visible_commit_indices();
        visible
            .get(self.history_cursor)
            .and_then(|index| self.history.get(*index))
    }

    pub fn palette_commands(&self, query: &str) -> Vec<PaletteCommand> {
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

    pub fn filtered_branches(items: &[Branch], query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        items
            .iter()
            .enumerate()
            .filter(|(_, branch)| query.is_empty() || branch.name.to_lowercase().contains(&query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Vec<AppEffect> {
        if let Some(modal) = self.modal.take() {
            return self.handle_modal_key(modal, key, now);
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
                KeyCode::Char('r') => self.request_refresh(&mut effects),
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
            KeyCode::Char('1') => self.switch_view(View::Changes),
            KeyCode::Char('2') => self.switch_view(View::History),
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Sidebar,
                };
            }
            KeyCode::Char('r') => self.request_refresh(&mut effects),
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
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.expanded_diff = !self.expanded_diff;
                self.content_scroll = 0;
                self.request_preview(&mut effects);
            }
            KeyCode::Char('b') => self.open_branches(&mut effects),
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
            KeyCode::Char('s') | KeyCode::Char(' ') if self.view == View::Changes => {
                self.toggle_stage_selected(&mut effects);
            }
            KeyCode::Char('u') if self.view == View::Changes => {
                self.unstage_selected(&mut effects);
            }
            KeyCode::Char('x') if self.view == View::Changes => self.confirm_discard(),
            KeyCode::Char('C') if self.view == View::History => self.confirm_cherry_pick(),
            KeyCode::Char('R') if self.view == View::History => self.confirm_revert(),
            KeyCode::Char('n') if self.view == View::History => self.prompt_branch_at_commit(),
            KeyCode::Char('f') => self.queue_operation(GitOperation::Fetch, &mut effects),
            KeyCode::Char('p') => self.queue_operation(GitOperation::Push, &mut effects),
            KeyCode::Char('l') if self.focus == Focus::Sidebar => {
                self.queue_operation(GitOperation::Pull, &mut effects);
            }
            KeyCode::Char('y') => self.queue_operation(GitOperation::Sync, &mut effects),
            KeyCode::Enter => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Sidebar,
                };
            }
            KeyCode::Esc => {
                if !self.filter.is_empty() {
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
            KeyCode::End => self.go_to_edge(true, now),
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
            KeyCode::Char('G') => self.go_to_edge(true, now),
            KeyCode::Char('[') => self.jump_hunk(false),
            KeyCode::Char(']') => self.jump_hunk(true),
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

    pub fn handle_paste(&mut self, text: &str) {
        match self.modal.as_mut() {
            Some(Modal::Commit { input, .. })
            | Some(Modal::Prompt { input, .. })
            | Some(Modal::CommandPalette { query: input, .. })
            | Some(Modal::Branches { query: input, .. }) => input.insert_str(text),
            _ => {}
        }
        self.apply_live_modal_filter();
    }

    pub fn handle_mouse(&mut self, event: MouseEvent, now: Instant) -> Vec<AppEffect> {
        let effects = Vec::new();
        if self.modal.is_some() {
            return effects;
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self
                    .geometry
                    .sidebar_divider
                    .contains((event.column, event.row).into())
                {
                    self.resize_target = Some(ResizeTarget::Sidebar);
                    self.resize_sidebar(event.column);
                } else if self
                    .geometry
                    .diff_divider
                    .is_some_and(|divider| divider.contains((event.column, event.row).into()))
                {
                    self.resize_target = Some(ResizeTarget::Diff);
                    self.resize_diff(event.column);
                } else if self
                    .geometry
                    .changes_tab
                    .contains((event.column, event.row).into())
                {
                    self.switch_view(View::Changes);
                } else if self
                    .geometry
                    .history_tab
                    .contains((event.column, event.row).into())
                {
                    self.switch_view(View::History);
                } else if self
                    .geometry
                    .sidebar
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Sidebar;
                    if let Some((_, hit)) = self
                        .geometry
                        .sidebar_hits
                        .iter()
                        .find(|(row, _)| *row == event.row)
                        .cloned()
                    {
                        match hit {
                            SidebarHit::Change(index) => {
                                if let Some(cursor) = self
                                    .visible_change_indices()
                                    .iter()
                                    .position(|visible| *visible == index)
                                {
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
                        }
                    }
                } else if self
                    .geometry
                    .content
                    .contains((event.column, event.row).into())
                {
                    self.focus = Focus::Content;
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => match self.resize_target {
                Some(ResizeTarget::Sidebar) => self.resize_sidebar(event.column),
                Some(ResizeTarget::Diff) => self.resize_diff(event.column),
                None => {}
            },
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

    pub fn tick(&mut self, now: Instant) -> (Vec<AppEffect>, bool) {
        let mut changed = false;
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| now >= toast.expires_at)
        {
            self.toast = None;
            changed = true;
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
        (effects, changed)
    }

    pub fn filesystem_changed(&mut self, effects: &mut Vec<AppEffect>) {
        self.request_refresh(effects);
    }

    pub fn periodic_refresh(&mut self, effects: &mut Vec<AppEffect>) {
        self.request_refresh(effects);
    }

    pub fn handle_worker_event(&mut self, event: WorkerEvent, now: Instant) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match event {
            WorkerEvent::Status { generation, result } => {
                if generation != self.status_generation {
                    return effects;
                }
                self.refreshing = false;
                match result {
                    Ok(status) => {
                        let selected = self.selected_change().cloned();
                        let branch_changed = self.status.branch.head != status.branch.head
                            || self.status.branch.oid != status.branch.oid;
                        self.status = status;
                        self.restore_change_selection(selected.as_ref());
                        self.last_refresh = Some(now);
                        if branch_changed && !self.history_loading {
                            self.request_history(true, &mut effects);
                        }
                        if self.view == View::Changes {
                            self.schedule_preview(now);
                        }
                    }
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
                }
                if self.refresh_again {
                    self.refresh_again = false;
                    self.request_refresh(&mut effects);
                }
            }
            WorkerEvent::Diff { generation, result }
            | WorkerEvent::CommitDetail { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
                self.document_loading = false;
                match result {
                    Ok(document) => {
                        self.document = document;
                        self.content_scroll = 0;
                        self.horizontal_scroll = 0;
                    }
                    Err(error) => {
                        self.document = DiffDocument::empty("Preview Error", error.clone());
                        self.show_toast(error, ToastLevel::Error, now);
                    }
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
                    Err(error) => self.show_toast(error, ToastLevel::Error, now),
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

    fn handle_modal_key(
        &mut self,
        mut modal: Modal,
        key: KeyEvent,
        now: Instant,
    ) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        match &mut modal {
            Modal::Help { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {}
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
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.queue_operation(operation.clone(), &mut effects);
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {}
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
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(visible.len().saturating_sub(1));
                    }
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
            Modal::CommandPalette { query, selected } => {
                if key.code == KeyCode::Esc {
                    return effects;
                }
                let commands = self.palette_commands(&query.value);
                match key.code {
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => {
                        *selected = (*selected + 1).min(commands.len().saturating_sub(1));
                    }
                    KeyCode::Enter => {
                        if let Some(command) = commands.get(*selected).copied() {
                            self.execute_palette(command, &mut effects);
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
        }
        effects
    }

    fn execute_palette(&mut self, command: PaletteCommand, effects: &mut Vec<AppEffect>) {
        match command {
            PaletteCommand::Refresh => self.request_refresh(effects),
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
            PaletteCommand::Stash => self.queue_operation(GitOperation::Stash, effects),
            PaletteCommand::StashPop => self.queue_operation(GitOperation::StashPop, effects),
            PaletteCommand::Branches => self.open_branches(effects),
            PaletteCommand::ToggleDiffLayout => self.toggle_diff_layout(),
            PaletteCommand::ShowChanges => self.switch_view(View::Changes),
            PaletteCommand::ShowHistory => self.switch_view(View::History),
            PaletteCommand::Help => self.modal = Some(Modal::Help { scroll: 0 }),
            PaletteCommand::Quit => effects.push(AppEffect::Quit),
        }
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

    fn resize_diff(&mut self, column: u16) {
        let content = self.geometry.content;
        if content.width == 0 {
            return;
        }
        let relative = column.saturating_sub(content.x).min(content.width);
        self.diff_split_percent = (relative.saturating_mul(100) / content.width)
            .clamp(MIN_DIFF_SPLIT_PERCENT, MAX_DIFF_SPLIT_PERCENT);
    }

    fn switch_view(&mut self, view: View) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.focus = Focus::Sidebar;
        self.sidebar_offset = 0;
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
        self.schedule_preview(Instant::now());
    }

    fn toggle_diff_layout(&mut self) {
        self.diff_layout = match self.diff_layout {
            DiffLayout::Unified => DiffLayout::SideBySide,
            DiffLayout::SideBySide => DiffLayout::Unified,
        };
        self.content_scroll = 0;
        self.horizontal_scroll = 0;
    }

    fn navigate(&mut self, amount: isize, now: Instant) {
        if self.focus == Focus::Content {
            if amount < 0 {
                self.content_scroll = self.content_scroll.saturating_sub(amount.unsigned_abs());
            } else {
                self.content_scroll = self.content_scroll.saturating_add(amount as usize);
            }
            return;
        }

        let length = match self.view {
            View::Changes => self.visible_change_indices().len(),
            View::History => self.visible_commit_indices().len(),
        };
        let cursor = match self.view {
            View::Changes => &mut self.change_cursor,
            View::History => &mut self.history_cursor,
        };
        if length == 0 {
            *cursor = 0;
            return;
        }
        *cursor = if amount < 0 {
            cursor.saturating_sub(amount.unsigned_abs())
        } else {
            (*cursor + amount as usize).min(length - 1)
        };
        self.schedule_preview(now);
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
            let amount = self.geometry.sidebar.height.saturating_sub(4).max(1) as isize;
            self.navigate(direction * amount, now);
        }
    }

    fn go_to_edge(&mut self, end: bool, now: Instant) {
        if self.focus == Focus::Content {
            self.content_scroll = if end {
                self.document.lines.len().saturating_sub(1)
            } else {
                0
            };
            return;
        }
        match self.view {
            View::Changes => {
                let length = self.visible_change_indices().len();
                self.change_cursor = if end { length.saturating_sub(1) } else { 0 };
            }
            View::History => {
                let length = self.visible_commit_indices().len();
                self.history_cursor = if end { length.saturating_sub(1) } else { 0 };
            }
        }
        self.schedule_preview(now);
    }

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

    fn toggle_stage_selected(&mut self, effects: &mut Vec<AppEffect>) {
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
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
        if change.area == ChangeArea::Staged {
            self.queue_operation(GitOperation::Unstage(vec![change.path]), effects);
        }
    }

    fn confirm_discard(&mut self) {
        let Some(change) = self.selected_change().cloned() else {
            return;
        };
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

    fn queue_operation(&mut self, operation: GitOperation, effects: &mut Vec<AppEffect>) {
        if self.busy.is_some() {
            return;
        }
        self.operation_id = self.operation_id.wrapping_add(1);
        self.busy = Some(operation.label().to_owned());
        effects.push(AppEffect::Git(Box::new(WorkerCommand::Operate {
            id: self.operation_id,
            operation,
        })));
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

    fn request_history(&mut self, reset: bool, effects: &mut Vec<AppEffect>) {
        if self.history_loading {
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
            skip,
            limit: HISTORY_PAGE_SIZE,
        })));
    }

    fn request_preview(&mut self, effects: &mut Vec<AppEffect>) {
        self.diff_generation = self.diff_generation.wrapping_add(1);
        let generation = self.diff_generation;
        match self.view {
            View::Changes => {
                if let Some(change) = self.selected_change().cloned() {
                    self.document_loading = true;
                    effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadDiff {
                        generation,
                        change,
                        expanded: self.expanded_diff,
                    })));
                } else {
                    self.document_loading = false;
                    self.document = DiffDocument::empty(
                        "Working Tree",
                        if self.status.changes.is_empty() {
                            "Working tree clean — no changes"
                        } else {
                            "No changes match the current filter"
                        },
                    );
                }
            }
            View::History => {
                if let Some(commit) = self.selected_commit().cloned() {
                    self.document_loading = true;
                    effects.push(AppEffect::Git(Box::new(WorkerCommand::LoadCommit {
                        generation,
                        commit: Box::new(commit),
                    })));
                    let visible_len = self.visible_commit_indices().len();
                    if self.history_cursor + 20 >= visible_len
                        && !self.history_loading
                        && !self.history_complete
                        && self.filter.is_empty()
                    {
                        self.request_history(false, effects);
                    }
                } else {
                    self.document_loading = false;
                    self.document = DiffDocument::empty(
                        "Commit History",
                        if self.history.is_empty() {
                            "No commits in this repository"
                        } else {
                            "No commits match the current filter"
                        },
                    );
                }
            }
        }
    }

    fn schedule_preview(&mut self, now: Instant) {
        self.preview_due = Some(now + PREVIEW_DEBOUNCE);
    }

    fn normalize_selection(&mut self) {
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
        self.change_cursor = selected
            .and_then(|selected| {
                visible.iter().position(|index| {
                    self.status.changes.get(*index).is_some_and(|change| {
                        change.path == selected.path && change.area == selected.area
                    })
                })
            })
            .unwrap_or_else(|| self.change_cursor.min(visible.len().saturating_sub(1)));
    }

    fn show_toast(&mut self, message: String, level: ToastLevel, now: Instant) {
        self.toast = Some(Toast {
            message,
            level,
            expires_at: now + TOAST_DURATION,
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::status::{BranchState, ChangeStatus};

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
        app
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
    fn enter_toggles_focus_between_sidebar_and_content() {
        let mut app = app_with_changes();
        let now = Instant::now();

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert_eq!(app.focus, Focus::Content);
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now);
        assert_eq!(app.focus, Focus::Sidebar);
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
}
