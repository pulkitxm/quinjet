#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Session {
    #[doc = " Start a session: gather what the source names, build the record, and"]
    #[doc = " give it its own checkout at the exact commit the pull request head is"]
    #[doc = " at right now."]
    pub(crate) fn start_work(
        &self,
        pull_request: &PullRequest,
        request: &WorkRequest,
    ) -> Result<WorkSession> {
        let id = next_work_session_id(pull_request.number);
        let (feedback, gate, annotations) = self.work_inputs(pull_request, request.source)?;
        let mut session = build_work_session(&WorkStartRequest {
            id,
            pull_request,
            source: request.source,
            feedback: feedback.as_ref(),
            gate: gate.as_ref(),
            annotations: annotations.as_ref(),
            created_at: crate::date_time::now_timestamp(),
        });
        if let Some(path) = &request.worktree {
            self.repository.create_work_worktree(&session, path)?;
            session.worktree = Some(path.clone());
        }
        record_work_session(session.clone());
        Ok(session)
    }

    #[doc = " Run one verification and fold the result into the stored record, so"]
    #[doc = " the session always says what has actually been run against it."]
    pub(crate) fn verify_work(id: &str, argv: &[String]) -> Result<WorkSession> {
        let mut session = Self::work_session(id)?;
        let commands = if argv.is_empty() {
            replayable(&session)?
        } else {
            vec![argv.to_vec()]
        };
        for command in commands {
            let verification =
                run_work_verification(&session, &command, crate::date_time::now_timestamp())?;
            session.push_verification(verification);
        }
        session.updated_at = crate::date_time::now_timestamp();
        record_work_session(session.clone());
        Ok(session)
    }

    pub(crate) fn work_diff(id: &str) -> Result<WorkDiff> {
        work_diff(&Self::work_session(id)?)
    }

    pub(crate) fn plan_work_publish(
        id: &str,
        message: Option<&str>,
    ) -> Result<(WorkSession, WorkPublishPlan)> {
        let session = Self::work_session(id)?;
        let plan = plan_work_publish(&session, message)?;
        Ok((session, plan))
    }

    pub(crate) fn publish_work(
        session: &WorkSession,
        plan: &WorkPublishPlan,
    ) -> Result<WorkSession> {
        let checkpoint = publish_work(session, plan, crate::date_time::now_timestamp())?;
        let mut session = session.clone();
        session.push_checkpoint(checkpoint);
        session.state = Some(WorkSessionState::Published);
        session.updated_at = crate::date_time::now_timestamp();
        record_work_session(session.clone());
        Ok(session)
    }

    #[doc = " Take the worktree and the branch away and forget the record. The"]
    #[doc = " pull request is untouched: a session was never anything GitHub knew"]
    #[doc = " about."]
    pub(crate) fn abort_work(&self, id: &str) -> Result<WorkSession> {
        let session = Self::work_session(id)?;
        self.repository.remove_work_worktree(&session)?;
        forget_work_session(&session.id);
        let mut session = session;
        session.state = Some(WorkSessionState::Abandoned);
        session.worktree = None;
        Ok(session)
    }

    pub(crate) fn work_session(id: &str) -> Result<WorkSession> {
        load_work_session(id).ok_or_else(|| anyhow::anyhow!("no work session is called `{id}`"))
    }

    fn work_inputs(&self, pull_request: &PullRequest, source: WorkSource) -> Result<WorkInputs> {
        match source {
            WorkSource::Whole => Ok((None, None, None)),
            WorkSource::Feedback => {
                let gate = self.repository.pull_request_gate(pull_request, false)?;
                let review = self.repository.pull_request_review(pull_request)?;
                let viewer = self.viewer_login(pull_request).unwrap_or_default();
                let feedback = build_feedback(&FeedbackInputs {
                    pull_request,
                    gate: Some(&gate),
                    review: &review,
                    annotations: None,
                    viewer: &viewer,
                    warnings: Vec::new(),
                });
                Ok((Some(feedback), Some(gate), None))
            }
            WorkSource::FailedChecks => {
                let gate = self.repository.pull_request_gate(pull_request, false)?;
                let annotations = self
                    .repository
                    .pull_request_annotations(pull_request, false)
                    .ok();
                Ok((None, Some(gate), annotations))
            }
        }
    }
}

type WorkInputs = (
    Option<PullRequestFeedback>,
    Option<MergeGate>,
    Option<PullRequestAnnotations>,
);

#[doc = " Re-running a session's verifications means running exactly the"]
#[doc = " commands it already recorded, in the order it recorded them. A session"]
#[doc = " that has run nothing has nothing to replay, and saying so beats"]
#[doc = " reporting a pass nobody earned."]
fn replayable(session: &WorkSession) -> Result<Vec<Vec<String>>> {
    if session.verifications.is_empty() {
        anyhow::bail!(
            "session {} has no recorded command to re-run; pass one after `--`",
            session.id
        );
    }
    Ok(session
        .verifications
        .iter()
        .map(|verification| verification.command.clone())
        .collect())
}
