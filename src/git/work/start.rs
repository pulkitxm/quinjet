#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " Everything `work start` needs, gathered by the caller so that turning"]
#[doc = " it into a session is a pure function that always produces the same"]
#[doc = " record for the same pull request and the same reads."]
pub(crate) struct WorkStartRequest<'a> {
    pub id: String,
    pub pull_request: &'a PullRequest,
    pub source: WorkSource,
    pub feedback: Option<&'a PullRequestFeedback>,
    pub gate: Option<&'a MergeGate>,
    pub annotations: Option<&'a PullRequestAnnotations>,
    pub created_at: String,
}

#[doc = " Build the record. The task list is whatever the source names and"]
#[doc = " nothing else: a session started from failing checks does not quietly"]
#[doc = " also carry the review threads, because then nobody could say what the"]
#[doc = " session was for."]
pub(crate) fn build_work_session(request: &WorkStartRequest<'_>) -> WorkSession {
    let pull_request = request.pull_request;
    let mut session = WorkSession {
        schema_version: WorkSession::SCHEMA_VERSION,
        id: request.id.clone(),
        repository: pull_request.base_repository.name_with_owner.clone(),
        number: pull_request.number,
        title: pull_request.title.clone(),
        url: pull_request.url.clone(),
        source: Some(request.source),
        start_oid: pull_request.head_oid.clone(),
        base_ref: pull_request.base_ref.clone(),
        head_ref: pull_request.head_ref.clone(),
        branch: work_branch(&request.id),
        worktree: None,
        created_at: request.created_at.clone(),
        updated_at: request.created_at.clone(),
        state: Some(WorkSessionState::Open),
        tasks: Vec::new(),
        checkpoints: Vec::new(),
        verifications: Vec::new(),
        allowed: WORK_ALLOWED
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect(),
        forbidden: WORK_FORBIDDEN
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect(),
    };
    match request.source {
        WorkSource::Feedback => push_feedback_tasks(&mut session, request.feedback),
        WorkSource::FailedChecks => {
            push_check_tasks(&mut session, request.gate, request.annotations);
        }
        WorkSource::Whole => {}
    }
    session
}

#[doc = " The branch a session commits on. It is namespaced so that a session"]
#[doc = " branch is never mistaken for one somebody made by hand, and so that"]
#[doc = " deleting every session branch is one glob."]
pub(crate) fn work_branch(id: &str) -> String {
    format!("quinjet/work/{id}")
}

fn push_feedback_tasks(session: &mut WorkSession, feedback: Option<&PullRequestFeedback>) {
    let Some(feedback) = feedback else {
        return;
    };
    for item in feedback.items.iter().filter(|item| item.kind.is_blocking()) {
        session.push_task(WorkTask {
            kind: item.kind.word().to_owned(),
            id: item.id.clone(),
            location: item.location(),
            summary: item.summary.clone(),
            body: item.body.clone(),
            resolved_by: item.action.clone(),
        });
    }
}

fn push_check_tasks(
    session: &mut WorkSession,
    gate: Option<&MergeGate>,
    annotations: Option<&PullRequestAnnotations>,
) {
    if let Some(gate) = gate {
        for check in gate.checks.failing() {
            session.push_task(WorkTask {
                kind: "check".to_owned(),
                id: check.name.clone(),
                location: check.display_name(),
                summary: if check.required {
                    format!("{} failed and is required", check.display_name())
                } else {
                    format!("{} failed", check.display_name())
                },
                body: check.url.clone(),
                resolved_by: format!(
                    "quinjet pr logs {} {}",
                    session.number,
                    shell_word(&check.name)
                ),
            });
        }
    }
    let Some(annotations) = annotations else {
        return;
    };
    for annotation in annotations
        .annotations
        .iter()
        .filter(|annotation| annotation.severity == AnnotationSeverity::Failure)
    {
        session.push_task(WorkTask {
            kind: "annotation".to_owned(),
            id: annotation.location(),
            location: annotation.location(),
            summary: annotation.headline(),
            body: annotation.message.clone(),
            resolved_by: format!(
                "quinjet pr checks annotations {} --file {}",
                session.number,
                shell_word(&annotation.path.display().to_string())
            ),
        });
    }
}

#[doc = " A name printed inside a suggested command line. Quoting keeps a"]
#[doc = " check name with a space in it copy-pasteable, and nothing here is ever"]
#[doc = " run through a shell by Quinjet itself."]
fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_./".contains(character))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

impl Repository {
    #[doc = " Give a session its own checkout at the exact commit it starts from,"]
    #[doc = " on its own branch, so nothing it does can touch the working tree the"]
    #[doc = " reviewer is using."]
    pub(crate) fn create_work_worktree(&self, session: &WorkSession, path: &Path) -> Result<()> {
        if path.exists() {
            bail!("{} already exists", path.display());
        }
        if session.start_oid.is_empty() {
            bail!("this pull request has no head commit to start from");
        }
        drop(self.checked([
            OsString::from("worktree"),
            OsString::from("add"),
            OsString::from("-b"),
            OsString::from(&session.branch),
            path.as_os_str().to_owned(),
            OsString::from(&session.start_oid),
        ])?);
        Ok(())
    }

    #[doc = " Take the worktree away again. The branch goes with it: a session"]
    #[doc = " nobody wants leaves nothing behind to clean up later."]
    pub(crate) fn remove_work_worktree(&self, session: &WorkSession) -> Result<()> {
        if let Some(worktree) = &session.worktree {
            drop(self.checked([
                OsString::from("worktree"),
                OsString::from("remove"),
                OsString::from("--force"),
                worktree.as_os_str().to_owned(),
            ])?);
        }
        drop(self.run([
            OsString::from("branch"),
            OsString::from("-D"),
            OsString::from(&session.branch),
        ])?);
        Ok(())
    }
}
