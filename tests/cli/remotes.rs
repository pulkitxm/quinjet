use super::*;

struct RemoteFixture {
    local: Scratch,
    remote: Scratch,
    peer: Scratch,
}

impl RemoteFixture {
    fn tracked() -> Result<Self> {
        let local = Scratch::repository()?;
        let remote = bare_repository()?;
        let remote_path = remote.path.display().to_string();
        local.git(&["remote", "add", "origin", &remote_path])?;
        local.git(&["push", "--set-upstream", "origin", "main"])?;
        let peer = Scratch::directory()?;
        clone_repository(&remote.path, &peer.path)?;
        peer.git(&["config", "user.name", "Quinjet Peer"])?;
        peer.git(&["config", "user.email", "peer@example.com"])?;
        peer.git(&["config", "commit.gpgsign", "false"])?;
        Ok(Self {
            local,
            remote,
            peer,
        })
    }

    fn peer_commit(&self, path: &str, contents: &str, message: &str) -> Result<String> {
        self.peer.write(path, contents)?;
        self.peer.git(&["add", path])?;
        self.peer.git(&["commit", "--message", message])?;
        self.peer.git(&["push", "origin", "main"])?;
        self.peer.git(&["rev-parse", "HEAD"])
    }

    fn remote_main(&self) -> Result<String> {
        self.remote.git(&["rev-parse", "refs/heads/main"])
    }
}

fn bare_repository() -> Result<Scratch> {
    let remote = Scratch::directory()?;
    remote.git(&["init", "--bare", "--initial-branch=main"])?;
    Ok(remote)
}

fn clone_repository(source: &Path, target: &Path) -> Result<()> {
    let mut command = ProcessCommand::new("git");
    command.arg("clone").arg("--quiet").arg(source).arg(target);
    isolate_git(&mut command);
    drop(Run::from(command.output().context("failed to clone the peer")?)?.success()?);
    Ok(())
}

fn assert_failure(run: &Run) -> Result<()> {
    ensure!(run.code == 1, "expected exit 1, got {}", run.code);
    ensure!(
        run.stdout.is_empty(),
        "failure wrote stdout: {}",
        run.stdout
    );
    ensure!(
        run.stderr.contains("error:"),
        "failure omitted its error: {}",
        run.stderr
    );
    Ok(())
}

#[test]
fn first_push_sets_origin_as_the_upstream() -> Result<()> {
    let local = Scratch::repository()?;
    let remote = bare_repository()?;
    let remote_path = remote.path.display().to_string();
    local.git(&["remote", "add", "origin", &remote_path])?;
    let head = local.git(&["rev-parse", "HEAD"])?;

    let pushed = local.quinjet(&["push", "--json"])?.success()?;

    ensure!(pushed.stderr.is_empty(), "{}", pushed.stderr);
    ensure!(pushed.json()?["message"] == "Push complete");
    ensure!(local.git(&["rev-parse", "--abbrev-ref", "@{upstream}"])? == "origin/main");
    ensure!(remote.git(&["rev-parse", "refs/heads/main"])? == head);
    ensure!(local.git(&["rev-parse", "HEAD"])? == head);
    Ok(())
}

#[test]
fn subsequent_push_updates_the_existing_upstream() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    fixture.local.write("local.txt", "local\n")?;
    fixture.local.git(&["add", "local.txt"])?;
    fixture.local.git(&["commit", "--message=local"])?;
    let head = fixture.local.git(&["rev-parse", "HEAD"])?;

    let pushed = fixture.local.quinjet(&["push"])?.success()?;

    ensure!(pushed.stdout == "Push complete\n", "{}", pushed.stdout);
    ensure!(pushed.stderr.is_empty(), "{}", pushed.stderr);
    ensure!(fixture.remote_main()? == head);
    ensure!(
        fixture
            .local
            .git(&["rev-parse", "--abbrev-ref", "@{upstream}"])?
            == "origin/main"
    );
    Ok(())
}

#[test]
fn push_without_an_upstream_or_origin_fails_without_moving_head() -> Result<()> {
    let local = Scratch::repository()?;
    let head = local.git(&["rev-parse", "HEAD"])?;
    let status = local.git(&["status", "--porcelain"])?;

    let pushed = local.quinjet(&["push"])?;

    assert_failure(&pushed)?;
    ensure!(pushed.stderr.contains("no upstream and no `origin`"));
    ensure!(local.git(&["rev-parse", "HEAD"])? == head);
    ensure!(local.git(&["status", "--porcelain"])? == status);
    ensure!(local.git(&["remote"])?.is_empty());
    Ok(())
}

#[test]
fn push_rejects_non_fast_forward_without_changing_either_tip() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    let remote = fixture.peer_commit("peer.txt", "peer\n", "peer")?;
    fixture.local.write("local.txt", "local\n")?;
    fixture.local.git(&["add", "local.txt"])?;
    fixture.local.git(&["commit", "--message=local"])?;
    let local = fixture.local.git(&["rev-parse", "HEAD"])?;

    let pushed = fixture.local.quinjet(&["push", "--json"])?;

    assert_failure(&pushed)?;
    ensure!(
        pushed.stderr.to_ascii_lowercase().contains("rejected"),
        "{}",
        pushed.stderr
    );
    ensure!(fixture.remote_main()? == remote);
    ensure!(fixture.local.git(&["rev-parse", "HEAD"])? == local);
    ensure!(fixture.local.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn fetch_discovers_a_new_remote_branch() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    fixture.peer.git(&["switch", "--create", "topic"])?;
    fixture.peer.write("topic.txt", "topic\n")?;
    fixture.peer.git(&["add", "topic.txt"])?;
    fixture.peer.git(&["commit", "--message=topic"])?;
    fixture
        .peer
        .git(&["push", "--set-upstream", "origin", "topic"])?;
    let topic = fixture.peer.git(&["rev-parse", "HEAD"])?;
    ensure!(
        fixture
            .local
            .git_run(&[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/remotes/origin/topic"
            ])?
            .code
            != 0
    );

    let fetched = fixture.local.quinjet(&["fetch", "--json"])?.success()?;

    ensure!(fetched.stderr.is_empty(), "{}", fetched.stderr);
    ensure!(fetched.json()?["message"] == "Fetch complete");
    ensure!(
        fixture
            .local
            .git(&["rev-parse", "refs/remotes/origin/topic"])?
            == topic
    );
    ensure!(fixture.local.git(&["branch", "--show-current"])? == "main");
    Ok(())
}

#[test]
fn fetch_prunes_a_deleted_remote_branch() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    fixture.peer.git(&["switch", "--create", "topic"])?;
    fixture
        .peer
        .git(&["push", "--set-upstream", "origin", "topic"])?;
    fixture.local.git(&["fetch", "origin"])?;
    fixture.peer.git(&["push", "origin", "--delete", "topic"])?;
    ensure!(
        fixture
            .local
            .git_run(&[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/remotes/origin/topic"
            ])?
            .code
            == 0
    );

    let fetched = fixture.local.quinjet(&["fetch"])?.success()?;

    ensure!(fetched.stdout == "Fetch complete\n", "{}", fetched.stdout);
    ensure!(fetched.stderr.is_empty(), "{}", fetched.stderr);
    ensure!(
        fixture
            .local
            .git_run(&[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/remotes/origin/topic"
            ])?
            .code
            != 0
    );
    Ok(())
}

#[test]
fn pull_fast_forwards_to_a_peer_commit() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    let before = fixture.local.git(&["rev-parse", "HEAD"])?;
    let peer = fixture.peer_commit("peer.txt", "from peer\n", "peer")?;

    let pulled = fixture.local.quinjet(&["pull", "--json"])?.success()?;

    ensure!(pulled.stderr.is_empty(), "{}", pulled.stderr);
    ensure!(pulled.json()?["message"] == "Pull complete");
    ensure!(fixture.local.git(&["rev-parse", "HEAD"])? == peer);
    ensure!(fixture.local.git(&["rev-parse", "HEAD"])? != before);
    ensure!(fs::read_to_string(fixture.local.path.join("peer.txt"))? == "from peer\n");
    ensure!(fixture.local.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn pull_without_an_upstream_fails_without_changing_the_repository() -> Result<()> {
    let local = Scratch::repository()?;
    let head = local.git(&["rev-parse", "HEAD"])?;
    let tree = local.git(&["write-tree"])?;

    let pulled = local.quinjet(&["pull", "--json"])?;

    assert_failure(&pulled)?;
    ensure!(local.git(&["rev-parse", "HEAD"])? == head);
    ensure!(local.git(&["write-tree"])? == tree);
    ensure!(local.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn sync_merges_nonconflicting_divergence_and_pushes_the_result() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    let peer = fixture.peer_commit("peer.txt", "peer\n", "peer")?;
    fixture.local.write("local.txt", "local\n")?;
    fixture.local.git(&["add", "local.txt"])?;
    fixture.local.git(&["commit", "--message=local"])?;
    let local = fixture.local.git(&["rev-parse", "HEAD"])?;
    fixture.local.git(&["config", "pull.rebase", "false"])?;

    let synced = fixture.local.quinjet(&["sync", "--json"])?.success()?;

    ensure!(synced.stderr.is_empty(), "{}", synced.stderr);
    ensure!(synced.json()?["message"] == "Synchronization complete");
    let merged = fixture.local.git(&["rev-parse", "HEAD"])?;
    ensure!(fixture.remote_main()? == merged);
    ensure!(
        fixture
            .local
            .git(&["merge-base", "--is-ancestor", &peer, &merged])?
            .is_empty()
    );
    ensure!(
        fixture
            .local
            .git(&["merge-base", "--is-ancestor", &local, &merged])?
            .is_empty()
    );
    ensure!(
        fixture
            .local
            .git(&["rev-list", "--parents", "-n", "1", "HEAD"])?
            .split_whitespace()
            .count()
            == 3
    );
    ensure!(fixture.local.path.join("peer.txt").is_file());
    ensure!(fixture.local.path.join("local.txt").is_file());
    ensure!(fixture.local.git(&["status", "--porcelain"])?.is_empty());
    Ok(())
}

#[test]
fn sync_stops_after_a_pull_conflict_without_pushing() -> Result<()> {
    let fixture = RemoteFixture::tracked()?;
    let remote = fixture.peer_commit("README.md", "peer\n", "peer conflict")?;
    fixture.local.write("README.md", "local\n")?;
    fixture.local.git(&["add", "README.md"])?;
    fixture.local.git(&["commit", "--message=local conflict"])?;
    let local = fixture.local.git(&["rev-parse", "HEAD"])?;
    fixture.local.git(&["config", "pull.rebase", "false"])?;

    let synced = fixture.local.quinjet(&["sync"])?;

    assert_failure(&synced)?;
    ensure!(fixture.remote_main()? == remote);
    ensure!(fixture.local.git(&["rev-parse", "HEAD"])? == local);
    ensure!(
        fixture
            .local
            .git(&["diff", "--name-only", "--diff-filter=U"])?
            == "README.md"
    );
    Ok(())
}
