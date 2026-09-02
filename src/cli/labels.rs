#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Verb {
    #[doc = " Argument relationships clap cannot express, checked before repository"]
    #[doc = " discovery so a usage mistake never reads a repository first."]
    pub(super) fn validate(&self) -> Result<()> {
        match self {
            Self::Pr {
                command: PrVerb::Checks(command),
            } if command.command.is_none() => drop(command.list.pull_request()?),
            Self::Pr {
                command: PrVerb::Artifacts(command),
            } if command.command.is_none() => {
                drop(command.list.pull_request("pr artifacts")?);
            }
            Self::Pr {
                command: PrVerb::Deployments(command),
            } if command.command.is_none() => {
                drop(command.list.pull_request("pr deployments")?);
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) const fn progress_label(&self) -> Option<&'static str> {
        match self {
            Self::Tui(_)
            | Self::Remote { .. }
            | Self::Project { .. }
            | Self::Completions(_)
            | Self::Man(_)
            | Self::Capabilities => None,
            Self::Status(args) if args.watch => None,
            Self::Status(_) => Some("Reading repository status"),
            Self::Diff(_) => Some("Loading the working-tree diff"),
            Self::Stage(_) => Some("Staging changes"),
            Self::Unstage(_) => Some("Unstaging changes"),
            Self::Discard(_) => Some("Reading changes to discard"),
            Self::Remove(_) => Some("Reading files to remove"),
            Self::Commit(_) => Some("Creating commit"),
            Self::Fetch => Some("Fetching remotes"),
            Self::Pull => Some("Pulling changes"),
            Self::Push => Some("Pushing changes"),
            Self::Sync => Some("Synchronizing changes"),
            Self::Log(_) => Some("Reading commit history"),
            Self::Show(_) => Some("Loading commit patch"),
            Self::Branch { .. } => Some("Reading branch state"),
            Self::Stash { .. } => Some("Reading stash state"),
            Self::Worktree { .. } => Some("Reading worktrees"),
            Self::CherryPick(_) => Some("Resolving commit to cherry-pick"),
            Self::Revert(_) => Some("Resolving commit to revert"),
            Self::Resolve(_) => Some("Resolving conflict"),
            Self::Repos(_) => Some("Discovering GitHub repositories"),
            Self::Pr {
                command: PrVerb::View(args) | PrVerb::Conversation(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Checks(command),
            } if command.command.is_none() && command.list.watch => None,
            Self::Pr {
                command: PrVerb::Logs(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Gate(args),
            } if args.watch => None,
            Self::Pr {
                command: PrVerb::Gate(_),
            } => Some("Evaluating the merge gate"),
            Self::Pr {
                command: PrVerb::Checks(command),
            } if command.command.is_some() => Some("Reading GitHub Actions state"),
            Self::Pr {
                command: PrVerb::Artifacts(_) | PrVerb::Deployments(_),
            } => Some("Reading GitHub Actions state"),
            Self::Pr {
                command: PrVerb::Merge(_) | PrVerb::Close(_) | PrVerb::Reopen(_),
            } => Some("Updating pull request"),
            Self::Pr {
                command: PrVerb::Reviews { .. },
            } => Some("Updating pull-request review"),
            Self::Pr { .. } => Some("Loading pull request"),
            Self::Stack { .. } => Some("Loading pull-request stack"),
            Self::Update(_) => Some("Checking for updates"),
        }
    }
}
