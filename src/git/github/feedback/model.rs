#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What kind of feedback a row is. Ordered by how directly it stands"]
#[doc = " between the pull request and merging, so the queue's order needs no"]
#[doc = " separate sort key."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FeedbackKind {
    #[doc = " A reviewer asked for changes."]
    ChangesRequested,
    #[doc = " A check reported a failure on a line."]
    Failure,
    #[doc = " An unresolved review thread on code that still exists."]
    Thread,
    #[doc = " An unresolved review thread a later commit made outdated."]
    OutdatedThread,
    #[doc = " A check reported a warning or a notice on a line."]
    Advisory,
}

impl FeedbackKind {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::ChangesRequested => "changes",
            Self::Failure => "failure",
            Self::Thread => "thread",
            Self::OutdatedThread => "outdated",
            Self::Advisory => "advisory",
        }
    }

    #[doc = " Whether this row is something the merge is actually waiting on, as"]
    #[doc = " opposed to something worth reading."]
    pub(crate) const fn is_blocking(self) -> bool {
        matches!(self, Self::ChangesRequested | Self::Failure | Self::Thread)
    }
}

#[doc = " Who the row is waiting on. This is the field that makes one queue"]
#[doc = " useful to both an author and a reviewer."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FeedbackOwner {
    #[doc = " The newest word is somebody else's, so it is yours to answer."]
    You,
    #[doc = " The newest word is yours, so it is waiting on somebody else."]
    Others,
    #[doc = " Nobody has spoken: a check finding rather than a conversation."]
    Nobody,
}

impl FeedbackOwner {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Others => "others",
            Self::Nobody => "-",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedbackItem {
    pub kind: FeedbackKind,
    #[doc = " The review thread id, the check run id, or the reviewer's login,"]
    #[doc = " depending on the kind. Stable enough to act on."]
    pub id: String,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub author: String,
    pub summary: String,
    pub body: String,
    pub url: String,
    pub owner: FeedbackOwner,
    #[doc = " You wrote the newest word on this row."]
    pub mine: bool,
    #[doc = " What resolves it, spelled out so a caller does not have to know the"]
    #[doc = " verb map."]
    pub action: String,
}

impl FeedbackItem {
    pub(crate) fn location(&self) -> String {
        let Some(path) = &self.path else {
            return String::new();
        };
        self.line.map_or_else(
            || path.display().to_string(),
            |line| format!("{}:{line}", path.display()),
        )
    }

    fn order(&self) -> (FeedbackKind, String, usize, String) {
        (
            self.kind,
            self.path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            self.line.unwrap_or_default(),
            self.id.clone(),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FeedbackCounts {
    pub blocking: usize,
    pub advisory: usize,
    pub awaiting_you: usize,
    pub awaiting_others: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestFeedback {
    pub schema_version: u8,
    pub number: u64,
    pub head_oid: String,
    #[doc = " The login the `mine` and `owner` fields are relative to."]
    pub viewer: String,
    pub items: Vec<FeedbackItem>,
    pub counts: FeedbackCounts,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl PullRequestFeedback {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn finish(&mut self) {
        self.items.sort_by_key(FeedbackItem::order);
        self.counts = FeedbackCounts::default();
        for item in &self.items {
            if item.kind.is_blocking() {
                self.counts.blocking += 1;
            } else {
                self.counts.advisory += 1;
            }
            match item.owner {
                FeedbackOwner::You => self.counts.awaiting_you += 1,
                FeedbackOwner::Others => self.counts.awaiting_others += 1,
                FeedbackOwner::Nobody => {}
            }
        }
        self.schema_version = Self::SCHEMA_VERSION;
    }

    #[doc = " The first row worth acting on, which is what a `next blocker` control"]
    #[doc = " would jump to."]
    pub(crate) fn next_blocker(&self) -> Option<&FeedbackItem> {
        self.items.iter().find(|item| item.kind.is_blocking())
    }
}

#[doc = " Which rows a caller wants. Applied before the counts, so the summary"]
#[doc = " always describes what is printed."]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FeedbackFilter {
    #[doc = " Only rows the merge is waiting on."]
    pub blocking_only: bool,
    #[doc = " Only rows waiting on you."]
    pub mine_only: bool,
}

impl FeedbackFilter {
    pub(crate) const fn keeps(self, item: &FeedbackItem) -> bool {
        if self.blocking_only && !item.kind.is_blocking() {
            return false;
        }
        !self.mine_only || matches!(item.owner, FeedbackOwner::You)
    }

    pub(crate) fn apply(self, mut feedback: PullRequestFeedback) -> PullRequestFeedback {
        feedback.items.retain(|item| self.keeps(item));
        feedback.finish();
        feedback
    }
}
