#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " What publishing a session would do, worked out before anything is"]
#[doc = " written so the confirmation has nothing left to surprise anyone with."]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkPublishPlan {
    pub id: String,
    pub branch: String,
    pub start_oid: String,
    pub files: Vec<String>,
    pub message: String,
    pub verified: bool,
    #[doc = " The verification that failed, if one did. Publishing over a failing"]
    #[doc = " verification is allowed, but never quietly."]
    pub failing: Option<String>,
    #[doc = " The Quinjet commands that would take this further. They are printed"]
    #[doc = " rather than run: publishing a session is a local commit, and every"]
    #[doc = " step that reaches GitHub stays something somebody asks for."]
    pub next: Vec<String>,
}

impl WorkPublishPlan {
    pub(crate) const fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[doc = " Work out what publishing would record, without recording it."]
pub(crate) fn plan_work_publish(
    session: &WorkSession,
    message: Option<&str>,
) -> Result<WorkPublishPlan> {
    let diff = work_diff(session)?;
    let untracked = work_untracked(session)?;
    let mut files = diff.files;
    for path in untracked {
        if !files.contains(&path) {
            files.push(path);
        }
    }
    files.sort();
    Ok(WorkPublishPlan {
        id: session.id.clone(),
        branch: session.branch.clone(),
        start_oid: session.start_oid.clone(),
        files,
        message: publish_message(session, message),
        verified: session.verified(),
        failing: session
            .failing_verification()
            .map(WorkVerification::display_command),
        next: next_steps(session),
    })
}

#[doc = " Record the session's work as one commit on its own branch. Nothing"]
#[doc = " leaves the machine: the push, the comment and the thread resolution"]
#[doc = " stay separate Quinjet operations that somebody has to ask for."]
pub(crate) fn publish_work(
    session: &WorkSession,
    plan: &WorkPublishPlan,
    at: String,
) -> Result<WorkCheckpoint> {
    let directory = session_worktree(session)?;
    if plan.is_empty() {
        bail!("session {} has changed nothing to publish", session.id);
    }
    drop(worktree_git(
        directory,
        &[OsString::from("add"), OsString::from("--all")],
        MAX_RECORDED_OUTPUT,
    )?);
    drop(worktree_git(
        directory,
        &[
            OsString::from("commit"),
            OsString::from("--message"),
            OsString::from(&plan.message),
        ],
        MAX_RECORDED_OUTPUT,
    )?);
    let oid = worktree_git(
        directory,
        &[OsString::from("rev-parse"), OsString::from("HEAD")],
        MAX_RECORDED_OUTPUT,
    )?;
    Ok(WorkCheckpoint {
        oid: String::from_utf8_lossy(&oid).trim().to_owned(),
        subject: plan.message.lines().next().unwrap_or_default().to_owned(),
        created_at: at,
    })
}

fn work_untracked(session: &WorkSession) -> Result<Vec<String>> {
    let directory = session_worktree(session)?;
    let output = worktree_git(
        directory,
        &[
            OsString::from("ls-files"),
            OsString::from("--others"),
            OsString::from("--exclude-standard"),
        ],
        MAX_SESSION_PATCH_BYTES,
    )?;
    Ok(String::from_utf8_lossy(&output)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn publish_message(session: &WorkSession, message: Option<&str>) -> String {
    message.map_or_else(
        || {
            format!(
                "work: {} on #{} from {}",
                session.id,
                session.number,
                session.source().word()
            )
        },
        str::to_owned,
    )
}

#[doc = " The operations that reach GitHub, named rather than performed."]
fn next_steps(session: &WorkSession) -> Vec<String> {
    let mut steps = vec![
        format!("git push origin {}", session.branch),
        format!("quinjet pr gate {}", session.number),
    ];
    if session.source() == WorkSource::Feedback {
        steps.push(format!(
            "quinjet pr feedback {} --unresolved",
            session.number
        ));
    }
    if session.source() == WorkSource::FailedChecks {
        steps.push(format!("quinjet pr checks {} --watch", session.number));
    }
    steps
}
