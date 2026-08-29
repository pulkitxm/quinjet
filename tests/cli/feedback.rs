use super::github::GitHubFixture;
use super::*;

#[test]
fn the_feedback_queue_combines_reviews_threads_and_check_findings() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "feedback", "42"])?.success()?;

    ensure!(plain.stderr.is_empty(), "{}", plain.stderr);
    for expected in [
        "failure   -       feature.txt:1",
        "thread    you     feature.txt:1",
        "outdated  others  feature.txt:1",
        "advisory  -       README.md:2",
        "next  failure feature.txt:1",
    ] {
        ensure!(
            plain.stdout.contains(expected),
            "missing `{expected}` in:\n{}",
            plain.stdout
        );
    }
    ensure!(
        plain
            .stdout
            .contains("2 blocking, 3 advisory · 1 on you, 1 on others"),
        "{}",
        plain.stdout
    );
    Ok(())
}

#[test]
fn the_feedback_json_names_who_each_row_waits_on_and_what_resolves_it() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let value = fixture
        .read(&["pr", "feedback", "42", "--json"])?
        .success()?
        .json()?;

    ensure!(value["schemaVersion"] == 1);
    ensure!(value["number"] == 42);
    ensure!(value["viewer"] == "octocat");
    let kinds: Vec<String> = value["items"]
        .as_array()
        .context("items must be an array")?
        .iter()
        .map(|item| item["kind"].as_str().unwrap_or_default().to_owned())
        .collect();
    ensure!(
        kinds
            == [
                "failure",
                "thread",
                "outdated-thread",
                "advisory",
                "advisory"
            ],
        "unexpected order: {kinds:?}"
    );
    let thread = &value["items"][1];
    ensure!(thread["id"] == "THREAD_1");
    ensure!(thread["owner"] == "you");
    ensure!(thread["mine"] == false);
    ensure!(thread["author"] == "hubot");
    ensure!(
        thread["action"]
            .as_str()
            .is_some_and(|action| action.contains("quinjet pr reviews reply")),
        "{thread}"
    );
    ensure!(value["items"][2]["owner"] == "others");
    ensure!(value["items"][0]["owner"] == "nobody");
    ensure!(value["counts"]["blocking"] == 2);
    ensure!(value["counts"]["awaitingYou"] == 1);
    Ok(())
}

#[test]
fn the_feedback_filters_narrow_the_rows_and_the_counts_together() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let blocking = fixture
        .read(&["pr", "feedback", "42", "--unresolved", "--json"])?
        .json()?;
    ensure!(
        blocking["items"]
            .as_array()
            .is_some_and(|items| items.len() == 2)
    );
    ensure!(blocking["counts"]["advisory"] == 0);

    let mine = fixture
        .read(&["pr", "feedback", "42", "--mine", "--json"])?
        .json()?;
    ensure!(
        mine["items"]
            .as_array()
            .is_some_and(|items| items.len() == 1)
    );
    ensure!(mine["items"][0]["id"] == "THREAD_1");

    fixture.clear_gh_calls();
    let without_checks = fixture
        .read(&["pr", "feedback", "42", "--no-checks", "--json"])?
        .json()?;
    ensure!(
        without_checks["items"]
            .as_array()
            .is_some_and(|items| items.len() == 2)
    );
    ensure!(
        !fixture
            .gh_calls()?
            .contains("check-runs/123456/annotations"),
        "--no-checks still read the annotations"
    );
    Ok(())
}

#[test]
fn the_feedback_exit_code_reports_only_what_the_merge_waits_on() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let blocking = fixture.read(&["pr", "feedback", "42", "--exit-code"])?;
    ensure!(blocking.code == 1, "expected 1, got {}", blocking.code);
    ensure!(
        !blocking.stdout.is_empty(),
        "a verdict still prints its rows"
    );

    let advisory = fixture.read(&["pr", "feedback", "42", "--mine", "--exit-code"])?;
    ensure!(advisory.code == 1, "expected 1, got {}", advisory.code);

    let clean = fixture.read(&["pr", "feedback", "42"])?;
    ensure!(clean.code == 0, "expected 0, got {}", clean.code);
    Ok(())
}

#[test]
fn the_full_feedback_face_prints_the_body_and_the_action() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&["pr", "feedback", "42", "--full"])?
        .success()?;

    ensure!(
        run.stdout.contains("      Please rename this file"),
        "{}",
        run.stdout
    );
    ensure!(
        run.stdout.contains("      Left over from an older push"),
        "{}",
        run.stdout
    );
    ensure!(run.stdout.contains("      -> "), "{}", run.stdout);
    Ok(())
}

#[test]
fn suggestions_list_with_their_range_and_whether_they_can_be_applied() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let plain = fixture.read(&["pr", "suggestions", "42"])?.success()?;

    ensure!(plain.stdout.contains("COMMENT_1"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("feature.txt:1"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("+1 -1"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("@hubot"), "{}", plain.stdout);
    ensure!(plain.stdout.contains("ready"), "{}", plain.stdout);
    ensure!(
        plain.stdout.contains("1 ready to apply, 0 blocked"),
        "{}",
        plain.stdout
    );

    let json = fixture
        .read(&["pr", "suggestions", "42", "--json"])?
        .json()?;
    ensure!(json["schemaVersion"] == 1);
    ensure!(json["applicable"] == 1);
    ensure!(json["suggestions"][0]["id"] == "COMMENT_1");
    ensure!(json["suggestions"][0]["path"] == "feature.txt");
    ensure!(json["suggestions"][0]["startLine"] == 1);
    ensure!(json["suggestions"][0]["endLine"] == 1);
    ensure!(json["suggestions"][0]["replacement"] == "from pull request, renamed");
    ensure!(json["suggestions"][0]["comment"] == "Please rename this file");
    ensure!(json["suggestions"][0]["blocker"].is_null());
    Ok(())
}

#[test]
fn applying_a_suggestion_previews_first_and_changes_nothing() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture.repository.git(&["switch", "feature"])?;

    let preview = fixture
        .run(&["pr", "suggestions", "apply", "42", "COMMENT_1"])?
        .success()?;

    ensure!(
        preview
            .stdout
            .starts_with("Would apply 1 suggestion(s) across 1 file(s), +1 -1"),
        "{}",
        preview.stdout
    );
    ensure!(
        preview.stdout.contains("feature.txt  +1 -1"),
        "{}",
        preview.stdout
    );
    ensure!(preview.stdout.contains("Pass --yes"), "{}", preview.stdout);
    ensure!(
        fs::read_to_string(fixture.repository.path.join("feature.txt"))? == "from pull request\n",
        "the preview wrote to the working tree"
    );
    Ok(())
}

#[test]
fn applying_a_suggestion_writes_it_and_can_record_it_as_one_commit() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture.repository.git(&["switch", "feature"])?;
    let before = fixture.repository.git(&["rev-parse", "HEAD"])?;

    let run = fixture
        .run(&[
            "pr",
            "suggestions",
            "apply",
            "42",
            "COMMENT_1",
            "--message",
            "fix: address review",
            "--yes",
        ])?
        .success()?;

    ensure!(run.stdout.contains("and committed them"), "{}", run.stdout);
    ensure!(
        fs::read_to_string(fixture.repository.path.join("feature.txt"))?
            == "from pull request, renamed\n",
        "the suggestion was not written"
    );
    let after = fixture.repository.git(&["rev-parse", "HEAD"])?;
    ensure!(after != before, "nothing was committed");
    ensure!(
        fixture
            .repository
            .git(&["log", "-1", "--format=%s"])?
            .contains("fix: address review"),
        "the commit message was not used"
    );
    Ok(())
}

#[test]
fn applying_all_suggestions_skips_the_ones_that_cannot_be_applied() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture.repository.git(&["switch", "feature"])?;

    let run = fixture
        .run(&["pr", "suggestions", "apply", "42", "--all", "--yes"])?
        .success()?;

    ensure!(
        run.stdout.starts_with("Applied 1 suggestion(s)"),
        "{}",
        run.stdout
    );
    ensure!(
        fs::read_to_string(fixture.repository.path.join("feature.txt"))?
            == "from pull request, renamed\n"
    );
    Ok(())
}

#[test]
fn applying_a_suggestion_is_refused_when_the_worktree_is_not_the_head() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.run(&["pr", "suggestions", "apply", "42", "COMMENT_1", "--yes"])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(
        run.stderr.contains("check the branch out first"),
        "{}",
        run.stderr
    );
    ensure!(
        fs::read_to_string(fixture.repository.path.join("feature.txt")).is_err(),
        "the file was written on the wrong branch"
    );
    Ok(())
}

#[test]
fn applying_a_suggestion_is_refused_when_the_file_has_uncommitted_changes() -> Result<()> {
    let fixture = GitHubFixture::new()?;
    fixture.repository.git(&["switch", "feature"])?;
    fixture
        .repository
        .write("feature.txt", "edited locally\n")?;

    let run = fixture.run(&["pr", "suggestions", "apply", "42", "COMMENT_1", "--yes"])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(run.stderr.contains("uncommitted changes"), "{}", run.stderr);
    ensure!(
        fs::read_to_string(fixture.repository.path.join("feature.txt"))? == "edited locally\n",
        "the local edit was clobbered"
    );
    Ok(())
}

#[test]
fn applying_an_unknown_suggestion_id_names_what_it_could_have_been() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.run(&["pr", "suggestions", "apply", "42", "nothing", "--yes"])?;

    ensure!(run.code == 3, "expected 3, got {}", run.code);
    ensure!(
        run.stderr.contains("no suggestion on this pull request"),
        "{}",
        run.stderr
    );
    ensure!(
        run.stderr.contains("quinjet pr suggestions"),
        "{}",
        run.stderr
    );
    Ok(())
}

#[test]
fn suggesting_a_change_posts_the_fence_github_renders() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture
        .read(&[
            "pr",
            "reviews",
            "suggest",
            "42",
            "feature.txt",
            "--line",
            "1",
            "--note",
            "Use the shorter form",
            "--body",
            "from pull request, renamed",
        ])?
        .success()?;

    ensure!(
        run.stdout.contains("Added pending review comment"),
        "{}",
        run.stdout
    );
    let calls = fixture.gh_calls()?;
    ensure!(calls.contains("Use the shorter form"), "{calls}");
    ensure!(calls.contains("```suggestion"), "{calls}");
    ensure!(calls.contains("from pull request, renamed"), "{calls}");
    ensure!(calls.contains("\"line\":1"), "{calls}");
    Ok(())
}

#[test]
fn a_suggestion_containing_a_fence_is_refused_before_anything_is_posted() -> Result<()> {
    let fixture = GitHubFixture::new()?;

    let run = fixture.read(&[
        "pr",
        "reviews",
        "suggest",
        "42",
        "feature.txt",
        "--line",
        "1",
        "--body",
        "```\nescape\n```",
    ])?;

    ensure!(run.code == 1, "expected 1, got {}", run.code);
    ensure!(
        run.stderr.contains("cannot contain a fenced code block"),
        "{}",
        run.stderr
    );
    Ok(())
}
