#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Args)]
pub(super) struct ReposArgs {
    #[doc = " Read the remotes again instead of answering from the cache"]
    #[arg(long)]
    pub(super) refresh: bool,
}

pub(super) fn run(session: &mut Session, out: &Emitter, verb: Verb) -> Result<u8> {
    match verb {
        Verb::Tui(_) => Err(Failure::new(
            EXIT_FAILURE,
            "the terminal interface is launched before any verb runs",
        )
        .into()),
        Verb::Completions(_) | Verb::Man(_) | Verb::Capabilities | Verb::Update(_) => {
            Err(Failure::new(
                EXIT_FAILURE,
                "metadata commands run before a repository is opened",
            )
            .into())
        }
        Verb::Status(args) => status(session, out, &args),
        Verb::Diff(args) => working_diff(session, out, &args),
        Verb::Stage(args) => {
            let operation = if args.all {
                GitOperation::StageAll
            } else {
                GitOperation::Stage(require_paths(args.paths, "stage")?)
            };
            operate(session, out, operation)
        }
        Verb::Unstage(args) => {
            let operation = if args.all {
                GitOperation::UnstageAll
            } else {
                GitOperation::Unstage(require_paths(args.paths, "unstage")?)
            };
            operate(session, out, operation)
        }
        Verb::Discard(args) => discard(session, out, &args),
        Verb::Remove(args) => remove(session, out, &args),
        Verb::Commit(args) => operate(
            session,
            out,
            GitOperation::Commit {
                message: args.message,
                amend: args.amend,
            },
        ),
        Verb::Fetch => operate(session, out, GitOperation::Fetch),
        Verb::Pull => operate(session, out, GitOperation::Pull),
        Verb::Push => operate(session, out, GitOperation::Push),
        Verb::Sync => operate(session, out, GitOperation::Sync),
        Verb::Log(args) => log(session, out, &args),
        Verb::Show(args) => show(session, out, &args),
        Verb::Branch { command } => branch(session, out, command),
        Verb::Stash { command } => stash(session, out, command),
        Verb::Worktree { command } => worktree(session, out, command),
        Verb::CherryPick(args) => {
            revision_operation(session, out, &args, "cherry-pick", GitOperation::CherryPick)
        }
        Verb::Revert(args) => {
            revision_operation(session, out, &args, "revert", GitOperation::Revert)
        }
        Verb::Resolve(args) => resolve(session, out, args),
        Verb::Repos(args) => repositories(session, out, &args),
        Verb::Pr { command } => pull_request(session, out, command),
    }
}

pub(super) fn status(session: &mut Session, out: &Emitter, args: &WatchableArgs) -> Result<u8> {
    if args.watch {
        return watch::run(interval(args.interval, 1), out.json, || {
            let status = session.execute(Command::Status)?.status()?;
            Ok(watch::Frame {
                text: render::status(&status),
                value: status,
                finished: false,
                code: 0,
            })
        });
    }
    let status = session.execute(Command::Status)?.status()?;
    out.emit(&status, || render::status(&status))?;
    Ok(0)
}

pub(super) fn working_diff(session: &mut Session, out: &Emitter, args: &DiffArgs) -> Result<u8> {
    let status = session.execute(Command::Status)?.status()?;
    let changes: Vec<Change> = status
        .changes
        .iter()
        .filter(|change| match (args.staged, args.unstaged) {
            (true, _) => change.area == ChangeArea::Staged,
            (_, true) => change.area == ChangeArea::Unstaged,
            _ => true,
        })
        .filter(|change| matches(&change.path, &args.paths))
        .cloned()
        .collect();
    if changes.is_empty() {
        out.message("No changes match")?;
        return Ok(0);
    }
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Changes {
                changes,
                version: 0,
                expanded: args.expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(&document, || render::diff(&document))?;
    Ok(0)
}

pub(super) fn log(session: &mut Session, out: &Emitter, args: &LogArgs) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    let commits = session
        .execute(Command::History {
            revision,
            skip: args.skip,
            limit: args.limit,
        })?
        .history()?;
    out.emit(&commits, || render::history(&commits))?;
    Ok(0)
}

pub(super) fn show(session: &mut Session, out: &Emitter, args: &ShowArgs) -> Result<u8> {
    let revision = revision(session, &args.revision)?;
    let commits = session
        .execute(Command::History {
            revision: revision.clone(),
            skip: 0,
            limit: 1,
        })?
        .history()?;
    let Some(commit) = commits.into_iter().next() else {
        return Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{revision}` does not name a commit in this repository"),
        )
        .into());
    };
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Commit {
                commit: Box::new(commit.clone()),
                expanded: args.expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(
        &CommitPatch {
            commit: &commit,
            diff: &document,
        },
        || format!("{}{}", render::commit(&commit), render::diff(&document)),
    )?;
    Ok(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitPatch<'a> {
    commit: &'a crate::git::history::Commit,
    diff: &'a DiffDocument,
}

pub(super) fn branch(session: &mut Session, out: &Emitter, command: BranchVerb) -> Result<u8> {
    match command {
        BranchVerb::List(args) if args.all => {
            let branches = session
                .execute(Command::HistoryBranches)?
                .history_branches()?;
            out.emit(&branches, || render::history_branches(&branches))?;
            Ok(0)
        }
        BranchVerb::List(_) => {
            let branches = session.execute(Command::Branches)?.branches()?;
            out.emit(&branches, || render::branches(&branches))?;
            Ok(0)
        }
        BranchVerb::Switch { name } => operate(session, out, GitOperation::Checkout(name)),
        BranchVerb::Create { name, start } => {
            let start = match start {
                Some(start) => Some(revision(session, &start)?),
                None => None,
            };
            operate(session, out, GitOperation::CreateBranch { name, start })
        }
        BranchVerb::Rename { old, new } => {
            operate(session, out, GitOperation::RenameBranch { old, new })
        }
        BranchVerb::Delete { name, yes } => {
            if !yes {
                out.message(&format!("Would delete `{name}`. Pass --yes to delete it."))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::DeleteBranch(name))
        }
        BranchVerb::Compare {
            reference,
            expanded,
        } => compare(session, out, &reference, expanded),
    }
}

pub(super) fn compare(
    session: &mut Session,
    out: &Emitter,
    reference: &str,
    expanded: bool,
) -> Result<u8> {
    let branches = session
        .execute(Command::HistoryBranches)?
        .history_branches()?;
    let Some(branch) = branches
        .iter()
        .find(|branch| branch.name == reference || branch.reference == reference)
    else {
        return Err(Failure::new(
            EXIT_NOT_FOUND,
            format!("`{reference}` does not name a branch in this repository"),
        )
        .hint("run `quinjet branch list --all` for the branches that exist")
        .into());
    };
    let status = session.execute(Command::Status)?.status()?;
    let document = whole_document(
        session,
        Command::PrepareLocalDiff {
            workspace: 0,
            request: Box::new(LocalDiffRequest::Branch {
                branch: Box::new(branch.clone()),
                current: status.branch.head.clone(),
                current_oid: status.branch.oid,
                expanded,
            }),
        },
        |workspace, path| Command::LocalDiffFile { workspace, path },
    )?;
    out.emit(&document, || render::diff(&document))?;
    Ok(0)
}

pub(super) fn worktree(session: &mut Session, out: &Emitter, command: WorktreeVerb) -> Result<u8> {
    match command {
        WorktreeVerb::List => {
            let worktrees = session.execute(Command::Worktrees)?.worktrees()?;
            out.emit(&worktrees, || render::worktrees(&worktrees))?;
            Ok(0)
        }
    }
}

pub(super) fn stash(session: &mut Session, out: &Emitter, command: StashVerb) -> Result<u8> {
    match command {
        StashVerb::List => {
            let stashes = session.execute(Command::Stashes)?.stashes()?;
            out.emit(&stashes, || render::stashes(&stashes))?;
            Ok(0)
        }
        StashVerb::Push {
            message,
            include_untracked,
            staged,
            paths,
        } => operate(
            session,
            out,
            GitOperation::StashPush {
                message,
                include_untracked,
                staged,
                paths,
            },
        ),
        StashVerb::Apply { reference } => {
            operate(session, out, GitOperation::StashApply(reference))
        }
        StashVerb::Pop { reference } => operate(session, out, GitOperation::StashPop(reference)),
        StashVerb::Drop { reference, yes } => {
            if !yes {
                out.message(&format!("Would drop `{reference}`. Pass --yes to drop it."))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::StashDrop(reference))
        }
        StashVerb::Clear { yes } => {
            if !yes {
                let stashes = session.execute(Command::Stashes)?.stashes()?;
                out.message(&format!(
                    "Would drop {} stashes. Pass --yes to drop them.",
                    stashes.len()
                ))?;
                return Ok(0);
            }
            operate(session, out, GitOperation::StashClear)
        }
        StashVerb::Show {
            reference,
            expanded,
        } => {
            let stashes = session.execute(Command::Stashes)?.stashes()?;
            let Some(stash) = stashes.iter().find(|stash| stash.reference == reference) else {
                return Err(Failure::new(
                    EXIT_NOT_FOUND,
                    format!("`{reference}` does not name a stash in this repository"),
                )
                .hint("run `quinjet stash list` for the stashes that exist")
                .into());
            };
            let document = whole_document(
                session,
                Command::PrepareLocalDiff {
                    workspace: 0,
                    request: Box::new(LocalDiffRequest::Stash {
                        stash: Box::new(stash.clone()),
                        expanded,
                    }),
                },
                |workspace, path| Command::LocalDiffFile { workspace, path },
            )?;
            out.emit(&document, || render::diff(&document))?;
            Ok(0)
        }
    }
}

pub(super) fn selected_changes(
    session: &mut Session,
    selection: &SelectionArgs,
    verb: &str,
) -> Result<Vec<Change>> {
    if !selection.all && selection.paths.is_empty() {
        return Err(Failure::new(
            EXIT_FAILURE,
            format!("{verb} needs paths, or --all for every change"),
        )
        .into());
    }
    let status = session.execute(Command::Status)?.status()?;
    Ok(status
        .changes
        .iter()
        .filter(|change| change.area != ChangeArea::Conflict)
        .filter(|change| selection.all || matches(&change.path, &selection.paths))
        .cloned()
        .collect())
}

pub(super) fn discard(session: &mut Session, out: &Emitter, args: &DiscardArgs) -> Result<u8> {
    let changes = selected_changes(session, &args.selection, "discard")?;
    if changes.is_empty() {
        out.message("No changes match")?;
        return Ok(0);
    }
    if !args.yes {
        let paths: Vec<String> = changes.iter().map(Change::display_path).collect();
        out.message(&format!(
            "Would discard {} change(s): {}. Pass --yes to discard them.",
            paths.len(),
            paths.join(", ")
        ))?;
        return Ok(0);
    }
    operate(session, out, GitOperation::Discard(changes))
}

pub(super) fn remove(session: &mut Session, out: &Emitter, args: &RemoveArgs) -> Result<u8> {
    let mut paths: Vec<PathBuf> = if args.selection.all {
        let mut changed: Vec<PathBuf> = Vec::new();
        for change in selected_changes(session, &args.selection, "remove")? {
            if !changed.contains(&change.path) {
                changed.push(change.path);
            }
        }
        changed
    } else {
        args.selection.paths.clone()
    };
    paths.dedup();
    if !args.selection.all && paths.is_empty() {
        return Err(Failure::new(
            EXIT_FAILURE,
            "remove needs paths, or --all for every changed file",
        )
        .into());
    }
    if paths.is_empty() {
        out.message("No files match")?;
        return Ok(0);
    }
    if !args.yes {
        let listed: Vec<String> = paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        out.message(&format!(
            "Would remove {} file(s): {}. Pass --yes to remove them.",
            listed.len(),
            listed.join(", ")
        ))?;
        return Ok(0);
    }
    operate(session, out, GitOperation::Remove(paths))
}

pub(super) fn resolve(session: &mut Session, out: &Emitter, args: ResolveArgs) -> Result<u8> {
    let operation = if args.side.stage {
        GitOperation::Stage(vec![args.path])
    } else if args.side.ours {
        GitOperation::ResolveConflict {
            path: args.path,
            choice: ConflictChoice::Ours,
        }
    } else if args.side.theirs {
        GitOperation::ResolveConflict {
            path: args.path,
            choice: ConflictChoice::Theirs,
        }
    } else {
        return Err(Failure::new(
            EXIT_FAILURE,
            "resolve needs one of --ours, --theirs or --stage",
        )
        .into());
    };
    operate(session, out, operation)
}

pub(super) fn repositories(session: &mut Session, out: &Emitter, args: &ReposArgs) -> Result<u8> {
    let (repositories, warnings) = session
        .execute(Command::GitHubRepositories {
            refresh: args.refresh,
        })?
        .github_repositories()?;
    out.emit(
        &RepositoryListing {
            repositories: &repositories,
            warnings: &warnings,
        },
        || render::repositories(&repositories, &warnings),
    )?;
    Ok(0)
}

#[derive(Serialize)]
struct RepositoryListing<'a> {
    repositories: &'a [GitHubRepository],
    warnings: &'a [String],
}
