use super::*;

fn conflicted_repository() -> Result<Scratch> {
    let scratch = Scratch::repository()?;
    scratch.git(&["switch", "--create", "incoming"])?;
    scratch.write("README.md", "incoming\n")?;
    scratch.git(&["add", "README.md"])?;
    scratch.git(&["commit", "--message=incoming"])?;
    scratch.git(&["switch", "main"])?;
    scratch.write("README.md", "current\n")?;
    scratch.git(&["add", "README.md"])?;
    scratch.git(&["commit", "--message=current"])?;
    let merge = scratch.git_run(&["merge", "incoming"])?;
    ensure!(merge.code != 0, "the fixture merge unexpectedly succeeded");
    Ok(scratch)
}

#[test]
fn stage_commit_log_show_round_trip() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("feature.txt", "feature\n")?;
    drop(scratch.quinjet(&["stage", "feature.txt"])?.success()?);
    drop(
        scratch
            .quinjet(&["commit", "--message", "add the feature"])?
            .success()?,
    );
    let log = scratch.quinjet(&["log", "-n", "1"])?.success()?;
    ensure!(
        log.stdout.contains("add the feature"),
        "log misses the commit: {}",
        log.stdout
    );
    let show = scratch.quinjet(&["show"])?.success()?;
    ensure!(
        show.stdout.contains("feature.txt"),
        "show misses the file: {}",
        show.stdout
    );
    Ok(())
}

#[test]
fn diff_shows_changes_and_stage_all_clears_them() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\ntwo\n")?;
    let diff = scratch.quinjet(&["diff"])?.success()?;
    ensure!(
        diff.stdout.contains("two"),
        "diff misses the new line: {}",
        diff.stdout
    );
    drop(scratch.quinjet(&["stage", "--all"])?.success()?);
    let staged = scratch.quinjet(&["diff", "--staged"])?.success()?;
    ensure!(
        staged.stdout.contains("two"),
        "staged diff misses the new line: {}",
        staged.stdout
    );
    Ok(())
}

#[test]
fn named_and_all_unstage_preserve_unselected_paths() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("one.txt", "one\n")?;
    scratch.write("two.txt", "two\n")?;
    drop(scratch.quinjet(&["stage", "--all"])?.success()?);
    let named = scratch.quinjet(&["unstage", "one.txt"])?.success()?;
    ensure!(named.stdout == "1 change unstaged\n", "{}", named.stdout);
    ensure!(named.stderr.is_empty(), "{}", named.stderr);
    ensure!(
        scratch.git(&["diff", "--cached", "--name-only"])? == "two.txt",
        "named unstage touched the wrong paths"
    );
    let all = scratch
        .quinjet(&["unstage", "--all", "--json"])?
        .success()?;
    ensure!(all.stderr.is_empty(), "{}", all.stderr);
    ensure!(all.json()?["message"] == "All changes unstaged");
    ensure!(
        scratch
            .git(&["diff", "--cached", "--name-only"])?
            .is_empty(),
        "unstage --all left index entries"
    );
    let status = scratch.git(&["status", "--porcelain"])?;
    ensure!(status.contains("?? one.txt"), "unexpected status: {status}");
    ensure!(status.contains("?? two.txt"), "unexpected status: {status}");
    Ok(())
}

#[test]
fn commit_amend_replaces_the_tip_without_growing_history() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("feature.txt", "first\n")?;
    scratch.git(&["add", "feature.txt"])?;
    scratch.git(&["commit", "--message=original feature"])?;
    let original = scratch.git(&["rev-parse", "HEAD"])?;
    let count = scratch.git(&["rev-list", "--count", "HEAD"])?;
    scratch.write("feature.txt", "revised\n")?;
    scratch.git(&["add", "feature.txt"])?;

    let args = ["commit", "-m", "revised feature", "--amend", "--json"];
    let amended = scratch.quinjet(&args)?.success()?;
    ensure!(amended.stderr.is_empty(), "{}", amended.stderr);
    ensure!(amended.json()?["message"] == "Commit amended");
    ensure!(scratch.git(&["rev-parse", "HEAD"])? != original);
    ensure!(scratch.git(&["rev-list", "--count", "HEAD"])? == count);
    ensure!(scratch.git(&["log", "-1", "--format=%s"])? == "revised feature");
    ensure!(scratch.git(&["show", "HEAD:feature.txt"])? == "revised");
    ensure!(scratch.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn branch_creation_from_a_revision_and_compare_do_not_move_head() -> Result<()> {
    let scratch = Scratch::repository()?;
    let base = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.write("main.txt", "main\n")?;
    scratch.git(&["add", "main.txt"])?;
    scratch.git(&["commit", "--message=main work"])?;
    let main = scratch.git(&["rev-parse", "HEAD"])?;

    let created = scratch
        .quinjet(&["branch", "create", "topic", &base])?
        .success()?;
    ensure!(created.stdout.contains("Created and switched to topic"));
    ensure!(created.stderr.is_empty());
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == base);
    scratch.write("topic.txt", "topic\n")?;
    scratch.git(&["add", "topic.txt"])?;
    scratch.git(&["commit", "--message=topic work"])?;
    scratch.git(&["switch", "main"])?;
    let refs = scratch.git(&["show-ref"])?;

    let compared = scratch
        .quinjet(&["branch", "compare", "topic", "--json"])?
        .success()?;
    ensure!(compared.stderr.is_empty(), "{}", compared.stderr);
    let document = compared.json()?;
    let rendered = document.to_string();
    ensure!(rendered.contains("main.txt"), "comparison omitted main.txt");
    ensure!(
        rendered.contains("topic.txt"),
        "comparison omitted topic.txt"
    );
    ensure!(scratch.git(&["branch", "--show-current"])? == "main");
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == main);
    ensure!(scratch.git(&["show-ref"])? == refs);
    ensure!(scratch.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn discard_previews_without_yes_and_acts_with_it() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\nchanged\n")?;
    let preview = scratch.quinjet(&["discard", "README.md"])?.success()?;
    ensure!(
        preview.stderr.is_empty(),
        "preview wrote: {}",
        preview.stderr
    );
    ensure!(
        preview.stdout.contains("Pass --yes"),
        "preview did not explain confirmation: {}",
        preview.stdout
    );
    let preserved = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(
        preserved.contains("changed"),
        "a preview discarded the change"
    );
    drop(
        scratch
            .quinjet(&["discard", "README.md", "--yes"])?
            .success()?,
    );
    let restored = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(restored == "one\n", "discard left: {restored}");
    Ok(())
}

#[test]
fn remove_previews_without_yes_and_deletes_with_it() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("untracked.txt", "scratch\n")?;
    let preview = scratch
        .quinjet(&["remove", "README.md", "untracked.txt"])?
        .success()?;
    ensure!(
        preview.stdout.contains("Pass --yes"),
        "preview did not explain confirmation: {}",
        preview.stdout
    );
    ensure!(
        scratch.path.join("README.md").exists() && scratch.path.join("untracked.txt").exists(),
        "a preview removed files"
    );
    let removed = scratch
        .quinjet(&["remove", "README.md", "untracked.txt", "--yes"])?
        .success()?;
    ensure!(
        removed.stdout.contains("2 files removed"),
        "remove reported: {}",
        removed.stdout
    );
    ensure!(
        !scratch.path.join("README.md").exists() && !scratch.path.join("untracked.txt").exists(),
        "remove left the files in the working tree"
    );
    ensure!(
        scratch
            .git(&["status", "--porcelain"])?
            .contains("D  README.md"),
        "remove did not stage the deletion"
    );
    Ok(())
}

#[test]
fn revision_mutations_preview_until_confirmed() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.git(&["switch", "--create", "feature"])?;
    scratch.write("feature.txt", "feature\n")?;
    scratch.git(&["add", "feature.txt"])?;
    scratch.git(&["commit", "--message=feature"])?;
    let feature = scratch.git(&["rev-parse", "HEAD"])?;
    scratch.git(&["switch", "main"])?;

    let before = scratch.git(&["rev-parse", "HEAD"])?;
    let preview = scratch.quinjet(&["cherry-pick", &feature])?.success()?;
    ensure!(preview.stdout.contains("Pass --yes"));
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == before);

    drop(
        scratch
            .quinjet(&["cherry-pick", &feature, "--yes"])?
            .success()?,
    );
    let applied = scratch.git(&["rev-parse", "HEAD"])?;
    ensure!(applied != before);

    let preview = scratch.quinjet(&["revert", &applied])?.success()?;
    ensure!(preview.stdout.contains("Pass --yes"));
    ensure!(scratch.git(&["rev-parse", "HEAD"])? == applied);

    drop(scratch.quinjet(&["revert", &applied, "--yes"])?.success()?);
    ensure!(scratch.git(&["rev-parse", "HEAD"])? != applied);
    Ok(())
}

#[test]
fn branch_lifecycle_round_trips() -> Result<()> {
    let scratch = Scratch::repository()?;
    drop(
        scratch
            .quinjet(&["branch", "create", "feature"])?
            .success()?,
    );
    drop(
        scratch
            .quinjet(&["branch", "rename", "feature", "renamed"])?
            .success()?,
    );
    drop(scratch.quinjet(&["branch", "switch", "main"])?.success()?);
    let listed = scratch.quinjet(&["branch", "list"])?.success()?;
    ensure!(
        listed.stdout.contains("renamed"),
        "branch list misses the branch: {}",
        listed.stdout
    );
    drop(
        scratch
            .quinjet(&["branch", "delete", "renamed", "--yes"])?
            .success()?,
    );
    let after = scratch.quinjet(&["branch", "list"])?.success()?;
    ensure!(
        !after.stdout.contains("renamed"),
        "branch delete left the branch: {}",
        after.stdout
    );
    Ok(())
}

#[test]
fn stash_push_and_pop_round_trips() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\nstashed\n")?;
    drop(
        scratch
            .quinjet(&["stash", "push", "--message", "held"])?
            .success()?,
    );
    let clean = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(clean == "one\n", "stash push left: {clean}");
    let listed = scratch.quinjet(&["stash", "list"])?.success()?;
    ensure!(
        listed.stdout.contains("held"),
        "stash list misses the entry: {}",
        listed.stdout
    );
    drop(scratch.quinjet(&["stash", "pop"])?.success()?);
    let restored = fs::read_to_string(scratch.path.join("README.md"))?;
    ensure!(
        restored.contains("stashed"),
        "stash pop did not restore: {restored}"
    );
    Ok(())
}

#[test]
fn stash_show_apply_and_drop_preserve_index_and_untracked_files() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("README.md", "one\nstaged\n")?;
    scratch.git(&["add", "README.md"])?;
    scratch.write("untracked.txt", "untracked\n")?;
    let pushed = scratch
        .quinjet(&[
            "stash",
            "push",
            "--message",
            "complete state",
            "--include-untracked",
            "--json",
        ])?
        .success()?;
    ensure!(pushed.json()?["message"] == "Changes stashed");
    ensure!(scratch.git(&["status", "--porcelain"])?.is_empty());

    let shown = scratch
        .quinjet(&["stash", "show", "stash@{0}", "--json"])?
        .success()?;
    ensure!(shown.stderr.is_empty(), "{}", shown.stderr);
    let patch = shown.json()?.to_string();
    ensure!(patch.contains("README.md"), "stash patch omitted README.md");
    ensure!(
        patch.contains("untracked.txt"),
        "stash patch omitted untracked.txt"
    );

    let applied = scratch
        .quinjet(&["stash", "apply", "stash@{0}"])?
        .success()?;
    ensure!(applied.stdout.contains("Applied stash@{0}"));
    ensure!(applied.stderr.is_empty(), "{}", applied.stderr);
    ensure!(fs::read_to_string(scratch.path.join("README.md"))? == "one\nstaged\n");
    ensure!(fs::read_to_string(scratch.path.join("untracked.txt"))? == "untracked\n");
    ensure!(scratch.git(&["diff", "--cached", "--name-only"])? == "README.md");
    ensure!(scratch.git(&["stash", "list"])?.lines().count() == 1);

    let preview = scratch
        .quinjet(&["stash", "drop", "stash@{0}", "--json"])?
        .success()?;
    ensure!(
        preview.json()?["message"]
            .as_str()
            .is_some_and(|message| message.contains("Pass --yes"))
    );
    ensure!(scratch.git(&["stash", "list"])?.lines().count() == 1);
    let dropped = scratch
        .quinjet(&["stash", "drop", "stash@{0}", "--yes", "--json"])?
        .success()?;
    ensure!(dropped.json()?["message"] == "Dropped stash@{0}");
    ensure!(scratch.git(&["stash", "list"])?.is_empty());
    Ok(())
}

#[test]
fn staged_stashes_leave_unstaged_work_and_clear_only_after_confirmation() -> Result<()> {
    let scratch = Scratch::repository()?;
    scratch.write("other.txt", "base\n")?;
    scratch.git(&["add", "other.txt"])?;
    scratch.git(&["commit", "--message=track other"])?;
    scratch.write("README.md", "staged only\n")?;
    scratch.git(&["add", "README.md"])?;
    scratch.write("other.txt", "left live\n")?;

    let first = ["stash", "push", "--staged", "-m", "index only"];
    drop(scratch.quinjet(&first)?.success()?);
    ensure!(
        fs::read_to_string(scratch.path.join("README.md"))? == "one\n"
            && fs::read_to_string(scratch.path.join("other.txt"))? == "left live\n"
    );
    scratch.git(&["restore", "other.txt"])?;
    scratch.write("README.md", "second staged\n")?;
    scratch.git(&["add", "README.md"])?;
    let second = ["stash", "push", "--staged", "-m", "second index"];
    drop(scratch.quinjet(&second)?.success()?);
    ensure!(fs::read_to_string(scratch.path.join("README.md"))? == "one\n");
    let shown = scratch
        .quinjet(&["stash", "show", "stash@{0}"])?
        .success()?;
    ensure!(shown.stdout.contains("README.md"), "{}", shown.stdout);

    let before = scratch.git(&["stash", "list"])?;
    ensure!(before.lines().count() == 2, "unexpected stashes: {before}");
    let preview = scratch.quinjet(&["stash", "clear", "--json"])?.success()?;
    ensure!(
        preview.json()?["message"]
            .as_str()
            .is_some_and(|message| message.contains("Would drop 2 stashes"))
    );
    ensure!(scratch.git(&["stash", "list"])? == before);
    let cleared = scratch
        .quinjet(&["stash", "clear", "--yes", "--json"])?
        .success()?;
    ensure!(cleared.json()?["message"] == "Dropped all stashes");
    ensure!(scratch.git(&["stash", "list"])?.is_empty());
    Ok(())
}

#[test]
fn conflict_resolution_choices_stage_the_selected_content() -> Result<()> {
    for (choice, expected) in [
        ("--ours", "current\n"),
        ("--theirs", "incoming\n"),
        ("--stage", "combined\n"),
    ] {
        let scratch = conflicted_repository()?;
        if choice == "--stage" {
            scratch.write("README.md", expected)?;
        }
        let resolved = scratch
            .quinjet(&["resolve", "README.md", choice, "--json"])?
            .success()?;
        ensure!(resolved.stderr.is_empty(), "{}", resolved.stderr);
        ensure!(resolved.json()?["message"].is_string());
        ensure!(fs::read_to_string(scratch.path.join("README.md"))? == expected);
        ensure!(
            scratch
                .git(&["diff", "--name-only", "--diff-filter=U"])?
                .is_empty(),
            "{choice} left an unresolved path"
        );
        let index = scratch.git(&["ls-files", "--stage", "README.md"])?;
        ensure!(
            index.ends_with(" 0\tREADME.md"),
            "{choice} left a non-stage-zero index entry: {index}"
        );
    }
    Ok(())
}

#[test]
fn worktree_list_includes_linked_trees() -> Result<()> {
    let scratch = Scratch::repository()?;
    let id = SCRATCH_ID.fetch_add(1, Ordering::Relaxed);
    let linked = std::env::temp_dir().join(format!(
        "quinjet-blackbox-linked-{}-{id}",
        std::process::id()
    ));
    drop(fs::remove_dir_all(&linked));
    let linked_display = linked.display().to_string();
    drop(scratch.git(&["worktree", "add", "-b", "topic", &linked_display])?);
    let listed = scratch.quinjet(&["worktree", "list"])?.success()?;
    ensure!(
        listed.stdout.contains("topic"),
        "worktree list misses the linked branch: {}",
        listed.stdout
    );
    let linked_name = linked
        .file_name()
        .and_then(|name| name.to_str())
        .context("the linked worktree has no file name")?;
    ensure!(
        listed.stdout.contains(linked_name),
        "worktree list misses the linked path: {}",
        listed.stdout
    );
    let trees = scratch
        .quinjet(&["worktree", "list", "--json"])?
        .success()?
        .json()?;
    let trees = trees.as_array().context("worktree JSON was not an array")?;
    ensure!(trees.len() == 2, "expected two worktrees, got {trees:?}");
    drop(fs::remove_dir_all(&linked));
    drop(scratch.git(&["worktree", "prune"])?);
    Ok(())
}
