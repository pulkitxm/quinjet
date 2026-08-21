#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(super) fn pull_request(session: &mut Session, out: &Emitter, command: PrVerb) -> Result<u8> {
    match command {
        PrVerb::View(args) => {
            if args.watch {
                return watch_pull_request(session, out, &args);
            }
            let snapshot = lookup_snapshot(session, out, &args.pull_request)?;
            report_warnings(out, &snapshot);
            out.emit(&snapshot, || render::pull_request(&snapshot.pull_request))?;
            Ok(0)
        }
        PrVerb::Files(args) => {
            let request = lookup(session, out, &args)?;
            let index = prepare(session, out, &request)?;
            out.emit(&index, || render::pull_request_files(&index))?;
            Ok(0)
        }
        PrVerb::Diff(args) => {
            let request = lookup(session, out, &args.pull_request)?;
            let document = pull_request_diff(session, out, &request, args.path.as_deref())?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
        PrVerb::Conversation(args) => {
            if args.watch {
                return watch_conversation(session, out, &args);
            }
            let request = lookup(session, out, &args.pull_request)?;
            let conversation = out
                .execute(
                    session,
                    Command::PullRequestConversation {
                        pull_request: Box::new(request),
                    },
                )?
                .conversation()?;
            out.emit(&conversation, || render::conversation(&conversation))?;
            Ok(0)
        }
        PrVerb::Checks(args) => checks(session, out, &args),
        PrVerb::Logs(args) => logs(session, out, &args),
        PrVerb::Open(args) => {
            let request = lookup(session, out, &args.pull_request)?;
            let url = match args.check {
                None => request.url,
                Some(name) => {
                    let listing = out
                        .execute(
                            session,
                            Command::PullRequestChecks {
                                pull_request: Box::new(request),
                                refresh: args.pull_request.refresh,
                            },
                        )?
                        .checks()?;
                    let check = select_check(&listing.checks, &name)?;
                    if check.link.is_empty() {
                        return Err(Failure::new(
                            EXIT_UNAVAILABLE,
                            format!("the `{}` check has no browser URL", check.name),
                        )
                        .into());
                    }
                    check.link
                }
            };
            open_url(&url)?;
            out.message(&format!("Opened {url}"))?;
            Ok(0)
        }
        PrVerb::Merge(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Merge {
                method: args.method.method(),
                delete_branch: args.delete_branch,
            },
        ),
        PrVerb::Close(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Close,
        ),
        PrVerb::Reopen(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Reopen,
        ),
    }
}

pub(super) fn mutate_pull_request(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
    yes: bool,
    operation: PullRequestOperation,
) -> Result<u8> {
    let pull_request = lookup(session, out, args)?;
    if !yes {
        out.message(&preview_pull_request_operation(&pull_request, &operation))?;
        return Ok(0);
    }
    let message = session
        .execute(Command::OperatePullRequest {
            pull_request: Box::new(pull_request),
            operation,
        })?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}

pub(super) fn preview_pull_request_operation(
    pull_request: &PullRequest,
    operation: &PullRequestOperation,
) -> String {
    let mut message = match operation {
        PullRequestOperation::Merge { method, .. } => {
            let mut text = String::from("Would ");
            text.push_str(method.preview_verb());
            text
        }
        PullRequestOperation::Close => String::from("Would close"),
        PullRequestOperation::Reopen => String::from("Would reopen"),
    };
    message.push_str(" #");
    message.push_str(&pull_request.number.to_string());
    message.push_str(" (");
    message.push_str(&pull_request.title);
    message.push_str("). Pass --yes to ");
    match operation {
        PullRequestOperation::Merge { .. } => message.push_str("merge it."),
        PullRequestOperation::Close => message.push_str("close it."),
        PullRequestOperation::Reopen => message.push_str("reopen it."),
    }
    message
}

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
    let request = lookup(session, out, &args.pull_request)?;
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
                refresh: args.pull_request.refresh,
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

pub(super) fn ensure_log_available(log: &CheckRunLog) -> Result<()> {
    log.unavailable.as_ref().map_or_else(
        || Ok(()),
        |reason| Err(Failure::new(EXIT_UNAVAILABLE, reason.clone()).into()),
    )
}

pub(super) fn select_check(checks: &[PullRequestCheck], wanted: &str) -> Result<PullRequestCheck> {
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

pub(super) fn exit_for(checks: &[PullRequestCheck]) -> u8 {
    let unhappy = checks.iter().any(|check| {
        matches!(
            check.status,
            PullRequestCheckStatus::Failed | PullRequestCheckStatus::Pending
        )
    });
    u8::from(unhappy)
}

pub(super) fn lookup(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<PullRequest> {
    let snapshot = lookup_snapshot(session, out, args)?;
    report_warnings(out, &snapshot);
    Ok(snapshot.pull_request)
}

pub(super) fn lookup_snapshot(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
) -> Result<PullRequestSnapshot> {
    let repositories = match &args.repo {
        None => Vec::new(),
        Some(_) => {
            out.execute(session, Command::GitHubRepositories { refresh: false })?
                .github_repositories()?
                .0
        }
    };
    let selected = match &args.repo {
        None => None,
        Some(wanted) => {
            let found = repositories
                .iter()
                .find(|repository| {
                    repository.name_with_owner.eq_ignore_ascii_case(wanted)
                        || repository.url.ends_with(wanted.as_str())
                })
                .cloned();
            match found {
                Some(repository) => Some(Box::new(repository)),
                None => {
                    return Err(Failure::new(
                        EXIT_NOT_FOUND,
                        format!("no remote of this checkout points at `{wanted}`"),
                    )
                    .hint("run `quinjet repos` for the repositories it can see")
                    .into());
                }
            }
        }
    };
    out.execute(
        session,
        Command::PullRequestLookup {
            repositories,
            repository: selected,
            number: args.number,
            refresh: args.refresh,
        },
    )?
    .pull_request()
}

pub(super) fn report_warnings(out: &Emitter, snapshot: &PullRequestSnapshot) {
    for warning in &snapshot.warnings {
        out.note(&format!("warning: {warning}"));
    }
}

pub(super) fn prepare(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
) -> Result<PullRequestDiffIndex> {
    out.execute(
        session,
        Command::PreparePullRequest {
            workspace: 0,
            pull_request: Box::new(request.clone()),
        },
    )?
    .pull_request_index()
}

pub(super) fn pull_request_diff(
    session: &mut Session,
    out: &Emitter,
    request: &PullRequest,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let index = prepare(session, out, request)?;
    let paths: Vec<PathBuf> = match path {
        Some(wanted) => {
            if !index.files.iter().any(|file| file.path == wanted) {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{}` is not part of this pull request", wanted.display()),
                )
                .hint("run `quinjet pr files <number>` for the files it changes")
                .into());
            }
            vec![wanted.to_path_buf()]
        }
        None => index.files.iter().map(|file| file.path.clone()).collect(),
    };
    let mut loaded = HashMap::new();
    for chunk in paths.chunks(16) {
        for (path, document) in out
            .execute(
                session,
                Command::PullRequestFileBatch {
                    workspace: 0,
                    paths: chunk.to_vec(),
                },
            )?
            .pull_request_diff_batch()?
        {
            drop(loaded.insert(path, document));
        }
    }
    let index = DiffIndex {
        title: format!("PR #{}", request.number),
        files: index
            .files
            .iter()
            .filter(|file| loaded.contains_key(&file.path))
            .map(|file| crate::git::diff::DiffFileIndexEntry {
                path: file.path.clone(),
                old_path: file.old_path.clone(),
                status: render::pull_request_file_label(file.status).to_owned(),
                counts: file.counts,
            })
            .collect(),
        truncated: index.truncated,
        commit_details: None,
    };
    Ok(index.document_with_visibility(&loaded, |_| true))
}

pub(super) fn whole_document(
    session: &mut Session,
    prepare: Command,
    file: impl Fn(u64, PathBuf) -> Command,
) -> Result<DiffDocument> {
    let index = session.execute(prepare)?.local_diff_index()?;
    let mut loaded = HashMap::new();
    for entry in &index.files {
        let (path, document) = session
            .execute(file(0, entry.path.clone()))?
            .local_diff_file()?;
        drop(loaded.insert(path, document));
    }
    Ok(index.document_with_visibility(&loaded, |_| true))
}
