use super::*;

#[test]
fn pull_request_commits_name_empty_and_truncated_results() {
    assert_eq!(
        pull_request_commits(&PullRequestCommits::default()),
        "No commits reported\n"
    );

    let result = PullRequestCommits {
        commits: vec![PullRequestCommit {
            abbreviated_oid: "abcdef123456".to_owned(),
            subject: "feat: add the stack inspector".to_owned(),
            author: "Ada Lovelace".to_owned(),
            authored_at: "2026-08-21T02:00:00Z".to_owned(),
            ..PullRequestCommit::default()
        }],
        total_commits: 600,
        truncated: true,
        ..PullRequestCommits::default()
    };
    let text = pull_request_commits(&result);

    assert!(text.contains("abcdef123456"), "{text}");
    assert!(text.contains("Ada Lovelace"), "{text}");
    assert!(text.contains("feat: add the stack inspector"), "{text}");
    assert!(text.contains("1 newest commits shown of 600"), "{text}");
}
