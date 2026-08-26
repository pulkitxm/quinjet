use std::sync::atomic::{AtomicUsize, Ordering};

use super::support::same_path;
use super::*;

static TEST_REPOSITORY_ID: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct TestRepository {
    path: PathBuf,
}

impl TestRepository {
    fn new() -> Self {
        Self::with_branch("main")
    }

    pub(crate) fn with_branch(branch: &str) -> Self {
        let id = TEST_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let name = format!("quinjet-git-test-{}-{id}", std::process::id());
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        let path = std::env::temp_dir().join(name);
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).unwrap();
        let branch_argument = format!("--initial-branch={branch}");
        run_test_git(&path, ["init", branch_argument.as_str()]);
        fs::write(path.join("README.md"), "test repository\n").unwrap();
        run_test_git(&path, ["add", "README.md"]);
        run_test_git(
            &path,
            [
                "-c",
                "user.name=Quinjet Test",
                "-c",
                "user.email=quinjet@example.com",
                "commit",
                "--message=initial",
            ],
        );
        Self { path }
    }

    pub(crate) fn repository(&self) -> Repository {
        Repository {
            root: self.path.clone(),
            github_cli: None,
        }
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

fn run_test_git<const N: usize>(path: &Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
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

mod operations;
mod stack_operation;
mod status;
