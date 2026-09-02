use super::github::GitHubFixture;
use super::*;

#[doc = " Fake GitHub CLI cases for review progress: one unresolved thread waiting"]
#[doc = " on the reader, and one earlier review of theirs to measure the delta from."]
pub(super) const GH_CASES: &str = r#"  *"reviewThreads(first: 50"*)
    printf '{"data":{"repository":{"pullRequest":{"id":"PR_42","headRefOid":"%s","reviewDecision":"REVIEW_REQUIRED","reviews":{"nodes":[]},"reviewThreads":{"nodes":[{"id":"THREAD_1","path":"feature.txt","diffSide":"RIGHT","line":1,"originalLine":1,"startDiffSide":null,"startLine":null,"originalStartLine":null,"subjectType":"LINE","isResolved":false,"isOutdated":false,"resolvedBy":null,"viewerCanReply":true,"viewerCanResolve":true,"viewerCanUnresolve":false,"comments":{"totalCount":1,"nodes":[{"id":"COMMENT_1","author":{"login":"hubot"},"body":"Please rename this file","createdAt":"2026-08-21T03:00:00Z","updatedAt":"2026-08-21T03:00:00Z","url":"https://github.com/acme/project/pull/42","state":"SUBMITTED","viewerDidAuthor":false,"viewerCanUpdate":false,"viewerCanDelete":false}]}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}}' "$FAKE_HEAD_OID"
    ;;
  *"latestReviews(last: 50)"*)
    printf '{"data":{"viewer":{"login":"octocat"},"repository":{"pullRequest":{"latestReviews":{"nodes":[{"state":"COMMENTED","submittedAt":"2026-08-20T05:00:00Z","author":{"login":"octocat"},"commit":{"oid":"%s"}}]}}}}}' "$FAKE_BASE_OID"
    ;;
"#;

#[test]
fn review_progress_measures_what_is_left_against_your_last_review() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture
        .read(&["pr", "reviews", "progress", "42"])?
        .success()?;

    ensure!(
        plain
            .stdout
            .starts_with("#42  0 of 1 files read  ·  1 unresolved"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain.stdout.contains("since    your last review"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain.stdout.contains("commits  1 new since then"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain
            .stdout
            .contains("threads  1 awaiting your reply, 0 awaiting others, 0 outdated"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain.stdout.contains("unviewed  feature.txt *"),
        "{}",
        plain.stdout
    );
    ensure!(
        plain.stdout.contains("next     unviewed feature.txt"),
        "{}",
        plain.stdout
    );
    Ok(())
}

#[test]
fn review_progress_json_carries_the_files_threads_and_next_step() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "reviews", "progress", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["repository"] == "acme/project");
    ensure!(value["number"] == 42);
    ensure!(value["headOid"] == fixture.head_oid);
    ensure!(value["since"]["oid"] == fixture.base_oid);
    ensure!(value["since"]["source"] == "review");
    ensure!(value["since"]["detail"] == "COMMENTED");
    ensure!(value["viewed"] == 0);
    ensure!(value["remaining"] == 1);
    ensure!(value["changedSince"] == 1);
    ensure!(value["files"][0]["path"] == "feature.txt");
    ensure!(value["files"][0]["state"] == "unviewed");
    ensure!(value["files"][0]["changedSince"] == true);
    ensure!(value["threads"]["unresolved"] == 1);
    ensure!(value["threads"]["awaitingYourReply"] == 1);
    ensure!(
        value["newCommits"]
            .as_array()
            .is_some_and(|new| new.len() == 1)
    );
    ensure!(value["next"]["kind"] == "file");
    ensure!(value["next"]["path"] == "feature.txt");
    ensure!(value["threadStep"]["kind"] == "thread");
    Ok(())
}

#[test]
fn marking_a_file_read_moves_the_next_step_on_to_the_threads() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let marked = fixture
        .run(&["pr", "reviews", "viewed", "42", "feature.txt"])?
        .success()?;
    ensure!(
        marked.stdout.contains("Marked 1 file(s) as read"),
        "{}",
        marked.stdout
    );

    let value = fixture
        .read(&["pr", "reviews", "progress", "42", "--json"])?
        .success()?
        .json()?;
    ensure!(value["viewed"] == 1);
    ensure!(value["remaining"] == 0);
    ensure!(value["files"][0]["state"] == "viewed");
    ensure!(value["next"]["kind"] == "thread");
    ensure!(value["next"]["id"] == "THREAD_1");
    ensure!(value["next"]["author"] == "hubot");
    ensure!(value["next"]["excerpt"] == "Please rename this file");
    Ok(())
}

#[test]
fn a_file_read_at_an_older_commit_reopens_when_it_moves() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let base = fixture.base_oid.clone();
    fixture
        .run_at_head(&["pr", "reviews", "viewed", "42", "feature.txt"], &base)?
        .success()?;

    let value = fixture
        .read(&["pr", "reviews", "progress", "42", "--refresh", "--json"])?
        .success()?
        .json()?;

    ensure!(
        value["files"][0]["state"] == "changed-since-viewed",
        "{value}"
    );
    ensure!(value["files"][0]["viewedAtOid"] == fixture.base_oid);
    ensure!(value["viewed"] == 0);
    ensure!(value["changedSinceViewed"] == 1);
    ensure!(value["remaining"] == 1);
    ensure!(value["next"]["state"] == "changed-since-viewed");
    Ok(())
}

#[test]
fn unmarking_and_resetting_return_the_pull_request_to_unread() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture
        .run(&["pr", "reviews", "viewed", "42", "--all"])?
        .success()?;
    ensure!(
        fixture
            .read(&["pr", "reviews", "progress", "42", "--json"])?
            .json()?["viewed"]
            == 1
    );

    fixture
        .run(&["pr", "reviews", "viewed", "42", "feature.txt", "--unviewed"])?
        .success()?;
    ensure!(
        fixture
            .read(&["pr", "reviews", "progress", "42", "--json"])?
            .json()?["viewed"]
            == 0
    );

    fixture
        .run(&["pr", "reviews", "viewed", "42", "--all"])?
        .success()?;
    let reset = fixture
        .run(&["pr", "reviews", "viewed", "42", "--reset"])?
        .success()?;
    ensure!(
        reset.stdout.contains("Cleared local review progress"),
        "{}",
        reset.stdout
    );
    ensure!(
        fixture
            .read(&["pr", "reviews", "progress", "42", "--json"])?
            .json()?["viewed"]
            == 0
    );
    Ok(())
}

#[test]
fn recording_a_visit_makes_the_delta_measure_from_it() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let visit = fixture.run(&["pr", "reviews", "visit", "42"])?.success()?;
    ensure!(
        visit.stdout.contains("Recorded a visit to #42"),
        "{}",
        visit.stdout
    );

    let value = fixture
        .read(&["pr", "reviews", "progress", "42", "--json"])?
        .json()?;
    ensure!(value["since"]["source"] == "review", "{value}");
    Ok(())
}

#[test]
fn review_next_selects_between_files_and_threads() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let file = fixture.read(&["pr", "reviews", "next", "42"])?.success()?;
    ensure!(
        file.stdout.contains("file    feature.txt"),
        "{}",
        file.stdout
    );
    ensure!(file.stdout.contains("state   unviewed"), "{}", file.stdout);

    let thread = fixture
        .read(&["pr", "reviews", "next", "42", "--threads"])?
        .success()?;
    ensure!(
        thread.stdout.contains("thread  feature.txt:1"),
        "{}",
        thread.stdout
    );
    ensure!(
        thread.stdout.contains("id      THREAD_1"),
        "{}",
        thread.stdout
    );
    ensure!(
        thread.stdout.contains("from    @hubot"),
        "{}",
        thread.stdout
    );
    ensure!(
        thread.stdout.contains("says    Please rename this file"),
        "{}",
        thread.stdout
    );

    let json = fixture
        .read(&["pr", "reviews", "next", "42", "--files", "--json"])?
        .json()?;
    ensure!(json["kind"] == "file");
    ensure!(json["path"] == "feature.txt");
    Ok(())
}

#[test]
fn review_next_says_when_there_is_nothing_left() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture
        .run(&["pr", "reviews", "viewed", "42", "--all"])?
        .success()?;

    let run = fixture
        .read(&["pr", "reviews", "next", "42", "--files"])?
        .success()?;

    ensure!(run.stdout == "Nothing left to review\n", "{}", run.stdout);
    ensure!(
        fixture
            .read(&["pr", "reviews", "next", "42", "--files", "--json"])?
            .json()?["next"]
            .is_null()
    );
    Ok(())
}

#[test]
fn pull_request_diff_since_review_prints_only_the_delta() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let delta = fixture
        .read(&["pr", "diff", "42", "--since-review"])?
        .success()?;

    ensure!(delta.stdout.contains("feature.txt"), "{}", delta.stdout);
    ensure!(
        delta.stdout.contains("+from pull request"),
        "{}",
        delta.stdout
    );

    let json = fixture
        .read(&["pr", "diff", "42", "--since-review", "--json"])?
        .json()?;
    let title = json["title"].as_str().unwrap_or_default();
    ensure!(title.starts_with("PR #42 since "), "{title}");
    ensure!(title.contains("your last review"), "{title}");
    Ok(())
}

#[test]
fn pull_request_diff_since_an_explicit_commit_resolves_it_in_the_pull_request() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let json = fixture
        .read(&["pr", "diff", "42", "--since", &fixture.base_oid, "--json"])?
        .json()?;
    ensure!(
        json["title"]
            .as_str()
            .is_some_and(|title| title.contains("the commit you named")),
        "{json}"
    );

    let unknown = fixture.read(&["pr", "diff", "42", "--since", "deadbeef"])?;
    ensure!(unknown.code == 3, "expected 3, got {}", unknown.code);
    ensure!(
        unknown.stderr.contains("does not name a commit"),
        "{}",
        unknown.stderr
    );
    Ok(())
}
