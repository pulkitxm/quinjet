use std::io::{self, Read};

mod track;
use track::track;

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum PrReviewVerb {
    #[doc = " Print review threads and pending-review state"]
    Show(PrArgs),
    #[doc = " Add a pending line or file comment"]
    Comment(PrReviewCommentArgs),
    #[doc = " Add a pending reply to a review thread"]
    Reply(PrReviewReplyArgs),
    #[doc = " Replace one of your review comments"]
    Edit(PrReviewEditArgs),
    #[doc = " Delete one of your review comments"]
    Delete(PrReviewDeleteArgs),
    #[doc = " Submit the current pending review"]
    Submit(PrReviewSubmitArgs),
    #[doc = " Discard the current pending review"]
    Discard(PrMutateArgs),
    #[doc = " Resolve a review thread"]
    Resolve(PrReviewThreadArgs),
    #[doc = " Reopen a resolved review thread"]
    Unresolve(PrReviewThreadArgs),
    #[doc = " Report what is left to review, measured against a commit"]
    Progress(PrReviewProgressArgs),
    #[doc = " Print the one thing to look at next"]
    Next(PrReviewNextArgs),
    #[doc = " Mark changed files as read or unread"]
    Viewed(PrReviewViewedArgs),
    #[doc = " Record the current head as the commit you last looked at"]
    Visit(PrArgs),
    #[doc = " Add a pending comment proposing an exact replacement"]
    Suggest(PrSuggestArgs),
}

#[derive(Debug, Args)]
pub(super) struct PrReviewProgressArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[command(flatten)]
    pub(super) since: PrSinceArgs,
    #[doc = " List every changed file, not only what is left"]
    #[arg(long)]
    pub(super) all: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewNextArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[command(flatten)]
    pub(super) wanted: PrReviewNextChoiceArgs,
}

#[derive(Debug, Args)]
#[group(multiple = false)]
pub(super) struct PrReviewNextChoiceArgs {
    #[doc = " Only consider changed files"]
    #[arg(long)]
    pub(super) files: bool,
    #[doc = " Only consider unresolved threads"]
    #[arg(long)]
    pub(super) threads: bool,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewViewedArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Repository-relative paths to mark"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) paths: Vec<PathBuf>,
    #[doc = " Mark every changed file instead of named paths"]
    #[arg(long, conflicts_with = "paths")]
    pub(super) all: bool,
    #[doc = " Mark as unread rather than read"]
    #[arg(long)]
    pub(super) unviewed: bool,
    #[doc = " Forget this pull request's local review progress entirely"]
    #[arg(long, conflicts_with_all = ["paths", "all", "unviewed"])]
    pub(super) reset: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewBodyArgs {
    #[doc = " Review text"]
    #[arg(short, long, value_name = "TEXT")]
    pub(super) body: Option<String>,
    #[doc = " Read review text from a file, or standard input with `-`"]
    #[arg(long, value_name = "PATH", value_hint = ValueHint::FilePath)]
    pub(super) body_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(super) enum PrReviewSideArg {
    Left,
    Right,
}

impl From<PrReviewSideArg> for PullRequestReviewSide {
    fn from(side: PrReviewSideArg) -> Self {
        match side {
            PrReviewSideArg::Left => Self::Left,
            PrReviewSideArg::Right => Self::Right,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct PrReviewCommentArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Repository-relative path to comment on"]
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) path: PathBuf,
    #[doc = " End line in the old or new file"]
    #[arg(long, required_unless_present = "file")]
    pub(super) line: Option<usize>,
    #[doc = " Side of the diff containing the end line"]
    #[arg(long, value_enum, required_unless_present = "file")]
    pub(super) side: Option<PrReviewSideArg>,
    #[doc = " First line of a multi-line comment"]
    #[arg(long, requires = "line")]
    pub(super) start_line: Option<usize>,
    #[doc = " Side containing the first line"]
    #[arg(long, value_enum, requires = "start_line")]
    pub(super) start_side: Option<PrReviewSideArg>,
    #[doc = " Comment on the whole file"]
    #[arg(long, conflicts_with_all = ["line", "side", "start_line", "start_side"])]
    pub(super) file: bool,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewReplyArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Review thread node ID"]
    #[arg(value_name = "THREAD_ID")]
    pub(super) thread_id: String,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewEditArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Review comment node ID"]
    #[arg(value_name = "COMMENT_ID")]
    pub(super) comment_id: String,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewDeleteArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Review comment node ID"]
    #[arg(value_name = "COMMENT_ID")]
    pub(super) comment_id: String,
    #[doc = " Confirm; without it the command reports what it would delete"]
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewDecisionArgs {
    #[doc = " Submit general feedback"]
    #[arg(long)]
    pub(super) comment: bool,
    #[doc = " Approve the pull request"]
    #[arg(long)]
    pub(super) approve: bool,
    #[doc = " Request changes"]
    #[arg(long)]
    pub(super) request_changes: bool,
}

impl PrReviewDecisionArgs {
    const fn decision(&self) -> PullRequestReviewDecision {
        if self.approve {
            PullRequestReviewDecision::Approve
        } else if self.request_changes {
            PullRequestReviewDecision::RequestChanges
        } else {
            PullRequestReviewDecision::Comment
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct PrReviewSubmitArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[command(flatten)]
    pub(super) decision: PrReviewDecisionArgs,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewThreadArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    #[doc = " Review thread node ID"]
    #[arg(value_name = "THREAD_ID")]
    pub(super) thread_id: String,
}

pub(super) fn review(session: &mut Session, out: &Emitter, command: PrReviewVerb) -> Result<u8> {
    match command {
        PrReviewVerb::Show(args) => {
            let pull_request = lookup(session, out, &args)?;
            let review = out
                .execute(
                    session,
                    Command::PullRequestReview {
                        pull_request: Box::new(pull_request),
                    },
                )?
                .review()?;
            out.emit(&review, || render::pull_request_review(&review))?;
            Ok(0)
        }
        PrReviewVerb::Comment(args) => {
            let body = review_body(&args.text)?;
            let subject = if args.file {
                PullRequestReviewThreadSubject::File
            } else {
                PullRequestReviewThreadSubject::Line
            };
            mutate(
                session,
                out,
                &args.pull_request,
                PullRequestReviewOperation::AddThread {
                    body,
                    path: args.path,
                    line: args.line,
                    side: args.side.map(Into::into),
                    start_line: args.start_line,
                    start_side: args.start_side.map(Into::into),
                    subject,
                },
            )
        }
        PrReviewVerb::Reply(args) => mutate(
            session,
            out,
            &args.pull_request,
            PullRequestReviewOperation::Reply {
                thread_id: args.thread_id,
                body: review_body(&args.text)?,
            },
        ),
        PrReviewVerb::Edit(args) => mutate(
            session,
            out,
            &args.pull_request,
            PullRequestReviewOperation::UpdateComment {
                comment_id: args.comment_id,
                body: review_body(&args.text)?,
            },
        ),
        PrReviewVerb::Delete(args) => {
            if !args.yes {
                out.message(&format!(
                    "Would delete review comment `{}`. Pass --yes to delete it.",
                    args.comment_id
                ))?;
                return Ok(0);
            }
            mutate(
                session,
                out,
                &args.pull_request,
                PullRequestReviewOperation::DeleteComment {
                    comment_id: args.comment_id,
                },
            )
        }
        PrReviewVerb::Submit(args) => mutate(
            session,
            out,
            &args.pull_request,
            PullRequestReviewOperation::Submit {
                body: review_body(&args.text)?,
                decision: args.decision.decision(),
            },
        ),
        PrReviewVerb::Discard(args) => {
            if !args.yes {
                out.message("Would discard the pending review. Pass --yes to discard it.")?;
                return Ok(0);
            }
            mutate(
                session,
                out,
                &args.pull_request,
                PullRequestReviewOperation::Discard,
            )
        }
        PrReviewVerb::Resolve(args) => mutate(
            session,
            out,
            &args.pull_request,
            PullRequestReviewOperation::Resolve {
                thread_id: args.thread_id,
            },
        ),
        PrReviewVerb::Unresolve(args) => mutate(
            session,
            out,
            &args.pull_request,
            PullRequestReviewOperation::Unresolve {
                thread_id: args.thread_id,
            },
        ),
        other => track(session, out, other),
    }
}

pub(super) fn mutate(
    session: &mut Session,
    out: &Emitter,
    args: &PrArgs,
    operation: PullRequestReviewOperation,
) -> Result<u8> {
    let pull_request = lookup(session, out, args)?;
    let message = out
        .execute(
            session,
            Command::OperatePullRequestReview {
                pull_request: Box::new(pull_request),
                operation,
            },
        )?
        .operation()?
        .2;
    out.message(&message)?;
    Ok(0)
}

pub(super) fn review_body(args: &PrReviewBodyArgs) -> Result<String> {
    let body = match (&args.body, &args.body_file) {
        (Some(body), None) => body.clone(),
        (None, Some(path)) if path == Path::new("-") => {
            let mut body = String::new();
            io::stdin()
                .lock()
                .read_to_string(&mut body)
                .map(|_| ())
                .context("unable to read review text from standard input")?;
            body
        }
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("unable to read review text from {}", path.display()))?,
        _ => return Err(Failure::new(EXIT_FAILURE, "review text is required").into()),
    };
    if body.trim().is_empty() {
        return Err(Failure::new(EXIT_FAILURE, "review text cannot be empty").into());
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_body_accepts_inline_and_file_text() {
        let inline = PrReviewBodyArgs {
            body: Some("Looks good".to_owned()),
            body_file: None,
        };
        assert_eq!(review_body(&inline).unwrap(), "Looks good");
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("review.txt");
        fs::write(&path, "Please adjust this\n").unwrap();
        let file = PrReviewBodyArgs {
            body: None,
            body_file: Some(path),
        };
        assert_eq!(review_body(&file).unwrap(), "Please adjust this\n");
    }

    #[test]
    fn review_body_rejects_missing_empty_and_unreadable_text() {
        let missing = PrReviewBodyArgs {
            body: None,
            body_file: None,
        };
        drop(review_body(&missing).unwrap_err());
        let empty = PrReviewBodyArgs {
            body: Some("  \n".to_owned()),
            body_file: None,
        };
        drop(review_body(&empty).unwrap_err());
        let unreadable = PrReviewBodyArgs {
            body: None,
            body_file: Some(PathBuf::from("missing-review.txt")),
        };
        drop(review_body(&unreadable).unwrap_err());
    }

    #[test]
    fn review_arguments_map_to_domain_values() {
        assert_eq!(
            PullRequestReviewSide::from(PrReviewSideArg::Left),
            PullRequestReviewSide::Left
        );
        assert_eq!(
            PullRequestReviewSide::from(PrReviewSideArg::Right),
            PullRequestReviewSide::Right
        );
        let decision = |approve: bool, request_changes: bool| {
            PrReviewDecisionArgs {
                comment: !approve && !request_changes,
                approve,
                request_changes,
            }
            .decision()
        };
        assert_eq!(decision(false, false), PullRequestReviewDecision::Comment);
        assert_eq!(decision(true, false), PullRequestReviewDecision::Approve);
        assert_eq!(
            decision(false, true),
            PullRequestReviewDecision::RequestChanges
        );
    }
}
