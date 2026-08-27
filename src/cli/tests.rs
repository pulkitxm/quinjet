use std::collections::HashSet;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use indicatif::InMemoryTerm;

use super::*;
use crate::git::github::PullRequestCheck;

static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

struct TestRepository {
    path: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("quinjet-cli-test-{}-{id}", std::process::id());
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        let path = std::env::temp_dir().join(name);
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).unwrap();
        let repository = Self { path };
        repository.git(&["init", "--initial-branch=main"]);
        repository.git(&["config", "user.name", "Quinjet Test"]);
        repository.git(&["config", "user.email", "quinjet@example.com"]);
        fs::write(repository.path.join("README.md"), "one\n").unwrap();
        repository.git(&["add", "README.md"]);
        repository.git(&["commit", "--message=base"]);
        repository
    }

    fn git(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.path)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn session(&self) -> Session {
        Session::new(Repository::discover(&self.path).unwrap())
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

fn check(name: &str, status: PullRequestCheckStatus) -> PullRequestCheck {
    PullRequestCheck {
        name: name.to_owned(),
        workflow: "CI".to_owned(),
        state: String::new(),
        status,
        description: String::new(),
        link: String::new(),
        started_at: String::new(),
        completed_at: String::new(),
    }
}

mod arguments;
mod output;
mod routes;
mod themes;
