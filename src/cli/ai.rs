use std::io::{self, BufRead as _, IsTerminal as _};
use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueHint};

use super::{EXIT_FAILURE, Emitter, Failure, graft};
use crate::git::Repository;

pub(crate) mod key;
pub(crate) mod model;
mod tasks;

pub(crate) use tasks::{AiAnswer, AiTask, run_task};

#[derive(Debug, Args)]
pub(super) struct AiArgs {
    #[command(subcommand)]
    command: AiVerb,
}

#[derive(Debug, Subcommand)]
enum AiVerb {
    #[doc = " Draft a commit message for the staged changes"]
    CommitMessage,
    #[doc = " Review the staged changes, or the unstaged ones when nothing is staged"]
    Review,
    #[doc = " Explain why a pull request's check failed"]
    ExplainCheck(ExplainArgs),
    #[doc = " Triage a pull request by reading its checks, logs, diff and conversation"]
    Investigate(InvestigateArgs),
    #[doc = " Save the Anthropic API key the AI commands use, read from standard input"]
    Key(KeyArgs),
}

#[derive(Debug, Args)]
struct ExplainArgs {
    #[doc = " Pull-request number"]
    #[arg(value_name = "NUMBER", value_hint = ValueHint::Other)]
    number: u64,
    #[doc = " Check run to explain; defaults to the first failing check"]
    #[arg(long, value_name = "NAME", value_hint = ValueHint::Other)]
    check: Option<String>,
}

#[derive(Debug, Args)]
struct InvestigateArgs {
    #[doc = " Pull-request number"]
    #[arg(value_name = "NUMBER", value_hint = ValueHint::Other)]
    number: u64,
}

#[derive(Debug, Args)]
struct KeyArgs {
    #[doc = " Forget the saved key instead"]
    #[arg(long)]
    clear: bool,
}

pub(super) fn run(out: &mut Emitter, repository: &Path, args: AiArgs) -> Result<u8> {
    let task = match args.command {
        AiVerb::Key(key_args) => return key_command(out, &key_args),
        AiVerb::CommitMessage => AiTask::CommitMessage,
        AiVerb::Review => AiTask::Review,
        AiVerb::Investigate(target) => AiTask::Investigate {
            pull_request: target.number,
        },
        AiVerb::ExplainCheck(target) => {
            out.start_progress("Reading the failing check")?;
            let mut session = graft::session(repository)?;
            let log = graft::check_log(&mut session, out, target.number, target.check.as_deref())?;
            AiTask::ExplainCheck {
                pull_request: target.number,
                log,
            }
        }
    };
    out.start_progress("Asking Claude")?;
    let found = Repository::discover(repository)?;
    let answer = run_task(&found, &task).map_err(|error| {
        let failure = Failure::new(EXIT_FAILURE, format!("{error:#}"));
        if format!("{error:#}").contains("no Anthropic API key") {
            failure.hint("paste a key from https://console.anthropic.com into `quinjet ai key`")
        } else {
            failure
        }
    })?;
    out.emit(&answer, || format!("{}\n", answer.text))?;
    Ok(0)
}

fn key_command(out: &Emitter, args: &KeyArgs) -> Result<u8> {
    if args.clear {
        let cleared = key::clear()?;
        out.emit(&serde_json::json!({ "cleared": cleared }), || {
            if cleared {
                "Forgot the saved Anthropic API key\n".to_owned()
            } else {
                "No Anthropic API key was saved\n".to_owned()
            }
        })?;
        return Ok(0);
    }
    if io::stdin().is_terminal() {
        super::note("Paste your Anthropic API key and press Enter:");
    }
    let mut line = String::new();
    let _read = io::stdin()
        .lock()
        .read_line(&mut line)
        .context("failed to read the key from standard input")?;
    let path = key::save(&line)?;
    out.emit(
        &serde_json::json!({ "saved": path.display().to_string() }),
        || format!("Saved the Anthropic API key to {}\n", path.display()),
    )?;
    Ok(0)
}
