#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Where a session's task list came from. The source is recorded rather"]
#[doc = " than inferred, so a session started from failing checks still says so"]
#[doc = " after the checks have gone green."]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WorkSource {
    #[doc = " The unresolved threads and requested changes on the pull request."]
    Feedback,
    #[doc = " The failing checks and the annotations they placed."]
    FailedChecks,
    #[doc = " The change itself, with no task list."]
    Whole,
}

impl WorkSource {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Feedback => "feedback",
            Self::FailedChecks => "failed-checks",
            Self::Whole => "whole",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WorkSessionState {
    #[doc = " Started and not yet finished with."]
    Open,
    #[doc = " Its work has been committed onto the session branch."]
    Published,
    #[doc = " Given up on; the worktree is gone."]
    Abandoned,
}

impl WorkSessionState {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Published => "published",
            Self::Abandoned => "abandoned",
        }
    }
}

#[doc = " One thing the session was started to deal with. Both the summary and"]
#[doc = " the body are text written by whoever can reach the pull request, so"]
#[doc = " anything reading a session must treat them as data rather than as"]
#[doc = " instructions."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkTask {
    pub kind: String,
    pub id: String,
    pub location: String,
    pub summary: String,
    pub body: String,
    #[doc = " The Quinjet command that resolves this task, which is never run by"]
    #[doc = " the session itself."]
    pub resolved_by: String,
}

#[doc = " A commit the session made on its own branch."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkCheckpoint {
    pub oid: String,
    pub subject: String,
    pub created_at: String,
}

#[doc = " One verification command and how it went. The command is stored as"]
#[doc = " argv rather than a string, because it was never run through a shell and"]
#[doc = " a record that implied otherwise would be a lie about what happened."]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkVerification {
    pub command: Vec<String>,
    pub exit_code: i32,
    pub passed: bool,
    pub ran_at: String,
    #[doc = " The tail of what the command wrote, bounded."]
    pub output: String,
}

impl WorkVerification {
    pub(crate) fn display_command(&self) -> String {
        self.command.join(" ")
    }
}

#[doc = " What a session is allowed to do. The list is stated rather than"]
#[doc = " implied, because the boundary is the point: a coding process working"]
#[doc = " inside a session can change files and commit them, and everything that"]
#[doc = " touches GitHub stays an explicit Quinjet operation somebody asked for."]
pub(crate) const WORK_ALLOWED: [&str; 3] = [
    "read and write files inside the session worktree",
    "commit to the session branch",
    "run verification commands recorded on the session",
];

pub(crate) const WORK_FORBIDDEN: [&str; 4] = [
    "push the branch or any other ref",
    "comment on the pull request or reply to a thread",
    "resolve, unresolve or otherwise change a review thread",
    "merge, close, reopen or edit the pull request",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkSession {
    pub schema_version: u8,
    pub id: String,
    pub repository: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub source: Option<WorkSource>,
    #[doc = " The commit the session starts from, recorded exactly. A session"]
    #[doc = " whose pull request has moved on is still measurable against the"]
    #[doc = " commit it was actually started at."]
    pub start_oid: String,
    pub base_ref: String,
    pub head_ref: String,
    pub branch: String,
    pub worktree: Option<PathBuf>,
    pub created_at: String,
    pub updated_at: String,
    pub state: Option<WorkSessionState>,
    pub tasks: Vec<WorkTask>,
    pub checkpoints: Vec<WorkCheckpoint>,
    pub verifications: Vec<WorkVerification>,
    pub allowed: Vec<String>,
    pub forbidden: Vec<String>,
}

impl WorkSession {
    pub(crate) const SCHEMA_VERSION: u8 = 1;

    pub(crate) fn source(&self) -> WorkSource {
        self.source.unwrap_or(WorkSource::Whole)
    }

    pub(crate) fn state(&self) -> WorkSessionState {
        self.state.unwrap_or(WorkSessionState::Open)
    }

    #[doc = " Whether every recorded verification passed. A session that has run"]
    #[doc = " nothing has not verified anything, which is not the same as passing."]
    pub(crate) fn verified(&self) -> bool {
        !self.verifications.is_empty()
            && self
                .verifications
                .iter()
                .all(|verification| verification.passed)
    }

    pub(crate) fn failing_verification(&self) -> Option<&WorkVerification> {
        self.verifications
            .iter()
            .find(|verification| !verification.passed)
    }

    pub(crate) fn headline(&self) -> String {
        format!(
            "{} on {}#{} from {}, {}",
            self.id,
            self.repository,
            self.number,
            self.source().word(),
            self.state().word()
        )
    }

    pub(crate) fn push_task(&mut self, task: WorkTask) {
        if self.tasks.len() < MAX_TASKS {
            self.tasks.push(task);
        }
    }

    pub(crate) fn push_checkpoint(&mut self, checkpoint: WorkCheckpoint) {
        self.checkpoints.push(checkpoint);
        if self.checkpoints.len() > MAX_CHECKPOINTS {
            drop(self.checkpoints.remove(0));
        }
    }

    pub(crate) fn push_verification(&mut self, verification: WorkVerification) {
        self.verifications
            .retain(|stored| stored.command != verification.command);
        self.verifications.push(verification);
        if self.verifications.len() > MAX_VERIFICATIONS {
            drop(self.verifications.remove(0));
        }
    }
}

#[doc = " What changed inside a session, measured from the commit it started at"]
#[doc = " rather than from anything that has happened on the branch since."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkDiff {
    pub id: String,
    pub start_oid: String,
    pub files: Vec<String>,
    pub patch: String,
    pub truncated: bool,
}

#[doc = " The listing of sessions, so `work list` is one document like every"]
#[doc = " other read."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkSessions {
    pub schema_version: u8,
    pub sessions: Vec<WorkSession>,
}

impl WorkSessions {
    pub(crate) const fn new(sessions: Vec<WorkSession>) -> Self {
        Self {
            schema_version: WorkSession::SCHEMA_VERSION,
            sessions,
        }
    }
}
