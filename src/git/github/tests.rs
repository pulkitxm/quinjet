use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

fn test_directory(label: &str) -> TestDirectory {
    let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
    // nosemgrep: rust.lang.security.temp-dir.temp-dir
    let path = env::temp_dir().join(format!(
        "quinjet-github-{label}-{}-{id}",
        std::process::id()
    ));
    drop(fs::remove_dir_all(&path));
    fs::create_dir_all(&path).unwrap();
    TestDirectory(path)
}

fn initialized_repository() -> TestDirectory {
    let directory = test_directory("repo");
    directory.git(&["init", "--initial-branch=main"]);
    directory.git(&["config", "user.name", "Quinjet Test"]);
    directory.git(&["config", "user.email", "quinjet@example.com"]);
    fs::write(directory.0.join("README.md"), "base\n").unwrap();
    directory.git(&["add", "README.md"]);
    directory.git(&["commit", "--message=base"]);
    directory
}

pub(super) fn repository(name: &str, url: &str, remotes: &[&str]) -> GitHubRepository {
    GitHubRepository {
        name_with_owner: name.to_owned(),
        url: url.to_owned(),
        remotes: remotes.iter().map(|remote| (*remote).to_owned()).collect(),
    }
}

pub(super) fn pull_request(base: GitHubRepository, number: u64) -> PullRequest {
    PullRequest {
        number,
        title: "Ship the rocket".to_owned(),
        description: "Launch safely".to_owned(),
        author: "octocat".to_owned(),
        state: "OPEN".to_owned(),
        is_draft: false,
        created_at: "2026-08-12T09:00:00Z".to_owned(),
        updated_at: "2026-08-13T12:00:00Z".to_owned(),
        url: format!("{}/pull/{number}", base.url),
        base_ref: "main".to_owned(),
        base_oid: String::new(),
        head_ref: "feature/rocket".to_owned(),
        head_oid: String::new(),
        base_repository: base,
        head_repository: Some("octocat/widget".to_owned()),
        head_remotes: vec!["origin".to_owned()],
        is_cross_repository: true,
        additions: 1,
        deletions: 0,
        changed_files: 1,
    }
}

mod operations;
mod parsing;
mod workspace;
