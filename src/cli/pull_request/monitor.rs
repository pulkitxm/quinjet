#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn watch_pull_request(
    session: &mut Session,
    out: &Emitter,
    args: &PrWatchArgs,
) -> Result<u8> {
    watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
        let mut request = args.pull_request.clone();
        request.refresh = true;
        let snapshot = lookup_snapshot(session, out, &request)?;
        Ok(watch::Frame {
            text: render::pull_request(&snapshot.pull_request),
            value: snapshot,
            finished: false,
            code: 0,
        })
    })
}

pub(super) fn watch_conversation(
    session: &mut Session,
    out: &Emitter,
    args: &PrWatchArgs,
) -> Result<u8> {
    watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
        let mut lookup_args = args.pull_request.clone();
        lookup_args.refresh = true;
        let request = lookup_snapshot(session, out, &lookup_args)?.pull_request;
        let conversation = session
            .execute(Command::PullRequestConversation {
                pull_request: Box::new(request),
            })?
            .conversation()?;
        Ok(watch::Frame {
            text: render::conversation(&conversation),
            value: conversation,
            finished: false,
            code: 0,
        })
    })
}

pub(super) fn checks(session: &mut Session, out: &Emitter, args: &PrChecksArgs) -> Result<u8> {
    let listing_args = args.pull_request()?;
    let request = lookup(session, out, &listing_args)?;
    if args.watch {
        return watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
            let checks = session
                .execute(Command::PullRequestChecks {
                    pull_request: Box::new(request.clone()),
                    refresh: true,
                })?
                .checks()?;
            let settled = !checks.checks.iter().any(|check| check.status.is_running());
            Ok(watch::Frame {
                text: render::checks(&checks.checks),
                finished: settled && !checks.checks.is_empty(),
                code: exit_for(&checks.checks),
                value: checks,
            })
        });
    }
    let checks = out
        .execute(
            session,
            Command::PullRequestChecks {
                pull_request: Box::new(request),
                refresh: listing_args.refresh,
            },
        )?
        .checks()?;
    out.emit(&checks, || render::checks(&checks.checks))?;
    Ok(if args.exit_code {
        exit_for(&checks.checks)
    } else {
        0
    })
}

pub(super) fn logs(session: &mut Session, out: &Emitter, args: &PrLogsArgs) -> Result<u8> {
    let request = lookup(session, out, &args.pull_request)?;
    let listing = out
        .execute(
            session,
            Command::PullRequestChecks {
                pull_request: Box::new(request.clone()),
                refresh: args.pull_request.refresh,
            },
        )?
        .checks()?;
    let check = select_check(&listing.checks, &args.check)?;
    if args.watch {
        let name = check.name;
        return watch::run(interval(args.interval, LOG_WATCH_FLOOR), out.json, || {
            let listing = session
                .execute(Command::PullRequestChecks {
                    pull_request: Box::new(request.clone()),
                    refresh: true,
                })?
                .checks()?;
            let check = select_check(&listing.checks, &name)?;
            let log = session
                .execute(Command::CheckRunLog {
                    pull_request: Box::new(request.clone()),
                    check: Box::new(check.clone()),
                })?
                .check_log()?;
            ensure_log_available(&log)?;
            Ok(watch::Frame {
                text: render::check_log(&check, &log),
                finished: !check.status.is_running(),
                code: u8::from(check.status == PullRequestCheckStatus::Failed),
                value: log,
            })
        });
    }
    let log = out
        .execute(
            session,
            Command::CheckRunLog {
                pull_request: Box::new(request),
                check: Box::new(check.clone()),
            },
        )?
        .check_log()?;
    ensure_log_available(&log)?;
    out.emit(&log, || render::check_log(&check, &log))?;
    Ok(0)
}

pub(in crate::cli) fn ensure_log_available(log: &CheckRunLog) -> Result<()> {
    log.unavailable.as_ref().map_or_else(
        || Ok(()),
        |reason| Err(Failure::new(EXIT_UNAVAILABLE, reason.clone()).into()),
    )
}

pub(in crate::cli) fn select_check(
    checks: &[PullRequestCheck],
    wanted: &str,
) -> Result<PullRequestCheck> {
    let exact: Vec<&PullRequestCheck> =
        checks.iter().filter(|check| check.name == wanted).collect();
    if let Some(check) = exact.first() {
        return Ok((*check).clone());
    }
    let partial: Vec<&PullRequestCheck> = checks
        .iter()
        .filter(|check| check.name.to_lowercase().contains(&wanted.to_lowercase()))
        .collect();
    match partial.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("no check on this pull request is called `{wanted}`"),
        )
        .hint(format!(
            "the checks are: {}",
            checks
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
        _ => Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{wanted}` matches more than one check"),
        )
        .hint(format!(
            "name one of: {}",
            partial
                .iter()
                .map(|check| check.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .into()),
    }
}

pub(in crate::cli) fn exit_for(checks: &[PullRequestCheck]) -> u8 {
    let unhappy = checks.iter().any(|check| {
        matches!(
            check.status,
            PullRequestCheckStatus::Failed | PullRequestCheckStatus::Pending
        )
    });
    u8::from(unhappy)
}

pub(super) fn gate(session: &mut Session, out: &Emitter, args: &PrGateArgs) -> Result<u8> {
    if args.watch {
        let request = lookup(session, out, &args.pull_request)?;
        return watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
            let gate = session
                .execute(Command::PullRequestGate {
                    pull_request: Box::new(request.clone()),
                    refresh: true,
                })?
                .gate()?;
            Ok(watch::Frame {
                text: render::merge_gate(&gate),
                finished: gate.verdict.is_settled(),
                code: gate_exit(&gate, args.no_exit_code),
                value: gate,
            })
        });
    }
    let request = lookup(session, out, &args.pull_request)?;
    let gate = out
        .execute(
            session,
            Command::PullRequestGate {
                pull_request: Box::new(request),
                refresh: args.pull_request.refresh,
            },
        )?
        .gate()?;
    out.emit(&gate, || render::merge_gate(&gate))?;
    Ok(gate_exit(&gate, args.no_exit_code))
}

const fn gate_exit(gate: &MergeGate, suppressed: bool) -> u8 {
    if suppressed {
        0
    } else {
        gate.verdict.exit_code()
    }
}
