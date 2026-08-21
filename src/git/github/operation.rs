use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RemoteUrl {
    pub(super) remote: String,
    pub(super) url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CacheDisposition {
    Network,
    Fresh,
    Stale,
}

/// How long an entry stays usable. `Immutable` is for content whose identity is
/// already in its key: a finished run's log, or a patch between two fixed
/// commits. Such an entry can never become wrong, only evicted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLife {
    Immutable,
    Ttl(Duration),
}

impl CacheLife {
    pub(super) fn accepts(self, age: Duration) -> bool {
        match self {
            Self::Immutable => true,
            Self::Ttl(ttl) => age <= ttl,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PullRequestProgress {
    LoadingMetadata,
    PreparingRepository,
    FetchingBase,
    FetchingHead,
    FindingMergeBase,
    EnumeratingFiles,
}

impl PullRequestProgress {
    pub(crate) const fn percent(self) -> u16 {
        match self {
            Self::LoadingMetadata => 10,
            Self::PreparingRepository => 20,
            Self::FetchingBase => 35,
            Self::FetchingHead => 50,
            Self::FindingMergeBase => 65,
            Self::EnumeratingFiles => 90,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::LoadingMetadata => "Fetching pull-request metadata",
            Self::PreparingRepository => "Preparing an isolated diff workspace",
            Self::FetchingBase => "Fetching the destination commit",
            Self::FetchingHead => "Fetching the source commit",
            Self::FindingMergeBase => "Finding the merge base",
            Self::EnumeratingFiles => "Enumerating changed files",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PullRequestMergeMethod {
    Merge,
    #[default]
    Squash,
    Rebase,
}

impl PullRequestMergeMethod {
    pub(crate) const ALL: [Self; 3] = [Self::Merge, Self::Squash, Self::Rebase];

    pub(crate) const fn flag(self) -> &'static str {
        match self {
            Self::Merge => "--merge",
            Self::Squash => "--squash",
            Self::Rebase => "--rebase",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Merge => "Create a merge commit",
            Self::Squash => "Squash and merge",
            Self::Rebase => "Rebase and merge",
        }
    }

    pub(crate) const fn preview_verb(self) -> &'static str {
        match self {
            Self::Merge => "create a merge commit for",
            Self::Squash => "squash and merge",
            Self::Rebase => "rebase and merge",
        }
    }

    pub(crate) const fn done_verb(self) -> &'static str {
        match self {
            Self::Merge => "Merged",
            Self::Squash => "Squashed and merged",
            Self::Rebase => "Rebased and merged",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PullRequestOperation {
    Merge {
        method: PullRequestMergeMethod,
        delete_branch: bool,
    },
    Close,
    Reopen,
}

impl PullRequestOperation {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Merge { method, .. } => method.label(),
            Self::Close => "Close pull request",
            Self::Reopen => "Reopen pull request",
        }
    }

    pub(crate) fn confirm_title(&self) -> String {
        let mut title = self.label().to_owned();
        title.push('?');
        title
    }

    pub(crate) fn confirm_message(&self, pull_request: &PullRequest) -> String {
        let mut message = match self {
            Self::Merge { method, .. } => {
                let mut text = String::from("Really ");
                text.push_str(method.preview_verb());
                text
            }
            Self::Close => String::from("Really close"),
            Self::Reopen => String::from("Really reopen"),
        };
        message.push_str(" #");
        message.push_str(&pull_request.number.to_string());
        message.push_str(" (");
        message.push_str(&pull_request.title);
        message.push_str(")?");
        message
    }

    pub(crate) fn success_message(&self, pull_request: &PullRequest) -> String {
        let mut message = match self {
            Self::Merge { method, .. } => method.done_verb().to_owned(),
            Self::Close => String::from("Closed"),
            Self::Reopen => String::from("Reopened"),
        };
        message.push_str(" #");
        message.push_str(&pull_request.number.to_string());
        message
    }
}
