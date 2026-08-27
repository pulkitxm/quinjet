use super::*;

#[test]
fn stack_activation_warms_selected_member_checks_and_conversations_first() {
    let mut app = App::new("/tmp/repo", "repo");
    let mut effects = Vec::new();

    app.apply_pull_request_stack_snapshot(Some(pull_request_stack(2)), &mut effects);

    let pull_requests = effects
        .iter()
        .find_map(|effect| match effect {
            AppEffect::Git(command) => match command.as_ref() {
                WorkerCommand::PrefetchPullRequestStackMembers { pull_requests, .. } => {
                    Some(pull_requests)
                }
                _ => None,
            },
            _ => None,
        })
        .expect("stack warm plan");
    assert_eq!(
        pull_requests
            .iter()
            .map(|pull_request| pull_request.number)
            .collect::<Vec<_>>(),
        vec![42, 41, 43]
    );
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command)
            if matches!(
                command.as_ref(),
                WorkerCommand::PreparePullRequestStack { .. }
                    | WorkerCommand::LoadPullRequestStackMemberCommits { .. }
            )
    )));
}

#[test]
fn unchanged_stack_does_not_restart_background_warming() {
    let mut app = App::new("/tmp/repo", "repo");
    let stack = pull_request_stack(2);
    app.apply_pull_request_stack_snapshot(Some(stack.clone()), &mut Vec::new());
    let mut effects = Vec::new();

    app.apply_pull_request_stack_snapshot(Some(stack), &mut effects);

    assert!(effects.iter().all(|effect| !matches!(
        effect,
        AppEffect::Git(command)
            if matches!(
                command.as_ref(),
                WorkerCommand::PrefetchPullRequestStackMembers { .. }
            )
    )));
}

#[test]
fn stack_teardown_cancels_background_warming() {
    let mut app = App::new("/tmp/repo", "repo");
    app.pull_request = Some(pull_request(42, "Root", "acme/widget"));
    app.apply_pull_request_stack_snapshot(Some(pull_request_stack(2)), &mut Vec::new());
    let mut effects = Vec::new();

    app.apply_pull_request_stack_snapshot(None, &mut effects);

    assert!(effects.iter().any(|effect| matches!(
        effect,
        AppEffect::Git(command)
            if matches!(
                command.as_ref(),
                WorkerCommand::PrefetchPullRequestStackMembers { pull_requests, .. }
                    if pull_requests.is_empty()
            )
    )));
}
