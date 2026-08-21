#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone)]
pub(crate) enum SidebarHit {
    ChangeSection(ChangeSection),
    Change(usize),
    Commit(usize),
    PullRequestFiles,
    PullRequestOverview,
    PullRequestConversation,
    PullRequestChooseRepository,
    PullRequestLookup,
    RecentPullRequest(usize),
    PullRequestDirectory(PathBuf),
    PullRequestFile(usize),
    PullRequestCheckSection(CheckStatusSection),
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

#[derive(Debug, Clone)]
pub(crate) struct ContentReviewHit {
    pub area: Rect,
    pub thread_id: String,
}

pub(crate) struct PullRequestContentRow {
    pub line: Line<'static>,
    pub step: Option<usize>,
    pub wide: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PullRequestContentLink {
    pub row: usize,
    pub start: usize,
    pub width: usize,
    pub target: OpenTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SideBySideRow {
    FileHeader(usize),
    FileFooter,
    Full { index: usize, boxed: bool },
    Split(Option<usize>, Option<usize>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScmAction {
    Stage(usize),
    Unstage(usize),
    Resolve(usize),
    StageSection(ChangeSection),
    UnstageSection(ChangeSection),
    ToggleCheck(usize),
    ToggleCheckSection(ChangeSection),
    Primary,
    RevertChecked,
    ToggleMenu,
    Menu(ScmMenuItem),
    PrPrimary,
    PrToggleMenu,
    PrMenu(PrMenuItem),
    JumpToBottom,
}

#[derive(Debug, Clone)]
pub(crate) struct HelpHit {
    pub area: Rect,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ScmActionHit {
    pub area: Rect,
    pub action: ScmAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepositoryTabAction {
    OpenProject,
    Close,
    CloseOthers,
    CloseAll,
}

impl RepositoryTabAction {
    pub(crate) const ALL: [Self; 4] = [
        Self::OpenProject,
        Self::Close,
        Self::CloseOthers,
        Self::CloseAll,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::OpenProject => "Open Project...",
            Self::Close => "Close",
            Self::CloseOthers => "Close Others",
            Self::CloseAll => "Close All",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepositoryTabHit {
    pub area: Rect,
    pub id: TabId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepositoryTabDrag {
    pub id: TabId,
    pub moved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepositoryTabMenu {
    pub id: TabId,
    pub column: u16,
    pub row: u16,
    pub selected: usize,
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
pub(crate) enum ChangeTarget {
    Section(ChangeSection),
    Change(usize),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct UiGeometry {
    pub repository_tab_hits: Vec<RepositoryTabHit>,
    pub repository_tab_open: Rect,
    pub repository_tab_previous: Rect,
    pub repository_tab_next: Rect,
    pub repository_tab_menu_hits: Vec<(Rect, RepositoryTabAction)>,
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
    pub modal_action_hits: Vec<(Rect, ModalAction)>,
    pub content_file_hits: Vec<ContentFileHit>,
    pub content_step_hits: Vec<ContentStepHit>,
    pub content_review_hits: Vec<ContentReviewHit>,
    pub link_hits: Vec<LinkHit>,
    pub help_hits: Vec<HelpHit>,
    pub project_hits: Vec<Rect>,
}
