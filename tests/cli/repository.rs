use super::*;

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
