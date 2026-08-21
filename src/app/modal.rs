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
pub(crate) enum PrMenuItem {
    Merge(PullRequestMergeMethod),
    Stage,
    AutoMerge,
    DisableAutoMerge,
    Dequeue,
    AdminMerge,
    Review,
    Comments,
    Edit,
    UpdateBranch,
    Lock,
    Unlock,
    Subscribe,
    Unsubscribe,
    AllowMaintainerEdits,
    DisallowMaintainerEdits,
    Revert,
    Close,
    OpenInBrowser,
}

impl PrMenuItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge(method) => method.label(),
            Self::Stage => "Change review stage…",
            Self::AutoMerge => "Enable auto-merge…",
            Self::DisableAutoMerge => "Disable auto-merge",
            Self::Dequeue => "Remove from merge queue",
            Self::AdminMerge => "Merge as administrator…",
            Self::Review => "Submit review…",
            Self::Comments => "Manage comments…",
            Self::Edit => "Edit metadata…",
            Self::UpdateBranch => "Update branch…",
            Self::Lock => "Lock conversation…",
            Self::Unlock => "Unlock conversation",
            Self::Subscribe => "Subscribe",
            Self::Unsubscribe => "Unsubscribe",
            Self::AllowMaintainerEdits => "Allow maintainer edits",
            Self::DisallowMaintainerEdits => "Disallow maintainer edits",
            Self::Revert => "Create revert pull request…",
            Self::Close => "Close pull request",
            Self::OpenInBrowser => "Open in browser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrPrimaryAction {
    Merge(PullRequestMergeMethod),
    Ready,
    Dequeue,
    DisableAutoMerge,
    Reopen,
    OpenInBrowser,
}

impl PrPrimaryAction {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge(method) => method.label(),
            Self::Ready => "Ready for review",
            Self::Dequeue => "Remove from merge queue",
            Self::DisableAutoMerge => "Disable auto-merge",
            Self::Reopen => "Reopen pull request",
            Self::OpenInBrowser => "Open in browser",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullRequestEditField {
    Title,
    Body,
    Base,
    AddAssignee,
    RemoveAssignee,
    AddLabel,
    RemoveLabel,
    AddProject,
    RemoveProject,
    AddReviewer,
    RemoveReviewer,
    Milestone,
    RemoveMilestone,
}

impl PullRequestEditField {
    pub(crate) const ALL: [Self; 13] = [
        Self::Title,
        Self::Body,
        Self::Base,
        Self::AddAssignee,
        Self::RemoveAssignee,
        Self::AddLabel,
        Self::RemoveLabel,
        Self::AddProject,
        Self::RemoveProject,
        Self::AddReviewer,
        Self::RemoveReviewer,
        Self::Milestone,
        Self::RemoveMilestone,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Title => "Edit title",
            Self::Body => "Edit description",
            Self::Base => "Change base branch",
            Self::AddAssignee => "Add assignees",
            Self::RemoveAssignee => "Remove assignees",
            Self::AddLabel => "Add labels",
            Self::RemoveLabel => "Remove labels",
            Self::AddProject => "Add to projects",
            Self::RemoveProject => "Remove from projects",
            Self::AddReviewer => "Request reviewers",
            Self::RemoveReviewer => "Remove review requests",
            Self::Milestone => "Set milestone",
            Self::RemoveMilestone => "Remove milestone",
        }
    }

    pub(crate) fn edit(self, value: String) -> PullRequestEdit {
        match self {
            Self::Title => PullRequestEdit::Title(value),
            Self::Body => PullRequestEdit::Body(value),
            Self::Base => PullRequestEdit::Base(value),
            Self::AddAssignee => PullRequestEdit::AddAssignee(value),
            Self::RemoveAssignee => PullRequestEdit::RemoveAssignee(value),
            Self::AddLabel => PullRequestEdit::AddLabel(value),
            Self::RemoveLabel => PullRequestEdit::RemoveLabel(value),
            Self::AddProject => PullRequestEdit::AddProject(value),
            Self::RemoveProject => PullRequestEdit::RemoveProject(value),
            Self::AddReviewer => PullRequestEdit::AddReviewer(value),
            Self::RemoveReviewer => PullRequestEdit::RemoveReviewer(value),
            Self::Milestone => PullRequestEdit::SetMilestone(value),
            Self::RemoveMilestone => PullRequestEdit::RemoveMilestone,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrActionItem {
    AutoMerge(PullRequestMergeMethod),
    AdminMerge(PullRequestMergeMethod),
    Review(PullRequestReviewKind),
    Comment(PullRequestCommentMode),
    Edit(PullRequestEditField),
    UpdateBranch(PullRequestUpdateMethod),
    Lock(Option<PullRequestLockReason>),
    Revert(bool),
}

impl PrActionItem {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::AutoMerge(method) | Self::AdminMerge(method) => method.label(),
            Self::Review(kind) => kind.label(),
            Self::Comment(PullRequestCommentMode::Create) => "Add comment",
            Self::Comment(PullRequestCommentMode::EditLast) => "Edit my latest comment",
            Self::Comment(PullRequestCommentMode::DeleteLast) => "Delete my latest comment",
            Self::Edit(field) => field.label(),
            Self::UpdateBranch(PullRequestUpdateMethod::Merge) => "Merge base into head",
            Self::UpdateBranch(PullRequestUpdateMethod::Rebase) => "Rebase head onto base",
            Self::Lock(None) => "Lock without a reason",
            Self::Lock(Some(PullRequestLockReason::OffTopic)) => "Lock as off topic",
            Self::Lock(Some(PullRequestLockReason::Resolved)) => "Lock as resolved",
            Self::Lock(Some(PullRequestLockReason::Spam)) => "Lock as spam",
            Self::Lock(Some(PullRequestLockReason::TooHeated)) => "Lock as too heated",
            Self::Revert(false) => "Create ready revert pull request",
            Self::Revert(true) => "Create draft revert pull request",
        }
    }

    pub(crate) const fn needs_input(self) -> bool {
        matches!(
            self,
            Self::Review(PullRequestReviewKind::Comment | PullRequestReviewKind::RequestChanges)
                | Self::Comment(PullRequestCommentMode::Create | PullRequestCommentMode::EditLast)
                | Self::Edit(
                    PullRequestEditField::Title
                        | PullRequestEditField::Body
                        | PullRequestEditField::Base
                        | PullRequestEditField::AddAssignee
                        | PullRequestEditField::RemoveAssignee
                        | PullRequestEditField::AddLabel
                        | PullRequestEditField::RemoveLabel
                        | PullRequestEditField::AddProject
                        | PullRequestEditField::RemoveProject
                        | PullRequestEditField::AddReviewer
                        | PullRequestEditField::RemoveReviewer
                        | PullRequestEditField::Milestone
                )
        )
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
    pub(crate) const ALL: [Self; 27] = [
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
            Self::OpenProject => "Open Project…",
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
