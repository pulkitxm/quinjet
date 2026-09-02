#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

pub(crate) fn review_progress(progress: &ReviewProgress, all: bool) -> String {
    let mut out = Report::default();
    out.line(&format!(
        "#{}  {} of {} files read  ·  {} unresolved",
        progress.number,
        progress.viewed,
        progress.files.len(),
        progress.threads.unresolved
    ));
    out.line(&format!(
        "since    {} {}",
        progress.since.source.label(),
        short_oid(&progress.since.oid)
    ));
    if !progress.new_commits.is_empty() {
        out.line(&format!(
            "commits  {} new since then",
            progress.new_commits.len()
        ));
    }
    if progress.changed_since_viewed > 0 {
        out.line(&format!(
            "changed  {} file(s) you had read have moved since",
            progress.changed_since_viewed
        ));
    }
    if progress.threads.unresolved > 0 {
        out.line(&format!(
            "threads  {} awaiting your reply, {} awaiting others, {} outdated",
            progress.threads.awaiting_your_reply,
            progress.threads.awaiting_others,
            progress.threads.outdated_unresolved
        ));
    }
    out.blank();
    let listed: Vec<&ReviewFileProgress> = progress
        .files
        .iter()
        .filter(|file| all || file.state.is_remaining())
        .collect();
    if listed.is_empty() {
        out.line(if progress.is_complete() {
            "Nothing left to review"
        } else {
            "No changed files left to read"
        });
    }
    for file in listed {
        let delta = if file.changed_since { " *" } else { "" };
        out.line(&format!(
            "{:<9} {}{delta}",
            file.state.word(),
            file.path.display()
        ));
    }
    if let Some(next) = &progress.next {
        out.line(&format!("\nnext     {}", next.summary()));
    }
    if progress.truncated {
        out.line("\n[the changed-file list or the review reached Quinjet's size cap]");
    }
    for warning in &progress.warnings {
        out.line(&format!("note     {warning}"));
    }
    out.finish()
}

pub(crate) fn review_next(step: &ReviewNextStep) -> String {
    let mut out = Report::default();
    match step {
        ReviewNextStep::File { path, state } => {
            out.line(&format!("file    {}", path.display()));
            out.line(&format!("state   {}", state.word()));
        }
        ReviewNextStep::Thread {
            id,
            path,
            line,
            outdated,
            author,
            excerpt,
        } => {
            let location = line.map_or_else(
                || path.display().to_string(),
                |line| format!("{}:{line}", path.display()),
            );
            out.line(&format!("thread  {location}"));
            out.line(&format!("id      {id}"));
            out.line(&format!("from    @{author}"));
            if *outdated {
                out.line("state   outdated by a later commit");
            }
            if !excerpt.is_empty() {
                out.line(&format!("says    {excerpt}"));
            }
        }
    }
    out.finish()
}

fn short_oid(oid: &str) -> String {
    if oid.is_empty() {
        return "an unknown commit".to_owned();
    }
    oid.chars().take(12).collect()
}
