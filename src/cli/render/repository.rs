#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

fn ahead_label(count: usize) -> String {
    format!(" ahead {count}")
}

fn behind_label(count: usize) -> String {
    format!(" behind {count}")
}

pub(crate) fn status(status: &RepoStatus) -> String {
    let mut out = Report::default();
    let branch = &status.branch;
    let head = if branch.detached {
        format!("HEAD detached at {}", branch.head)
    } else {
        format!("On branch {}", branch.head)
    };
    out.line(&head);
    if let Some(upstream) = &branch.upstream {
        let mut divergence = String::new();
        if branch.ahead > 0 {
            divergence.push_str(&ahead_label(branch.ahead));
        }
        if branch.behind > 0 {
            divergence.push_str(&behind_label(branch.behind));
        }
        out.line(&format!("Tracking {upstream}{divergence}"));
    }
    if status.changes.is_empty() {
        out.line("\nWorking tree clean");
        return out.finish();
    }
    for area in [
        ChangeArea::Conflict,
        ChangeArea::Staged,
        ChangeArea::Unstaged,
    ] {
        let changes: Vec<_> = status
            .changes
            .iter()
            .filter(|change| change.area == area)
            .collect();
        if changes.is_empty() {
            continue;
        }
        out.line(&format!("\n{} ({})", area.label(), changes.len()));
        for change in changes {
            let rename = change
                .original_path
                .as_ref()
                .map(|from| format!(" (from {})", from.display()))
                .unwrap_or_default();
            out.line(&format!(
                "  {:<3} {}{rename}",
                change.status.code(),
                change.display_path()
            ));
        }
    }
    out.finish()
}

pub(crate) fn diff(document: &DiffDocument) -> String {
    let mut out = Report::default();
    for line in &document.lines {
        let text = line.text();
        match line.kind {
            DiffLineKind::FileHeader => {
                if !out.empty() {
                    out.blank();
                }
                out.line(&text);
            }
            DiffLineKind::FileFooter => {}
            DiffLineKind::HunkHeader | DiffLineKind::Meta => {
                out.line(&text);
            }
            DiffLineKind::Added => {
                out.line(&format!("+{text}"));
            }
            DiffLineKind::Removed => {
                out.line(&format!("-{text}"));
            }
            DiffLineKind::Context => {
                out.line(&format!(" {text}"));
            }
        }
    }
    if document.truncated {
        out.line("\n[output reached Quinjet's size cap and was truncated]");
    }
    out.finish()
}

pub(crate) fn history(commits: &[Commit]) -> String {
    let mut out = Report::default();
    for commit in commits {
        let decorations = if commit.decorations.is_empty() {
            String::new()
        } else {
            format!("  ({})", commit.decorations.join(", "))
        };
        out.line(&format!(
            "{}  {}  {:<16}  {}{decorations}",
            commit.short_id,
            format_local_timestamp(&commit.authored_at),
            truncate(&commit.author, 16),
            commit.subject
        ));
    }
    out.finish()
}

pub(crate) fn commit(commit: &Commit) -> String {
    let mut out = Report::default();
    out.line(&format!("commit {}", commit.id));
    if commit.parent_ids.len() > 1 {
        out.line(&format!("Merge:  {}", commit.parent_ids.join(" ")));
    }
    out.line(&format!(
        "Author: {} <{}>",
        commit.author, commit.author_email
    ));
    out.line(&format!(
        "Date:   {}",
        format_local_timestamp(&commit.authored_at)
    ));
    out.line(&format!("\n    {}\n", commit.subject));
    out.finish()
}

pub(crate) fn branches(branches: &[Branch]) -> String {
    let mut out = Report::default();
    for branch in branches {
        let upstream = branch
            .upstream
            .as_ref()
            .map(|upstream| format!("  -> {upstream}"))
            .unwrap_or_default();
        out.line(&format!(
            "{} {:<28} {:<10} {}{upstream}",
            if branch.current { "*" } else { " " },
            branch.name,
            branch.short_id,
            format_local_timestamp(&branch.relative_date)
        ));
    }
    out.finish()
}

pub(crate) fn history_branches(branches: &[HistoryBranch]) -> String {
    let mut out = Report::default();
    for branch in branches {
        out.line(&format!(
            "{} {:<40} {:<8} {:<10} {}",
            if branch.current { "*" } else { " " },
            branch.name,
            if branch.remote { "remote" } else { "local" },
            branch.short_id,
            format_local_timestamp(&branch.relative_date)
        ));
    }
    out.finish()
}

pub(crate) fn stashes(stashes: &[Stash]) -> String {
    let mut out = Report::default();
    for stash in stashes {
        out.line(&format!(
            "{:<12} {:<10} {:<14} on {}: {}",
            stash.reference,
            stash.short_id,
            format_local_timestamp(&stash.relative_date),
            stash.branch,
            stash.message
        ));
    }
    out.finish()
}

pub(crate) fn worktrees(worktrees: &[Worktree]) -> String {
    let mut out = Report::default();
    for worktree in worktrees {
        let branch = worktree.branch_label();
        let head = if worktree.head.is_empty() {
            "-"
        } else {
            worktree.short_head()
        };
        let mut flags = String::new();
        if worktree.locked.is_some() {
            flags.push_str("  locked");
        }
        if worktree.prunable.is_some() {
            flags.push_str("  prunable");
        }
        out.line(&format!(
            "{} {}  {:<16}  {head}{flags}",
            if worktree.current { "*" } else { " " },
            worktree.path.display(),
            branch
        ));
    }
    out.finish()
}
