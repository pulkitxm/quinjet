#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
enum SearchScope {
    #[default]
    Changes,
    History,
    PullRequest,
}

#[derive(Debug, Args)]
pub(super) struct SearchArgs {
    #[doc = " Text to match. Name uses a substring; Contents uses ripgrep regex"]
    #[arg(value_name = "QUERY")]
    query: String,
    #[doc = " Name matches list labels, Contents searches file or commit text, Both unions them. Default Name"]
    #[arg(long, value_enum, default_value_t = SearchMode::Name)]
    mode: SearchMode,
    #[doc = " Which list to search"]
    #[arg(long, value_enum, default_value_t = SearchScope::Changes)]
    scope: SearchScope,
    #[doc = " Branch, tag, or commit to read from when searching history"]
    #[arg(long, default_value = "HEAD", value_name = "REVISION", value_hint = ValueHint::Other)]
    revision: String,
    #[doc = " Commits to skip when searching history"]
    #[arg(long, default_value_t = 0, value_hint = ValueHint::Other)]
    skip: usize,
    #[doc = " Commits to search when searching history"]
    #[arg(long, short = 'n', default_value_t = 300, value_hint = ValueHint::Other)]
    limit: usize,
    #[doc = " Pull-request number when --scope pull-request"]
    #[arg(long = "pr", value_name = "NUMBER", value_hint = ValueHint::Other)]
    pull_request: Option<u64>,
    #[doc = " GitHub repository owner/name when searching a pull request"]
    #[arg(long, value_name = "OWNER/NAME", value_hint = ValueHint::Other)]
    repo: Option<String>,
}

pub(super) fn search(session: &mut Session, out: &Emitter, args: &SearchArgs) -> Result<u8> {
    let hits = match args.scope {
        SearchScope::Changes => {
            let status = session.execute(Command::Status)?.status()?;
            let files = status
                .changes
                .into_iter()
                .map(|change| crate::search::SearchFile {
                    path: change.path,
                    source: if change.area == ChangeArea::Staged {
                        crate::search::SearchSource::Index
                    } else {
                        crate::search::SearchSource::Worktree
                    },
                })
                .collect();
            session
                .execute(Command::Search(Box::new(crate::search::SearchRequest {
                    query: args.query.clone(),
                    mode: args.mode,
                    target: crate::search::SearchTarget::Changes { files },
                })))?
                .search()?
        }
        SearchScope::History => {
            let revision = revision(session, &args.revision)?;
            session
                .execute(Command::Search(Box::new(crate::search::SearchRequest {
                    query: args.query.clone(),
                    mode: args.mode,
                    target: crate::search::SearchTarget::History {
                        revision,
                        skip: args.skip,
                        limit: args.limit,
                        selected: None,
                    },
                })))?
                .search()?
        }
        SearchScope::PullRequest => {
            let number = args.pull_request.ok_or_else(|| {
                Failure::new(EXIT_FAILURE, "--pr is required when --scope pull-request")
            })?;
            let request = lookup(
                session,
                out,
                &PrArgs {
                    number,
                    repo: args.repo.clone(),
                    refresh: false,
                },
            )?;
            let index = prepare(session, out, &request)?;
            let paths = index.files.into_iter().map(|file| file.path).collect();
            session
                .execute(Command::Search(Box::new(crate::search::SearchRequest {
                    query: args.query.clone(),
                    mode: args.mode,
                    target: crate::search::SearchTarget::PullRequest {
                        workspace: 0,
                        paths,
                    },
                })))?
                .search()?
        }
    };
    out.emit(&hits, || render::search(&hits))?;
    Ok(0)
}
