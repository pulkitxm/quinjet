use super::arguments::assert_argument_cases;

#[test]
fn argument_relationships_accept_the_supported_combinations() {
    let cases = "
-C /tmp --json diff --staged src|branch list --json -C /tmp|pr reviews show 7 -C /tmp --json
stage src|stage --all|unstage src|unstage --all|discard --all --yes
rm generated --yes|status --watch --interval 1|diff --unstaged --expanded src
resolve file --ours|resolve file --theirs|resolve file --stage
stash push --staged -m saved|stash push --include-untracked src|stash pop stash@{0}
completions --install|completions fish --install|completion zsh|log HEAD --skip 2 --limit 4
branch create topic HEAD|pr view 7 --watch --interval 2|pr conversation 7 --watch --interval 2
pr checks 7 --watch --interval 2|pr checks 7 --exit-code|pr logs 7 lint --watch --interval 3
pr admin-merge 7 --merge|pr auto-merge 7 --rebase --delete-branch|pr merge 7 --merge
pr review 7 --approve|pr review 7 --comment --body note|pr update-branch 7 --rebase --yes
pr revert 7 --title undo --body reason --draft --yes|pr lock 7 --reason off-topic --yes
pr edit 7 title value|pr edit 7 body value|pr edit 7 base main
pr edit 7 add-assignee octo|pr edit 7 remove-assignee octo|pr edit 7 add-label bug
pr edit 7 remove-label bug|pr edit 7 add-project road|pr edit 7 remove-project road
pr edit 7 add-reviewer octo|pr edit 7 remove-reviewer octo|pr edit 7 milestone v1
pr edit 7 remove-milestone
pr reviews comment 7 file --line 2 --side left --start-line 1 --start-side left -b note
pr reviews comment 7 file --file --body-file note.txt|pr reviews reply 7 thread -b reply
pr reviews reply 7 thread --body-file -|pr reviews edit 7 comment --body-file note.txt
pr reviews delete 7 comment --yes|pr reviews submit 7 --comment -b note
pr reviews submit 7 --request-changes --body-file note.txt|pr reviews discard 7 --yes
pr reviews resolve 7 thread|pr reviews unresolve 7 thread
pr gate 7|pr gate 7 --json|pr gate 7 --watch --interval 2|pr gate 7 --no-exit-code
pr gate 7 --refresh --repo acme/project|stack gate 7|stack gate 7 --no-exit-code --json
pr diff 7 --since abc123|pr diff 7 --since-review|pr diff 7 src/lib.rs --since-review
pr reviews progress 7|pr reviews progress 7 --all --since-review|pr reviews progress 7 --since abc
pr reviews next 7|pr reviews next 7 --files|pr reviews next 7 --threads
pr reviews viewed 7 src/lib.rs|pr reviews viewed 7 --all|pr reviews viewed 7 src/lib.rs --unviewed
pr reviews viewed 7 --reset|pr reviews visit 7
pr checks annotations 7|pr checks annotations 7 --severity failure|pr checks annotations 7 --json
pr checks annotations 7 --check clippy --file src --in-diff --group check --full --exit-code
pr checks annotations 7 --group severity|pr checks annotations 7 --group file
pr checks runs 7|pr checks rerun 7 --failed|pr checks rerun 7 --all --yes
pr checks rerun 7 --check windows --yes|pr checks cancel 7|pr checks cancel 7 --yes
pr artifacts 7|pr artifacts 7 --json|pr artifacts download 7 snapshots
pr artifacts download 7 snapshots --into /tmp/out
pr deployments 7|pr deployments approve 7 staging --yes
pr deployments reject 7 staging --comment no --yes
pr feedback 7|pr feedback 7 --unresolved --mine --no-checks --full --exit-code --json
pr suggestions 7|pr suggestions apply 7 COMMENT_1 --yes
pr suggestions apply 7 --all --message fix --yes
pr reviews suggest 7 src/lib.rs --line 8 -b text
pr reviews suggest 7 src/lib.rs --line 8 --start-line 6 --note why --body-file f.txt
pr dependencies 7|pr dependencies 7 --json|pr security 7|pr security 7 --refresh
pr context 7|pr context 7 --purpose review|pr context 7 --purpose address-feedback
pr context 7 --purpose fix-ci --budget 1000 --file src/lib.rs --json
work list|work start --pr 7|work start --pr 7 --from feedback --worktree
work start --pr 7 --from failed-checks --into /tmp/work --repo acme/project --refresh
work start --pr 7 --from whole|work inspect w7-1|work diff w7-1
work verify w7-1|work verify w7-1 --exit-code -- cargo test
work publish w7-1|work publish w7-1 -m message --yes|work abort w7-1 --yes
";
    assert_eq!(assert_argument_cases(cases, true), 126);
}

#[test]
fn argument_relationships_reject_unsupported_combinations() {
    let cases = "
stage|stage src --all|unstage|unstage src --all|discard|discard src --all
remove|remove src --all|status --interval 1|status --watch --interval 0
diff --staged --unstaged|resolve file|resolve file --ours --theirs
resolve file --ours --stage|resolve file --theirs --stage|stash push --staged --include-untracked
completions|completions bash --automatic|completions --automatic|commit -m|branch create
pr view 7 --interval 2|pr view 7 --watch --interval 1
pr conversation 7 --interval 2|pr conversation 7 --watch --interval 1
pr checks 7 --interval 2|pr checks 7 --watch --interval 1|pr checks 7 --watch --exit-code
pr logs 7 lint --interval 3|pr logs 7 lint --watch --interval 2
pr admin-merge 7|pr admin-merge 7 --merge --squash
pr auto-merge 7|pr auto-merge 7 --rebase --merge|pr merge 7 --squash --rebase
pr review 7|pr review 7 --approve --comment|pr review 7 --comment --request-changes|pr review 7 --approve --request-changes
pr edit 7 unknown value|pr lock 7 --reason noisy
pr reviews comment 7 file --line 2 -b note
pr reviews comment 7 file --side right -b note
pr reviews comment 7 file --line 2 --side right --start-side left -b note
pr reviews comment 7 file --file --line 2 --side right -b note
pr reviews comment 7 file --file -b note --body-file note.txt
pr reviews comment 7 file --line 2 --side right
pr reviews reply 7 thread|pr reviews reply 7 thread -b note --body-file note.txt
pr reviews edit 7 comment|pr reviews edit 7 comment -b note --body-file note.txt
pr reviews submit 7 -b note|pr reviews submit 7 --approve --comment -b note
pr reviews submit 7 --approve|pr reviews submit 7 --approve -b note --body-file note.txt
pr reviews delete 7|pr reviews resolve 7|pr reviews unresolve 7
pr gate|pr gate 7 --interval 2|pr gate 7 --watch --interval 1|stack gate
pr diff 7 --since abc --since-review|pr reviews progress 7 --since abc --since-review
pr reviews next 7 --files --threads|pr reviews viewed 7 src/lib.rs --all
pr reviews viewed 7 --reset --all|pr reviews viewed 7 src/lib.rs --reset|pr reviews visit
pr checks annotations|pr checks annotations 7 --severity nothing|pr checks annotations 7 --group nothing
pr checks 7 annotations
pr checks rerun 7|pr checks rerun 7 --failed --all|pr checks rerun 7 --failed --check windows
pr checks runs|pr checks cancel|pr artifacts download 7|pr deployments approve 7
pr deployments approve|pr artifacts download
pr feedback|pr suggestions apply 7|pr suggestions apply 7 COMMENT_1 --all
pr reviews suggest 7 src/lib.rs -b text|pr reviews suggest 7 --line 8 -b text
pr reviews suggest 7 src/lib.rs --line 8
pr dependencies|pr security|pr context|pr context 7 --purpose nothing
pr context 7 --budget|pr context 7 --file
work|work start|work start --pr|work start --pr 7 --from nothing
work inspect|work diff|work verify|work publish|work abort
";
    assert_eq!(assert_argument_cases(cases, false), 103);
}
