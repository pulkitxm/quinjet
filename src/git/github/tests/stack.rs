use super::*;

fn stack_response(entries: &str, selected_position: usize, size: usize) -> Vec<u8> {
    format!(
        r#"{{"data":{{"repository":{{"pullRequest":{{"stackEntry":{{"position":{selected_position}}},"stack":{{"id":"STACK_node","number":12,"size":{size},"baseRefName":"main","entries":{{"totalCount":{size},"nodes":[{entries}]}}}}}}}}}}}}"#
    )
    .into_bytes()
}

fn entry(position: usize, number: u64, base: &str, head: &str) -> String {
    format!(
        r#"{{"id":"ENTRY_{position}","position":{position},"pullRequest":{{"id":"PR_{number}","number":{number},"title":"Layer {position}","author":{{"login":"octocat"}},"state":"OPEN","isDraft":false,"updatedAt":"2026-08-26T00:00:00Z","url":"https://github.com/acme/widget/pull/{number}","baseRefName":"base-{position}","baseRefOid":"{base}","headRefName":"head-{position}","headRefOid":"{head}","headRepository":{{"nameWithOwner":"acme/widget"}},"isCrossRepository":false,"additions":10,"deletions":2,"changedFiles":3,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","reviewDecision":"APPROVED","mergeQueueEntry":null,"commits":{{"nodes":[{{"commit":{{"statusCheckRollup":{{"state":"SUCCESS"}}}}}}]}}}}}}"#
    )
}

#[test]
fn stack_response_sorts_members_and_builds_exact_comparison() {
    let base = "1".repeat(40);
    let middle = "2".repeat(40);
    let head = "3".repeat(40);
    let entries = format!(
        "{},{}",
        entry(2, 42, &middle, &head),
        entry(1, 41, &base, &middle)
    );
    let repository = repository("acme/widget", "https://github.com/acme/widget", &["origin"]);
    let stack = parse_pull_request_stack(&stack_response(&entries, 2, 2), &repository, 42)
        .unwrap()
        .unwrap();

    assert_eq!(stack.number, 12);
    assert_eq!(stack.selected_position, 2);
    assert_eq!(
        stack
            .members
            .iter()
            .map(|member| member.number)
            .collect::<Vec<_>>(),
        vec![41, 42]
    );
    assert!(!stack.truncated);
    let comparison = stack.comparison(1, 2).unwrap();
    assert_eq!(comparison.base_oid, base);
    assert_eq!(comparison.head_oid, head);
    assert_eq!(comparison.base_ref, "base-1");
    assert_eq!(comparison.head_ref, "head-2");
    assert_eq!(comparison.number, 42);
}

#[test]
fn standalone_pull_request_is_a_successful_empty_stack() {
    let response = br#"{"data":{"repository":{"pullRequest":{"stackEntry":null,"stack":null}}}}"#;
    let repository = repository("acme/widget", "https://github.com/acme/widget", &[]);

    assert_eq!(
        parse_pull_request_stack(response, &repository, 42).unwrap(),
        None
    );
}

#[test]
fn incomplete_and_duplicate_entries_are_not_silently_trusted() {
    let base = "1".repeat(40);
    let head = "2".repeat(40);
    let one = entry(1, 41, &base, &head);
    let repository = repository("acme/widget", "https://github.com/acme/widget", &[]);
    let incomplete = parse_pull_request_stack(
        &stack_response(&format!("{one},null"), 1, 2),
        &repository,
        41,
    )
    .unwrap()
    .unwrap();
    assert!(incomplete.truncated);

    let duplicate = stack_response(&format!("{one},{one}"), 1, 2);
    let error = parse_pull_request_stack(&duplicate, &repository, 41)
        .unwrap_err()
        .to_string();
    assert!(error.contains("duplicate or invalid"));
}

#[test]
fn graphql_errors_are_reported_as_stack_failures() {
    let repository = repository("acme/widget", "https://github.com/acme/widget", &[]);
    let response = br#"{"data":null,"errors":[{"message":"stack preview unavailable"}]}"#;
    let error = parse_pull_request_stack(response, &repository, 42)
        .unwrap_err()
        .to_string();

    assert!(error.contains("stack preview unavailable"));
}
