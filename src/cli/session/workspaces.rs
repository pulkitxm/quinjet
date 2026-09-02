#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[derive(Clone, Copy)]
pub(super) enum LocalDiffWorkspaceKind {
    Changes,
    History,
}

impl LocalDiffWorkspaceKind {
    pub(super) const fn from_request(request: &LocalDiffRequest) -> Self {
        match request {
            LocalDiffRequest::Commit { .. } => Self::History,
            LocalDiffRequest::Changes { .. }
            | LocalDiffRequest::Branch { .. }
            | LocalDiffRequest::Stash { .. } => Self::Changes,
        }
    }
}

pub(super) struct LocalDiffWorkspaces<T> {
    changes: Option<(u64, T)>,
    history: Option<(u64, T)>,
}

impl<T> LocalDiffWorkspaces<T> {
    pub(super) const fn new() -> Self {
        Self {
            changes: None,
            history: None,
        }
    }

    pub(super) fn store(&mut self, kind: LocalDiffWorkspaceKind, workspace: u64, prepared: T) {
        let slot = match kind {
            LocalDiffWorkspaceKind::Changes => &mut self.changes,
            LocalDiffWorkspaceKind::History => &mut self.history,
        };
        *slot = Some((workspace, prepared));
    }

    pub(super) fn get(&self, workspace: u64) -> Option<&T> {
        [&self.changes, &self.history]
            .into_iter()
            .flatten()
            .find(|(candidate, _)| *candidate == workspace)
            .map(|(_, prepared)| prepared)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::cell::Cell;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    use super::{Command, Outcome, Session};
    use super::{LocalDiffWorkspaceKind, LocalDiffWorkspaces};
    #[cfg(unix)]
    use crate::git::github::{GitHubRepository, PullRequest};
    #[cfg(unix)]
    use crate::git::tests::TestRepository;

    #[test]
    fn paused_changes_workspace_survives_history_browsing() {
        let mut workspaces = LocalDiffWorkspaces::new();
        workspaces.store(LocalDiffWorkspaceKind::Changes, 11, 110);

        for generation in 12..100 {
            workspaces.store(LocalDiffWorkspaceKind::History, generation, generation * 10);
        }

        assert_eq!(workspaces.get(11), Some(&110));
        assert_eq!(workspaces.get(98), None);
        assert_eq!(workspaces.get(99), Some(&990));
    }

    #[test]
    fn each_view_replaces_only_its_own_workspace() {
        let mut workspaces = LocalDiffWorkspaces::new();
        workspaces.store(LocalDiffWorkspaceKind::Changes, 21, 210);
        workspaces.store(LocalDiffWorkspaceKind::History, 22, 220);
        workspaces.store(LocalDiffWorkspaceKind::Changes, 23, 230);

        assert_eq!(workspaces.get(21), None);
        assert_eq!(workspaces.get(22), Some(&220));
        assert_eq!(workspaces.get(23), Some(&230));
    }

    #[cfg(unix)]
    #[test]
    fn stack_warming_continues_after_a_member_read_fails() {
        let fixture = TestRepository::with_branch("main");
        let repository = fixture.repository();
        let executable = repository.root().join("gh");
        let calls = repository.root().join("calls");
        fs::write(
            &executable,
            format!(
                "#!/bin/sh\nprintf 'call\\n' >> '{}'\nprintf 'failed\\n' >&2\nexit 1\n",
                calls.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        let mut session = Session::new(fixture.repository_with_github_cli(executable));
        let pull_requests = [41, 42]
            .map(|number| PullRequest {
                number,
                head_oid: format!("head-{number}"),
                base_repository: GitHubRepository {
                    name_with_owner: "acme/widget".to_owned(),
                    url: format!("https://example.test/warm/{number}"),
                    remotes: Vec::new(),
                },
                ..PullRequest::default()
            })
            .into_iter()
            .collect();
        let wanted_calls = Cell::new(0);

        let outcome = session
            .execute_with(
                Command::WarmPullRequestStackMembers { pull_requests },
                &mut |_| {},
                &|| {
                    let current = wanted_calls.get();
                    wanted_calls.set(current + 1);
                    current < 2
                },
            )
            .unwrap();

        assert!(matches!(outcome, Outcome::Warmed));
        assert_eq!(fs::read_to_string(calls).unwrap(), "call\ncall\n");
    }
}
