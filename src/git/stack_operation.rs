use std::ffi::OsString;

use super::github::PullRequestMergeMethod;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StackModifyAction {
    Abort,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StackRebaseAction {
    Start {
        branch: Option<String>,
        downstack: bool,
        upstack: bool,
        no_trunk: bool,
        preserve_dates: bool,
        remote: Option<String>,
    },
    Abort,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StackOperation {
    Init {
        branches: Vec<String>,
        base: Option<String>,
    },
    Add {
        branch: String,
        all: bool,
        update: bool,
        message: Option<String>,
    },
    Checkout(String),
    Modify(StackModifyAction),
    Unstack {
        stack: Option<String>,
        local: bool,
    },
    Link {
        members: Vec<String>,
        base: Option<String>,
        open: bool,
        remote: Option<String>,
    },
    Merge {
        target: Option<String>,
        method: PullRequestMergeMethod,
    },
    Push {
        remote: Option<String>,
    },
    Rebase(StackRebaseAction),
    Submit {
        open: bool,
        remote: Option<String>,
    },
    Sync {
        prune: bool,
        remote: Option<String>,
    },
    Bottom,
    Down(u64),
    Top,
    Trunk,
    Up(u64),
}

impl StackOperation {
    pub(crate) fn arguments(&self) -> Vec<OsString> {
        let mut arguments = vec![OsString::from("stack")];
        match self {
            Self::Init { branches, base } => {
                arguments.push(OsString::from("init"));
                push_option(&mut arguments, "--base", base.as_ref());
                if !branches.is_empty() {
                    arguments.push(OsString::from("--"));
                }
                arguments.extend(branches.iter().map(OsString::from));
            }
            Self::Add {
                branch,
                all,
                update,
                message,
            } => {
                arguments.push(OsString::from("add"));
                if *all {
                    arguments.push(OsString::from("--all"));
                }
                if *update {
                    arguments.push(OsString::from("--update"));
                }
                push_option(&mut arguments, "--message", message.as_ref());
                arguments.push(OsString::from("--"));
                arguments.push(OsString::from(branch));
            }
            Self::Checkout(target) => {
                arguments.extend([OsString::from("checkout"), OsString::from("--")]);
                arguments.push(OsString::from(target));
            }
            Self::Modify(action) => {
                arguments.push(OsString::from("modify"));
                arguments.push(OsString::from(match action {
                    StackModifyAction::Abort => "--abort",
                    StackModifyAction::Continue => "--continue",
                }));
            }
            Self::Unstack { stack, local } => {
                arguments.push(OsString::from("unstack"));
                if *local {
                    arguments.push(OsString::from("--local"));
                }
                push_positional(&mut arguments, stack.as_ref());
            }
            Self::Link {
                members,
                base,
                open,
                remote,
            } => {
                arguments.push(OsString::from("link"));
                push_option(&mut arguments, "--base", base.as_ref());
                if *open {
                    arguments.push(OsString::from("--open"));
                }
                push_option(&mut arguments, "--remote", remote.as_ref());
                arguments.push(OsString::from("--"));
                arguments.extend(members.iter().map(OsString::from));
            }
            Self::Merge { target, method } => {
                arguments.extend([
                    OsString::from("merge"),
                    OsString::from(method.flag()),
                    OsString::from("--yes"),
                ]);
                push_positional(&mut arguments, target.as_ref());
            }
            Self::Push { remote } => {
                arguments.push(OsString::from("push"));
                push_option(&mut arguments, "--remote", remote.as_ref());
            }
            Self::Rebase(action) => {
                arguments.push(OsString::from("rebase"));
                push_rebase_arguments(&mut arguments, action);
            }
            Self::Submit { open, remote } => {
                arguments.extend([OsString::from("submit"), OsString::from("--auto")]);
                if *open {
                    arguments.push(OsString::from("--open"));
                }
                push_option(&mut arguments, "--remote", remote.as_ref());
            }
            Self::Sync { prune, remote } => {
                arguments.push(OsString::from("sync"));
                if *prune {
                    arguments.push(OsString::from("--prune"));
                }
                push_option(&mut arguments, "--remote", remote.as_ref());
            }
            Self::Bottom => arguments.push(OsString::from("bottom")),
            Self::Down(steps) => {
                arguments.extend([OsString::from("down"), OsString::from(steps.to_string())]);
            }
            Self::Top => arguments.push(OsString::from("top")),
            Self::Trunk => arguments.push(OsString::from("trunk")),
            Self::Up(steps) => {
                arguments.extend([OsString::from("up"), OsString::from(steps.to_string())]);
            }
        }
        arguments
    }

    pub(crate) const fn progress_label(&self) -> &'static str {
        match self {
            Self::Init { .. } => "Initializing stack",
            Self::Add { .. } => "Adding stack branch",
            Self::Checkout(_) => "Checking out stack",
            Self::Modify(_) => "Recovering stack modification",
            Self::Unstack { .. } => "Removing stack tracking",
            Self::Link { .. } => "Linking pull request stack",
            Self::Merge { .. } => "Merging pull request stack",
            Self::Push { .. } => "Pushing stack branches",
            Self::Rebase(_) => "Rebasing stack branches",
            Self::Submit { .. } => "Submitting pull request stack",
            Self::Sync { .. } => "Synchronizing pull request stack",
            Self::Bottom | Self::Down(_) | Self::Top | Self::Trunk | Self::Up(_) => {
                "Navigating stack branches"
            }
        }
    }

    pub(crate) fn preview_message(&self) -> String {
        format!(
            "Would {}. Pass --yes to continue.",
            self.preview_description()
        )
    }

    pub(crate) const fn success_message(&self) -> &'static str {
        match self {
            Self::Init { .. } => "Stack initialized",
            Self::Add { .. } => "Branch added to stack",
            Self::Checkout(_) => "Stack checked out",
            Self::Modify(StackModifyAction::Abort) => "Stack modification aborted",
            Self::Modify(StackModifyAction::Continue) => "Stack modification continued",
            Self::Unstack { .. } => "Stack tracking removed",
            Self::Link { .. } => "Pull requests linked into a stack",
            Self::Merge { .. } => "Stack merged",
            Self::Push { .. } => "Stack branches pushed",
            Self::Rebase(StackRebaseAction::Abort) => "Stack rebase aborted",
            Self::Rebase(StackRebaseAction::Continue) => "Stack rebase continued",
            Self::Rebase(StackRebaseAction::Start { .. }) => "Stack rebased",
            Self::Submit { .. } => "Stack submitted",
            Self::Sync { .. } => "Stack synchronized",
            Self::Bottom => "Switched to the bottom stack branch",
            Self::Down(_) => "Moved down the stack",
            Self::Top => "Switched to the top stack branch",
            Self::Trunk => "Switched to the stack trunk",
            Self::Up(_) => "Moved up the stack",
        }
    }

    fn preview_description(&self) -> String {
        match self {
            Self::Init { .. } => "initialize a stack".to_owned(),
            Self::Add {
                branch,
                all,
                update,
                message,
            } => message.as_ref().map_or_else(
                || format!("add branch {branch} to the stack"),
                |message| {
                    if *all {
                        format!(
                            "add branch {branch}, stage all changes, and commit them as `{message}`"
                        )
                    } else if *update {
                        format!(
                            "add branch {branch}, stage tracked changes, and commit them as `{message}`"
                        )
                    } else {
                        format!("add branch {branch} and commit staged changes as `{message}`")
                    }
                },
            ),
            Self::Checkout(target) => format!("check out stack {target}"),
            Self::Modify(StackModifyAction::Abort) => {
                "abort the active stack modification".to_owned()
            }
            Self::Modify(StackModifyAction::Continue) => {
                "continue the active stack modification".to_owned()
            }
            Self::Unstack { stack, local } => stack.as_ref().map_or_else(
                || {
                    if *local {
                        "remove local tracking for the active stack".to_owned()
                    } else {
                        "remove local and GitHub tracking for the active stack".to_owned()
                    }
                },
                |stack| {
                    if *local {
                        format!("remove local tracking for stack {stack}")
                    } else {
                        format!("remove local and GitHub tracking for stack {stack}")
                    }
                },
            ),
            Self::Link { members, .. } => {
                format!("link {} members into a pull request stack", members.len())
            }
            Self::Merge { target, method } => target.as_ref().map_or_else(
                || format!("atomically {} the active stack", method.preview_verb()),
                |target| format!("atomically {} stack {target}", method.preview_verb()),
            ),
            Self::Push { .. } => "push the active stack".to_owned(),
            Self::Rebase(StackRebaseAction::Abort) => "abort the active stack rebase".to_owned(),
            Self::Rebase(StackRebaseAction::Continue) => {
                "continue the active stack rebase".to_owned()
            }
            Self::Rebase(StackRebaseAction::Start { .. }) => "rebase the active stack".to_owned(),
            Self::Submit { .. } => "submit the active stack".to_owned(),
            Self::Sync { prune, .. } => {
                if *prune {
                    "synchronize the active stack and prune merged local branches".to_owned()
                } else {
                    "synchronize the active stack".to_owned()
                }
            }
            Self::Bottom => "switch to the bottom stack branch".to_owned(),
            Self::Down(steps) => format!("move {steps} branches down the stack"),
            Self::Top => "switch to the top stack branch".to_owned(),
            Self::Trunk => "switch to the stack trunk".to_owned(),
            Self::Up(steps) => format!("move {steps} branches up the stack"),
        }
    }
}

fn push_option(arguments: &mut Vec<OsString>, flag: &str, value: Option<&String>) {
    if let Some(value) = value {
        arguments.extend([OsString::from(flag), OsString::from(value)]);
    }
}

fn push_positional(arguments: &mut Vec<OsString>, value: Option<&String>) {
    if let Some(value) = value {
        arguments.extend([OsString::from("--"), OsString::from(value)]);
    }
}

fn push_rebase_arguments(arguments: &mut Vec<OsString>, action: &StackRebaseAction) {
    match action {
        StackRebaseAction::Start {
            branch,
            downstack,
            upstack,
            no_trunk,
            preserve_dates,
            remote,
        } => {
            if *downstack {
                arguments.push(OsString::from("--downstack"));
            }
            if *upstack {
                arguments.push(OsString::from("--upstack"));
            }
            if *no_trunk {
                arguments.push(OsString::from("--no-trunk"));
            }
            if *preserve_dates {
                arguments.push(OsString::from("--committer-date-is-author-date"));
            }
            push_option(arguments, "--remote", remote.as_ref());
            push_positional(arguments, branch.as_ref());
        }
        StackRebaseAction::Abort => arguments.push(OsString::from("--abort")),
        StackRebaseAction::Continue => arguments.push(OsString::from("--continue")),
    }
}
