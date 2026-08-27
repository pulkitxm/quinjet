#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

mod commits;
mod monitor;
pub(super) use monitor::select_check;
use monitor::{checks, logs, watch_conversation, watch_pull_request};
#[cfg(test)]
pub(super) use monitor::{ensure_log_available, exit_for};

#[expect(
    clippy::too_many_lines,
    reason = "the match is the exhaustive pull-request command routing table"
)]
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
        PrVerb::Commits(args) => commits::commits(session, out, &args),
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
                mode: PullRequestMergeMode::Direct,
                delete_branch: args.delete_branch,
            },
        ),
        PrVerb::AdminMerge(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Merge {
                method: args.method.method(),
                mode: PullRequestMergeMode::Admin,
                delete_branch: args.delete_branch,
            },
        ),
        PrVerb::AutoMerge(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Merge {
                method: args.method.method(),
                mode: PullRequestMergeMode::Auto,
                delete_branch: args.delete_branch,
            },
        ),
        PrVerb::DisableAutoMerge(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::DisableAutoMerge,
        ),
        PrVerb::Dequeue(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Dequeue,
        ),
        PrVerb::Ready(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::SetDraft(false),
        ),
        PrVerb::Draft(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::SetDraft(true),
        ),
        PrVerb::Review(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Review {
                kind: args.choice.kind(),
                body: args.body.unwrap_or_default(),
            },
        ),
        PrVerb::Comment(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Comment {
                mode: PullRequestCommentMode::Create,
                body: args.body,
            },
        ),
        PrVerb::EditLastComment(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Comment {
                mode: PullRequestCommentMode::EditLast,
                body: args.body,
            },
        ),
        PrVerb::DeleteLastComment(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Comment {
                mode: PullRequestCommentMode::DeleteLast,
                body: String::new(),
            },
        ),
        PrVerb::Edit(args) => {
            let edit = args.edit()?;
            mutate_pull_request(
                session,
                out,
                &args.pull_request,
                args.yes,
                PullRequestOperation::Edit(edit),
            )
        }
        PrVerb::UpdateBranch(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::UpdateBranch(if args.rebase {
                PullRequestUpdateMethod::Rebase
            } else {
                PullRequestUpdateMethod::Merge
            }),
        ),
        PrVerb::Lock(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Lock(args.reason.map(Into::into)),
        ),
        PrVerb::Unlock(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Unlock,
        ),
        PrVerb::Subscribe(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Subscribe(true),
        ),
        PrVerb::Unsubscribe(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Subscribe(false),
        ),
        PrVerb::AllowMaintainerEdits(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::SetMaintainerEdits(true),
        ),
        PrVerb::DisallowMaintainerEdits(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::SetMaintainerEdits(false),
        ),
        PrVerb::Revert(args) => mutate_pull_request(
            session,
            out,
            &args.pull_request,
            args.yes,
            PullRequestOperation::Revert {
                draft: args.draft,
                title: args.title.unwrap_or_default(),
                body: args.body.unwrap_or_default(),
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
        PrVerb::Reviews { command } => review(session, out, command),
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
    let (mut message, action) = match operation {
        PullRequestOperation::Merge {
            method,
            mode: PullRequestMergeMode::Direct,
            ..
        } => {
            let mut text = String::from("Would ");
            text.push_str(method.preview_verb());
            (text, "merge it.")
        }
        PullRequestOperation::Close => (String::from("Would close"), "close it."),
        PullRequestOperation::Reopen => (String::from("Would reopen"), "reopen it."),
        _ => {
            let mut text = String::from("Would ");
            text.push_str(&operation.label().to_lowercase());
            (text, "perform this action.")
        }
    };
    message.push_str(" #");
    message.push_str(&pull_request.number.to_string());
    message.push_str(" (");
    message.push_str(&pull_request.title);
    message.push_str("). Pass --yes to ");
    message.push_str(action);
    message
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
    prepared_pull_request_diff(
        session,
        out,
        &index,
        format!("PR #{}", request.number),
        path,
    )
}

pub(super) fn prepared_pull_request_diff(
    session: &mut Session,
    out: &Emitter,
    index: &PullRequestDiffIndex,
    title: String,
    path: Option<&Path>,
) -> Result<DiffDocument> {
    let paths: Vec<PathBuf> = match path {
        Some(wanted) => {
            if !index.files.iter().any(|file| file.path == wanted) {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{}` is not part of this pull request", wanted.display()),
                )
                .hint("list the files in this pull-request comparison first")
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
        title,
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
