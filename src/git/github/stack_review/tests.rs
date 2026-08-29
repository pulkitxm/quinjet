use super::*;

fn gate(verdict: MergeGateVerdict, blockers: Vec<MergeGateBlocker>) -> MergeGate {
    MergeGate {
        schema_version: MergeGate::SCHEMA_VERSION,
        repository: "acme/project".to_owned(),
        number: 0,
        title: String::new(),
        url: String::new(),
        state: "OPEN".to_owned(),
        is_draft: false,
        verdict,
        blockers,
        checks: MergeGateChecks::default(),
        review: MergeGateReview::default(),
        branch: MergeGateBranch {
            head_oid: "a".repeat(40),
            ..MergeGateBranch::default()
        },
        queue: None,
        auto_merge: MergeGateAutoMerge::default(),
        warnings: Vec::new(),
        from_cache: false,
    }
}

fn blocker(kind: MergeGateBlockerKind, summary: &str) -> MergeGateBlocker {
    MergeGateBlocker {
        kind,
        summary: summary.to_owned(),
        details: Vec::new(),
    }
}

fn member(position: usize, verdict: MergeGateVerdict, paths: &[&str]) -> StackReviewMemberInputs {
    StackReviewMemberInputs {
        position,
        number: 40 + position as u64,
        title: format!("Member {position}"),
        url: String::new(),
        selected: position == 1,
        gate: gate(verdict, Vec::new()),
        paths: Some(paths.iter().map(PathBuf::from).collect()),
        additions: position,
        deletions: 0,
    }
}

fn review(members: Vec<StackReviewMemberInputs>) -> StackReview {
    let size = members.len();
    build_stack_review(StackReviewInputs {
        number: 41,
        base_ref: "main".to_owned(),
        size,
        selected_position: 1,
        members,
        truncated: false,
        warnings: Vec::new(),
    })
}

#[test]
fn the_merge_order_stops_at_the_first_member_that_cannot_merge() {
    let stack = review(vec![
        member(1, MergeGateVerdict::Mergeable, &[]),
        member(2, MergeGateVerdict::Blocked, &[]),
        member(3, MergeGateVerdict::Mergeable, &[]),
    ]);

    assert_eq!(stack.merge_order, [1]);
    assert_eq!(stack.critical_position, Some(2));
    assert!(!stack.is_clear());
}

#[test]
fn a_merged_member_does_not_stop_the_order_that_follows_it() {
    let stack = review(vec![
        member(1, MergeGateVerdict::Merged, &[]),
        member(2, MergeGateVerdict::Mergeable, &[]),
    ]);

    assert_eq!(stack.merge_order, [1, 2]);
    assert_eq!(stack.critical_position, None);
    assert!(stack.is_clear());
    assert_eq!(stack.critical_path, Vec::<usize>::new());
}

#[test]
fn a_clear_member_above_a_blocked_one_is_waiting_rather_than_working() {
    let stack = review(vec![
        member(1, MergeGateVerdict::Blocked, &[]),
        member(2, MergeGateVerdict::Mergeable, &[]),
        member(3, MergeGateVerdict::Mergeable, &[]),
    ]);

    assert_eq!(stack.downstream_blocked, [2, 3]);
    assert_eq!(stack.critical_path, [1, 2, 3]);
    let sources: Vec<StackBlockSource> = stack
        .members
        .iter()
        .map(|member| member.block_source)
        .collect();
    assert_eq!(
        sources,
        [
            StackBlockSource::Own,
            StackBlockSource::Downstream,
            StackBlockSource::Downstream
        ]
    );
}

#[test]
fn a_member_below_the_critical_one_is_neither_blocked_nor_waiting() {
    let stack = review(vec![
        member(1, MergeGateVerdict::Mergeable, &[]),
        member(2, MergeGateVerdict::Blocked, &[]),
    ]);

    assert_eq!(
        stack.members.first().map(|member| member.block_source),
        Some(StackBlockSource::None)
    );
    assert_eq!(stack.downstream_blocked, Vec::<usize>::new());
}

#[test]
fn members_are_reviewed_in_stack_order_however_they_arrive() {
    let stack = review(vec![
        member(3, MergeGateVerdict::Mergeable, &[]),
        member(1, MergeGateVerdict::Mergeable, &[]),
        member(2, MergeGateVerdict::Mergeable, &[]),
    ]);

    let positions: Vec<usize> = stack.members.iter().map(|member| member.position).collect();
    assert_eq!(positions, [1, 2, 3]);
    assert_eq!(stack.merge_order, [1, 2, 3]);
}

#[test]
fn a_path_two_members_touch_is_reported_and_one_only_one_touches_is_not() {
    let stack = review(vec![
        member(
            1,
            MergeGateVerdict::Mergeable,
            &["src/lib.rs", "src/one.rs"],
        ),
        member(
            2,
            MergeGateVerdict::Mergeable,
            &["src/lib.rs", "src/two.rs"],
        ),
        member(3, MergeGateVerdict::Mergeable, &["src/two.rs"]),
    ]);

    let overlaps: Vec<(String, Vec<usize>)> = stack
        .duplicated_paths
        .iter()
        .map(|entry| (entry.path.display().to_string(), entry.positions.clone()))
        .collect();
    assert_eq!(
        overlaps,
        [
            ("src/lib.rs".to_owned(), vec![1, 2]),
            ("src/two.rs".to_owned(), vec![2, 3])
        ]
    );
}

#[test]
fn a_member_whose_incremental_comparison_failed_contributes_no_paths() {
    let mut first = member(1, MergeGateVerdict::Mergeable, &["src/lib.rs"]);
    first.paths = None;
    let stack = review(vec![
        first,
        member(2, MergeGateVerdict::Mergeable, &["src/lib.rs"]),
    ]);

    assert_eq!(stack.duplicated_paths, Vec::new());
    assert_eq!(
        stack.members.first().map(|member| member.changed_files),
        Some(0)
    );
}

#[test]
fn the_earliest_failing_check_is_the_one_lowest_in_merge_order() {
    let mut lower = member(1, MergeGateVerdict::Blocked, &[]);
    lower.gate.checks.checks = vec![failing_check("windows")];
    let mut upper = member(2, MergeGateVerdict::Blocked, &[]);
    upper.gate.checks.checks = vec![failing_check("macos")];

    let stack = review(vec![upper, lower]);

    let failure = stack
        .earliest_failing_check
        .expect("one failing check is reported");
    assert_eq!(failure.position, 1);
    assert_eq!(failure.number, 41);
    assert_eq!(failure.check, "CI / windows");
}

#[test]
fn a_green_stack_reports_no_failing_check_at_all() {
    let stack = review(vec![member(1, MergeGateVerdict::Mergeable, &[])]);

    assert_eq!(stack.earliest_failing_check, None);
    assert!(stack.is_clear());
}

#[test]
fn an_approval_on_an_older_commit_is_named_with_the_reviewer_who_gave_it() {
    let mut only = member(1, MergeGateVerdict::Blocked, &[]);
    only.gate.review.reviews = vec![
        GateReview {
            author: "octocat".to_owned(),
            state: "APPROVED".to_owned(),
            commit_oid: "b".repeat(40),
            stale: true,
        },
        GateReview {
            author: "hubot".to_owned(),
            state: "APPROVED".to_owned(),
            commit_oid: "a".repeat(40),
            stale: false,
        },
        GateReview {
            author: "nobody".to_owned(),
            state: "CHANGES_REQUESTED".to_owned(),
            commit_oid: "b".repeat(40),
            stale: true,
        },
    ];

    let stack = review(vec![only]);

    assert_eq!(stack.stale_approvals, 1);
    let approval = stack
        .members
        .first()
        .and_then(|member| member.stale_approvals.first())
        .expect("one stale approval");
    assert_eq!(approval.reviewer, "octocat");
    assert_eq!(approval.approved_oid, "b".repeat(40));
    assert_eq!(approval.head_oid, "a".repeat(40));
}

#[test]
fn a_members_blockers_are_carried_through_in_the_gates_own_order() {
    let mut only = member(1, MergeGateVerdict::Blocked, &[]);
    only.gate.blockers = vec![
        blocker(MergeGateBlockerKind::Ci, "1 required check failed"),
        blocker(MergeGateBlockerKind::Threads, "2 unresolved threads"),
    ];

    let stack = review(vec![only]);

    let member = stack.members.first().expect("one member");
    assert_eq!(member.blockers.len(), 2);
    assert!(
        member.headline().contains("1 required check failed"),
        "{}",
        member.headline()
    );
}

#[test]
fn a_clear_member_headlines_with_its_verdict_rather_than_nothing() {
    let stack = review(vec![member(1, MergeGateVerdict::Mergeable, &[])]);

    assert_eq!(
        stack.members.first().map(StackReviewMember::headline),
        Some("mergeable".to_owned())
    );
}

#[test]
fn an_empty_stack_is_not_clear_because_there_is_nothing_to_say_so_about() {
    let stack = review(Vec::new());

    assert!(!stack.is_clear());
    assert_eq!(stack.merge_order, Vec::<usize>::new());
    assert_eq!(stack.schema_version, StackReview::SCHEMA_VERSION);
}

fn failing_check(name: &str) -> GateCheck {
    GateCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state: GateCheckState::Failed,
        required: true,
        url: String::new(),
        awaiting_approval: false,
    }
}

fn feedback_item(kind: FeedbackKind, owner: FeedbackOwner, id: &str) -> FeedbackItem {
    FeedbackItem {
        kind,
        id: id.to_owned(),
        path: Some(PathBuf::from("src/lib.rs")),
        line: Some(1),
        author: "hubot".to_owned(),
        summary: "Please look at this".to_owned(),
        body: String::new(),
        url: String::new(),
        owner,
        mine: false,
        action: format!("quinjet pr reviews reply 41 {id}"),
    }
}

fn queue(items: Vec<FeedbackItem>) -> PullRequestFeedback {
    let mut queue = PullRequestFeedback {
        items,
        ..PullRequestFeedback::default()
    };
    queue.finish();
    queue
}

fn stack_feedback(members: Vec<(usize, PullRequestFeedback)>) -> StackFeedback {
    let size = members.len();
    build_stack_feedback(StackFeedbackInputs {
        number: 41,
        size,
        selected_position: 1,
        viewer: "octocat".to_owned(),
        members: members
            .into_iter()
            .map(|(position, queue)| {
                (
                    position,
                    40 + position as u64,
                    format!("Member {position}"),
                    position == 1,
                    queue,
                )
            })
            .collect(),
        truncated: false,
        warnings: Vec::new(),
    })
}

#[test]
fn the_stack_queue_totals_every_members_counts_and_reads_bottom_first() {
    let queue = stack_feedback(vec![
        (
            2,
            queue(vec![feedback_item(
                FeedbackKind::Thread,
                FeedbackOwner::You,
                "THREAD_2",
            )]),
        ),
        (
            1,
            queue(vec![
                feedback_item(FeedbackKind::Thread, FeedbackOwner::You, "THREAD_1"),
                feedback_item(FeedbackKind::Advisory, FeedbackOwner::Others, "NOTE_1"),
            ]),
        ),
    ]);

    let positions: Vec<usize> = queue.members.iter().map(|member| member.position).collect();
    assert_eq!(positions, [1, 2]);
    assert_eq!(queue.counts.blocking, 2);
    assert_eq!(queue.counts.advisory, 1);
    assert_eq!(queue.counts.awaiting_you, 2);
    assert_eq!(queue.counts.awaiting_others, 1);
    assert_eq!(queue.next_position, Some(1));
    assert_eq!(queue.schema_version, StackFeedback::SCHEMA_VERSION);
}

#[test]
fn the_next_blocker_is_the_lowest_members_first_blocking_row() {
    let queue = stack_feedback(vec![
        (
            1,
            queue(vec![feedback_item(
                FeedbackKind::Advisory,
                FeedbackOwner::Nobody,
                "NOTE_1",
            )]),
        ),
        (
            2,
            queue(vec![feedback_item(
                FeedbackKind::Thread,
                FeedbackOwner::You,
                "THREAD_2",
            )]),
        ),
    ]);

    let (position, item) = queue.next_blocker().expect("one blocking row");
    assert_eq!(position, 2);
    assert_eq!(item.id, "THREAD_2");
}

#[test]
fn filtering_the_stack_queue_moves_the_totals_and_the_next_position_together() {
    let queue = stack_feedback(vec![
        (
            1,
            queue(vec![feedback_item(
                FeedbackKind::Advisory,
                FeedbackOwner::Others,
                "NOTE_1",
            )]),
        ),
        (
            2,
            queue(vec![
                feedback_item(FeedbackKind::Thread, FeedbackOwner::You, "THREAD_2"),
                feedback_item(FeedbackKind::Advisory, FeedbackOwner::Others, "NOTE_2"),
            ]),
        ),
    ]);

    let blocking = FeedbackFilter {
        blocking_only: true,
        mine_only: false,
    }
    .apply_stack(queue);

    assert_eq!(blocking.counts.blocking, 1);
    assert_eq!(blocking.counts.advisory, 0);
    assert_eq!(blocking.next_position, Some(2));
    assert_eq!(
        blocking.members.first().map(|member| member.items.len()),
        Some(0)
    );
}

#[test]
fn a_filter_that_removes_every_blocking_row_leaves_no_next_position() {
    let queue = stack_feedback(vec![(
        1,
        queue(vec![feedback_item(
            FeedbackKind::Thread,
            FeedbackOwner::Others,
            "THREAD_1",
        )]),
    )]);

    let mine = FeedbackFilter {
        blocking_only: false,
        mine_only: true,
    }
    .apply_stack(queue);

    assert_eq!(mine.next_position, None);
    assert_eq!(mine.next_blocker(), None);
    assert_eq!(mine.counts.blocking, 0);
}
