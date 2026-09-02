use super::command::WorkRequest;
use super::work_verbs::{
    WorkAbortArgs, WorkIdArgs, WorkPublishArgs, WorkStartArgs, WorkVerb, WorkVerifyArgs,
};
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn work(session: &mut Session, out: &Emitter, command: WorkVerb) -> Result<u8> {
    match command {
        WorkVerb::Start(args) => start(session, out, &args),
        WorkVerb::List => list(session, out),
        WorkVerb::Inspect(args) => inspect(session, out, &args),
        WorkVerb::Diff(args) => diff(session, out, &args),
        WorkVerb::Verify(args) => verify(session, out, &args),
        WorkVerb::Publish(args) => publish(session, out, &args),
        WorkVerb::Abort(args) => abort(session, out, &args),
    }
}

fn start(session: &mut Session, out: &Emitter, args: &WorkStartArgs) -> Result<u8> {
    let request = lookup(
        session,
        out,
        &PrArgs {
            number: args.number,
            repo: args.repo.clone(),
            refresh: args.refresh,
        },
    )?;
    let worktree = worktree_path(session, args)?;
    let record = out
        .execute(
            session,
            Command::StartWork {
                pull_request: Box::new(request),
                request: Box::new(WorkRequest {
                    source: args.from.source(),
                    worktree,
                }),
            },
        )?
        .work()?;
    out.emit(&record, || render::work_session(&record))?;
    Ok(0)
}

#[doc = " Where the isolated checkout goes. A sibling of the repository rather"]
#[doc = " than a directory inside it, so a session's files never show up as"]
#[doc = " untracked changes in the checkout somebody is reviewing from."]
fn worktree_path(session: &Session, args: &WorkStartArgs) -> Result<Option<PathBuf>> {
    if let Some(into) = &args.into {
        return Ok(Some(into.clone()));
    }
    if !args.worktree {
        return Ok(None);
    }
    let root = session.repository_root();
    let parent = root
        .parent()
        .ok_or_else(|| Failure::new(EXIT_FAILURE, "this repository has no parent directory"))?;
    let name = root.file_name().map_or_else(
        || "repository".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(Some(parent.join(format!("{name}-work-{}", args.number))))
}

fn list(session: &mut Session, out: &Emitter) -> Result<u8> {
    let sessions = out.execute(session, Command::ListWork)?.work_sessions()?;
    out.emit(&sessions, || render::work_sessions(&sessions))?;
    Ok(0)
}

fn inspect(session: &mut Session, out: &Emitter, args: &WorkIdArgs) -> Result<u8> {
    let record = read_session(session, out, &args.id)?;
    out.emit(&record, || render::work_session(&record))?;
    Ok(0)
}

fn diff(session: &mut Session, out: &Emitter, args: &WorkIdArgs) -> Result<u8> {
    let changes = out
        .execute(
            session,
            Command::WorkDiff {
                id: args.id.clone(),
            },
        )
        .map_err(missing_session)?
        .work_diff()?;
    out.emit(&changes, || render::work_diff(&changes))?;
    Ok(0)
}

fn verify(session: &mut Session, out: &Emitter, args: &WorkVerifyArgs) -> Result<u8> {
    let record = out
        .execute(
            session,
            Command::VerifyWork {
                id: args.id.clone(),
                command: args.command.clone(),
            },
        )
        .map_err(missing_session)?
        .work()?;
    out.emit(&record, || render::work_session(&record))?;
    Ok(if args.exit_code && !record.verified() {
        EXIT_FAILURE
    } else {
        0
    })
}

fn publish(session: &mut Session, out: &Emitter, args: &WorkPublishArgs) -> Result<u8> {
    let record = read_session(session, out, &args.id)?;
    let plan = out
        .execute(
            session,
            Command::PlanWorkPublish {
                id: args.id.clone(),
                message: args.message.clone(),
            },
        )
        .map_err(missing_session)?
        .work_publish_plan()?;
    if !args.yes || plan.is_empty() {
        out.emit(&plan, || render::work_publish_preview(&plan))?;
        return Ok(0);
    }
    let published = session
        .execute(Command::PublishWork {
            session: Box::new(record),
            plan: Box::new(plan),
        })?
        .work()?;
    out.emit(&published, || render::work_session(&published))?;
    Ok(0)
}

fn abort(session: &mut Session, out: &Emitter, args: &WorkAbortArgs) -> Result<u8> {
    let record = read_session(session, out, &args.id)?;
    if !args.yes {
        out.message(&render::work_abort_preview(&record))?;
        return Ok(0);
    }
    let record = out
        .execute(
            session,
            Command::AbortWork {
                id: args.id.clone(),
            },
        )
        .map_err(missing_session)?
        .work()?;
    out.message(&format!(
        "Abandoned {} and removed {}",
        record.id, record.branch
    ))?;
    Ok(0)
}

fn read_session(session: &mut Session, out: &Emitter, id: &str) -> Result<WorkSession> {
    out.execute(session, Command::InspectWork { id: id.to_owned() })
        .map_err(missing_session)?
        .work()
}

#[doc = " A name that matches no session is a not-found, not a failure, and the"]
#[doc = " hint names the one command that lists what it could have been."]
fn missing_session(error: anyhow::Error) -> anyhow::Error {
    let message = format!("{error:#}");
    if message.contains("no work session is called") {
        return Failure::new(EXIT_NOT_FOUND, message)
            .hint("run `quinjet work list` for the sessions it knows about")
            .into();
    }
    error
}
