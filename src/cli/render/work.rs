#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn work_sessions(listing: &WorkSessions) -> String {
    let mut out = Report::default();
    if listing.sessions.is_empty() {
        out.line("No work sessions recorded");
        return out.finish();
    }
    for session in &listing.sessions {
        out.line(&format!(
            "{:<10} {:<10} {:<14} {:<40} {}",
            session.id,
            session.state().word(),
            session.source().word(),
            truncate(&format!("{}#{}", session.repository, session.number), 40),
            truncate(&session.title, 44)
        ));
    }
    out.line(&format!("\n{} session(s)", listing.sessions.len()));
    out.finish()
}

#[doc = " The whole record. The boundaries are printed rather than assumed,"]
#[doc = " because what a session may not do is the part somebody reading it"]
#[doc = " actually needs to be sure of."]
pub(crate) fn work_session(session: &WorkSession) -> String {
    let mut out = Report::default();
    out.line(&session.headline());
    out.line(&format!(
        "start {}  branch {}",
        short_oid(&session.start_oid),
        session.branch
    ));
    out.line(&session.worktree.as_ref().map_or_else(
        || String::from("worktree none, this session records tasks only"),
        |path| format!("worktree {}", path.display()),
    ));
    if !session.tasks.is_empty() {
        out.blank();
        out.line("tasks (text written by pull-request participants)");
        for task in &session.tasks {
            out.line(&format!(
                "  {:<11} {:<32} {}",
                task.kind,
                truncate(&task.location, 32),
                truncate(&task.summary, 56)
            ));
        }
    }
    if !session.verifications.is_empty() {
        out.blank();
        out.line("verification");
        for verification in &session.verifications {
            out.line(&format!(
                "  {:<7} {}",
                if verification.passed {
                    "passed"
                } else {
                    "failed"
                },
                verification.display_command()
            ));
        }
    }
    if !session.checkpoints.is_empty() {
        out.blank();
        out.line("checkpoints");
        for checkpoint in &session.checkpoints {
            out.line(&format!(
                "  {}  {}",
                short_oid(&checkpoint.oid),
                truncate(&checkpoint.subject, 60)
            ));
        }
    }
    out.blank();
    out.line("this session may");
    for entry in &session.allowed {
        out.line(&format!("  + {entry}"));
    }
    out.line("this session may not");
    for entry in &session.forbidden {
        out.line(&format!("  - {entry}"));
    }
    out.finish()
}

pub(crate) fn work_diff(changes: &WorkDiff) -> String {
    let mut out = Report::default();
    if changes.files.is_empty() {
        out.line(&format!(
            "{} has changed nothing since {}",
            changes.id,
            short_oid(&changes.start_oid)
        ));
        return out.finish();
    }
    for path in &changes.files {
        out.line(path);
    }
    out.blank();
    out.line(changes.patch.trim_end());
    if changes.truncated {
        out.line("[the session patch reached Quinjet's size cap]");
    }
    out.finish()
}

pub(crate) fn work_publish_preview(plan: &WorkPublishPlan) -> String {
    let mut out = Report::default();
    if plan.is_empty() {
        out.line(&format!(
            "Nothing to publish: {} has changed nothing",
            plan.id
        ));
        return out.finish();
    }
    out.line(&format!(
        "Would commit {} file(s) onto {} as:",
        plan.files.len(),
        plan.branch
    ));
    out.line(&format!("  {}", plan.message));
    for path in &plan.files {
        out.line(&format!("  {path}"));
    }
    if let Some(failing) = &plan.failing {
        out.line(&format!("verification `{failing}` last failed"));
    } else if !plan.verified {
        out.line("nothing has been verified on this session yet");
    }
    out.blank();
    out.line("publishing writes one local commit and nothing else. To go further, run:");
    for step in &plan.next {
        out.line(&format!("  {step}"));
    }
    out.line("\nPass --yes to record the commit.");
    out.finish()
}

pub(crate) fn work_abort_preview(session: &WorkSession) -> String {
    let mut text = format!("Would abandon {} and delete {}", session.id, session.branch);
    if let Some(worktree) = &session.worktree {
        text.push_str("\n  removing ");
        text.push_str(&worktree.display().to_string());
    }
    if !session.checkpoints.is_empty() {
        text.push_str("\n  ");
        text.push_str(&session.checkpoints.len().to_string());
        text.push_str(" checkpoint commit(s) would go with the branch");
    }
    text.push_str("\nThe pull request is not touched.\nPass --yes to remove it.");
    text
}
