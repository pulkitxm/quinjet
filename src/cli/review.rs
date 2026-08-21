use std::io::{self, Read};

#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Subcommand)]
pub(super) enum PrReviewVerb {
    /// Print review threads and pending-review state
    Show(PrArgs),
    /// Add a pending line or file comment
    Comment(PrReviewCommentArgs),
    /// Add a pending reply to a review thread
    Reply(PrReviewReplyArgs),
    /// Replace one of your review comments
    Edit(PrReviewEditArgs),
    /// Delete one of your review comments
    Delete(PrReviewDeleteArgs),
    /// Submit the current pending review
    Submit(PrReviewSubmitArgs),
    /// Discard the current pending review
    Discard(PrMutateArgs),
    /// Resolve a review thread
    Resolve(PrReviewThreadArgs),
    /// Reopen a resolved review thread
    Unresolve(PrReviewThreadArgs),
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewBodyArgs {
    /// Review text
    #[arg(short, long, value_name = "TEXT")]
    pub(super) body: Option<String>,
    /// Read review text from a file, or standard input with `-`
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
    /// Repository-relative path to comment on
    #[arg(value_name = "PATH", value_hint = ValueHint::AnyPath)]
    pub(super) path: PathBuf,
    /// End line in the old or new file
    #[arg(long, required_unless_present = "file")]
    pub(super) line: Option<usize>,
    /// Side of the diff containing the end line
    #[arg(long, value_enum, required_unless_present = "file")]
    pub(super) side: Option<PrReviewSideArg>,
    /// First line of a multi-line comment
    #[arg(long, requires = "line")]
    pub(super) start_line: Option<usize>,
    /// Side containing the first line
    #[arg(long, value_enum, requires = "start_line")]
    pub(super) start_side: Option<PrReviewSideArg>,
    /// Comment on the whole file
    #[arg(long, conflicts_with_all = ["line", "side", "start_line", "start_side"])]
    pub(super) file: bool,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewReplyArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Review thread node ID
    #[arg(value_name = "THREAD_ID")]
    pub(super) thread_id: String,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewEditArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Review comment node ID
    #[arg(value_name = "COMMENT_ID")]
    pub(super) comment_id: String,
    #[command(flatten)]
    pub(super) text: PrReviewBodyArgs,
}

#[derive(Debug, Args)]
pub(super) struct PrReviewDeleteArgs {
    #[command(flatten)]
    pub(super) pull_request: PrArgs,
    /// Review comment node ID
    #[arg(value_name = "COMMENT_ID")]
    pub(super) comment_id: String,
    /// Confirm; without it the command reports what it would delete
    #[arg(long)]
    pub(super) yes: bool,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub(super) struct PrReviewDecisionArgs {
    /// Submit general feedback
    #[arg(long)]
    pub(super) comment: bool,
    /// Approve the pull request
    #[arg(long)]
    pub(super) approve: bool,
    /// Request changes
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
    /// Review thread node ID
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
    }
}

fn mutate(
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

fn review_body(args: &PrReviewBodyArgs) -> Result<String> {
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
