#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum GateVerdict {
    Pass,
    Pending,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GateReason {
    category: &'static str,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PullRequestGate {
    verdict: GateVerdict,
    number: u64,
    head_oid: String,
    blockers: Vec<GateReason>,
    pending: Vec<GateReason>,
    from_cache: bool,
    review_truncated: bool,
}

impl PullRequestGate {
    fn evaluate(
        snapshot: &PullRequestSnapshot,
        checks: &[PullRequestCheck],
        unresolved_threads: usize,
        review_truncated: bool,
    ) -> Self {
        let request = &snapshot.pull_request;
        let action = &request.action_state;
        let mut blockers = Vec::new();
        let mut pending = Vec::new();
        if request.state != "OPEN" {
            blockers.push(reason(
                "state",
                format!("pull request is {}", request.state.to_ascii_lowercase()),
            ));
        }
        if request.is_draft {
            blockers.push(reason("state", "pull request is a draft"));
        }
        for check in checks {
            let destination = match check.status {
                PullRequestCheckStatus::Pending => &mut pending,
                PullRequestCheckStatus::Failed
                | PullRequestCheckStatus::Cancelled
                | PullRequestCheckStatus::Unknown => &mut blockers,
                PullRequestCheckStatus::Passed | PullRequestCheckStatus::Skipped => continue,
            };
            destination.push(reason(
                "ci",
                format!("{} is {}", check.name, check.state.to_ascii_lowercase()),
            ));
        }
        match action.review_decision.as_str() {
            "CHANGES_REQUESTED" => blockers.push(reason("review", "changes were requested")),
            "REVIEW_REQUIRED" => {
                blockers.push(reason("approval", "an approving review is required"));
            }
            _ => {}
        }
        if unresolved_threads > 0 {
            blockers.push(reason(
                "review",
                format!("{unresolved_threads} unresolved review threads"),
            ));
        }
        if action.mergeable == "CONFLICTING" || action.merge_state == "DIRTY" {
            blockers.push(reason("conflict", "head conflicts with the base branch"));
        }
        match action.merge_state.as_str() {
            "BEHIND" => blockers.push(reason("branch", "head is behind the base branch")),
            "BLOCKED" => blockers.push(reason(
                "ruleset",
                "a branch protection rule is not satisfied",
            )),
            "UNKNOWN" | "UNSTABLE" => pending.push(reason(
                "merge",
                "GitHub has not produced a stable merge state",
            )),
            _ => {}
        }
        if !action.merge_queue_entry_id.is_empty()
            && !matches!(action.merge_queue_state.as_str(), "MERGEABLE" | "READY")
        {
            pending.push(reason(
                "queue",
                format!(
                    "merge queue entry is {}",
                    action.merge_queue_state.to_ascii_lowercase()
                ),
            ));
        }
        let verdict = if blockers.is_empty() {
            if pending.is_empty() {
                GateVerdict::Pass
            } else {
                GateVerdict::Pending
            }
        } else {
            GateVerdict::Blocked
        };
        Self {
            verdict,
            number: request.number,
            head_oid: request.head_oid.clone(),
            blockers,
            pending,
            from_cache: snapshot.from_cache,
            review_truncated,
        }
    }

    const fn exit_code(&self) -> u8 {
        match self.verdict {
            GateVerdict::Pass => 0,
            GateVerdict::Blocked => EXIT_FAILURE,
            GateVerdict::Pending => GATE_EXIT_PENDING,
        }
    }
}

fn reason(category: &'static str, summary: impl Into<String>) -> GateReason {
    GateReason {
        category,
        summary: summary.into(),
    }
}

fn read_gate(session: &mut Session, out: &Emitter, args: &PrArgs) -> Result<PullRequestGate> {
    let snapshot = lookup_snapshot(session, out, args)?;
    report_warnings(out, &snapshot);
    let checks = session
        .execute(Command::PullRequestChecks {
            pull_request: Box::new(snapshot.pull_request.clone()),
            refresh: args.refresh,
        })?
        .checks()?;
    let review = session
        .execute(Command::PullRequestReview {
            pull_request: Box::new(snapshot.pull_request.clone()),
        })?
        .review()?;
    Ok(PullRequestGate::evaluate(
        &snapshot,
        &checks.checks,
        review.unresolved_count(),
        review.truncated,
    ))
}

pub(super) fn gate(session: &mut Session, out: &Emitter, args: &PrGateArgs) -> Result<u8> {
    if args.watch {
        return watch::run(interval(args.interval, CHECK_WATCH_FLOOR), out.json, || {
            let mut lookup_args = args.pull_request.clone();
            lookup_args.refresh = true;
            let gate = read_gate(session, out, &lookup_args)?;
            Ok(watch::Frame {
                text: render_gate(&gate),
                finished: gate.verdict != GateVerdict::Pending,
                code: gate.exit_code(),
                value: gate,
            })
        });
    }
    let gate = read_gate(session, out, &args.pull_request)?;
    out.emit(&gate, || render_gate(&gate))?;
    Ok(gate.exit_code())
}

fn render_gate(gate: &PullRequestGate) -> String {
    let mut text = format!(
        "{}\n",
        match gate.verdict {
            GateVerdict::Pass => "pass",
            GateVerdict::Pending => "pending",
            GateVerdict::Blocked => "blocked",
        }
    );
    for blocker in &gate.blockers {
        text.push_str("  ");
        text.push_str(blocker.category);
        text.push_str(": ");
        text.push_str(&blocker.summary);
        text.push('\n');
    }
    for pending in &gate.pending {
        text.push_str("  ");
        text.push_str(pending.category);
        text.push_str(": ");
        text.push_str(&pending.summary);
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::github::PullRequestActionState;

    fn snapshot() -> PullRequestSnapshot {
        let repository = GitHubRepository {
            name_with_owner: "acme/rocket".to_owned(),
            url: "https://github.com/acme/rocket".to_owned(),
            remotes: vec!["origin".to_owned()],
        };
        PullRequestSnapshot {
            repositories: vec![repository.clone()],
            selected_repository: Some(repository.clone()),
            pull_request: PullRequest {
                number: 42,
                title: "Launch".to_owned(),
                description: String::new(),
                author: "octocat".to_owned(),
                state: "OPEN".to_owned(),
                is_draft: false,
                created_at: String::new(),
                updated_at: String::new(),
                url: "https://github.com/acme/rocket/pull/42".to_owned(),
                base_ref: "main".to_owned(),
                base_oid: "base".to_owned(),
                head_ref: "feature".to_owned(),
                head_oid: "head".to_owned(),
                base_repository: repository,
                head_repository: None,
                head_remotes: Vec::new(),
                is_cross_repository: false,
                additions: 1,
                deletions: 0,
                changed_files: 1,
                action_state: PullRequestActionState::default(),
            },
            warnings: Vec::new(),
            exact_number: Some(42),
            from_cache: false,
        }
    }

    fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
        PullRequestCheck {
            name: name.to_owned(),
            workflow: "CI".to_owned(),
            state: format!("{status:?}"),
            status,
            description: String::new(),
            link: String::new(),
            started_at: String::new(),
            completed_at: String::new(),
        }
    }

    #[test]
    fn passing_gate_has_no_reasons_and_exits_zero() {
        let gate = PullRequestGate::evaluate(
            &snapshot(),
            &[check("linux", PullRequestCheckStatus::Passed)],
            0,
            false,
        );
        assert_eq!(gate.verdict, GateVerdict::Pass);
        assert_eq!(gate.exit_code(), 0);
        assert_eq!(render_gate(&gate), "pass\n");
    }

    #[test]
    fn pending_checks_have_a_distinct_verdict_and_exit_code() {
        let gate = PullRequestGate::evaluate(
            &snapshot(),
            &[check("windows", PullRequestCheckStatus::Pending)],
            0,
            false,
        );
        assert_eq!(gate.verdict, GateVerdict::Pending);
        assert_eq!(gate.exit_code(), GATE_EXIT_PENDING);
        assert!(render_gate(&gate).contains("ci: windows is pending"));
    }

    #[test]
    fn gate_explains_independent_blockers() {
        let mut snapshot = snapshot();
        snapshot.pull_request.action_state.review_decision = "CHANGES_REQUESTED".to_owned();
        snapshot.pull_request.action_state.merge_state = "BEHIND".to_owned();
        let gate = PullRequestGate::evaluate(
            &snapshot,
            &[check("windows", PullRequestCheckStatus::Failed)],
            2,
            false,
        );
        assert_eq!(gate.verdict, GateVerdict::Blocked);
        assert_eq!(gate.exit_code(), EXIT_FAILURE);
        let text = render_gate(&gate);
        assert!(text.contains("ci: windows is failed"));
        assert!(text.contains("review: changes were requested"));
        assert!(text.contains("review: 2 unresolved review threads"));
        assert!(text.contains("branch: head is behind the base branch"));
    }
}
