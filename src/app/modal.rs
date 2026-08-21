#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

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

    #[doc = " The text on each side of the cursor, never panicking when the cursor"]
    #[doc = " sits between two bytes of the same character."]
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
        paths: Vec<PathBuf>,
    },
    PullRequest {
        pull_request: Box<PullRequest>,
        action: PrActionItem,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum ConfirmAction {
    Operate(GitOperation),
    OpenPrompt {
        title: String,
        kind: PromptKind,
    },
    PullRequest {
        pull_request: Box<PullRequest>,
        operation: PullRequestOperation,
    },
    PullRequestReview(PullRequestReviewOperation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScmMenuItem {
    StageAll,
    UnstageAll,
    DiscardChecked,
    RemoveChecked,
    DiscardSelected,
    RemoveSelected,
    DiscardUnstaged,
    DiscardAll,
    CompareBranch,
    ManageStashes,
    StashAll,
    StashIncludeUntracked,
    StashStagedOnly,
}

impl ScmMenuItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::StageAll => "Stage All",
            Self::UnstageAll => "Unstage All",
            Self::DiscardChecked => "Revert Checked Files",
            Self::RemoveChecked => "Remove Checked Files",
            Self::DiscardSelected => "Revert Selected File",
            Self::RemoveSelected => "Remove Selected File",
            Self::DiscardUnstaged => "Revert Unstaged Changes",
            Self::DiscardAll => "Revert All Changes",
            Self::CompareBranch => "Compare Branch",
            Self::ManageStashes => "Manage Stashes",
            Self::StashAll => "Stash All Changes",
            Self::StashIncludeUntracked => "Stash Including Untracked",
            Self::StashStagedOnly => "Stash Staged Only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalAction {
    CommitSubmit,
    CommitCancel,
    CommitToggleAmend,
    ConfirmYes,
    ConfirmNo,
    PullRequestAction(usize),
    PullRequestReviewThreadAction(usize),
}

#[derive(Debug, Clone)]
pub(crate) enum Modal {
    Help {
        selected: usize,
        scroll: usize,
        hover: Option<usize>,
    },
    Commit {
        input: TextBuffer,
        amend: bool,
    },
    PullRequestReviewComment {
        input: TextBuffer,
        target: PullRequestReviewTarget,
    },
    PullRequestReviewThreadActions {
        items: Vec<PullRequestReviewThreadAction>,
        selected: usize,
    },
    PullRequestReviewSubmit {
        input: TextBuffer,
        decision: PullRequestReviewDecision,
    },
    Prompt {
        title: String,
        input: TextBuffer,
        kind: PromptKind,
    },
    PullRequestActions {
        title: String,
        items: Vec<PrActionItem>,
        selected: usize,
    },
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
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
    Projects {
        groups: Vec<ProjectGroup>,
        selected: usize,
        query: TextBuffer,
        loading: bool,
        mode: ProjectOpenMode,
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
        original_choice: AppearanceChoice,
        original_appearance: Appearance,
    },
    Conflict {
        change: Change,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProjectOpenMode {
    CurrentTab,
    NewTab,
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
    OpenProject,
    OpenProjectNewTab,
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
    pub(crate) const ALL: [Self; 28] = [
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
        Self::OpenProject,
        Self::OpenProjectNewTab,
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
            Self::OpenProject => "Switch Project in Current Tab…",
            Self::OpenProjectNewTab => "Open Project in New Tab…",
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
