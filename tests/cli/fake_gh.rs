use std::os::unix::fs::PermissionsExt;

use super::annotations::GH_CASES as ANNOTATION_CASES;
use super::ci_operations::GH_CASES as ACTION_CASES;
use super::gate::GH_CASES;
use super::review_progress::GH_CASES as REVIEW_CASES;
use super::*;

const GH_SCRIPT_HEAD: &str = r#"#!/bin/sh
input=$(cat)
{
  printf 'argv'
  for argument in "$@"; do
    printf '\t%s' "$argument"
  done
  printf '\n'
  printf 'env\t%s\t%s\t%s\t%s\n' "$GH_PROMPT_DISABLED" "$GH_PAGER" "$GH_NO_UPDATE_NOTIFIER" "$NO_COLOR"
  printf 'stdin\t%s\n' "$input"
} >> "$FAKE_GH_CAPTURE"
case "$* $input" in
  *"stackEntry"*)
    printf '{"data":{"repository":{"pullRequest":{"stackEntry":{"position":2},"stack":{"id":"STACK_node","number":12,"size":2,"baseRefName":"main","entries":{"totalCount":2,"nodes":[{"id":"ENTRY_1","position":1,"pullRequest":{"id":"PR_41","number":41,"title":"Build stack model","author":{"login":"octocat"},"state":"OPEN","isDraft":false,"updatedAt":"2026-08-21T01:00:00Z","url":"https://github.com/acme/project/pull/41","baseRefName":"main","baseRefOid":"%s","headRefName":"stack-model","headRefOid":"%s","headRepository":{"nameWithOwner":"acme/project"},"isCrossRepository":false,"additions":1,"deletions":0,"changedFiles":1,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","reviewDecision":"APPROVED","mergeQueueEntry":null,"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"SUCCESS"}}}]} }},{"id":"ENTRY_2","position":2,"pullRequest":{"id":"PR_42","number":42,"title":"Add stack view","author":{"login":"octocat"},"state":"OPEN","isDraft":false,"updatedAt":"2026-08-21T02:00:00Z","url":"https://github.com/acme/project/pull/42","baseRefName":"stack-model","baseRefOid":"%s","headRefName":"stack-view","headRefOid":"%s","headRepository":{"nameWithOwner":"acme/project"},"isCrossRepository":false,"additions":0,"deletions":0,"changedFiles":0,"mergeStateStatus":"CLEAN","mergeable":"MERGEABLE","reviewDecision":"REVIEW_REQUIRED","mergeQueueEntry":null,"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"PENDING"}}}]}}}]}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID"
    ;;
"#;

const GH_SCRIPT_TAIL: &str = r#"  *"older"*)
    printf '{"data":{"repository":{"pullRequest":{"baseRefOid":"%s","headRefOid":"%s","commits":{"totalCount":2,"nodes":[{"commit":{"oid":"%s","abbreviatedOid":"base000","messageHeadline":"build the base","authoredDate":"2026-08-20T01:00:00Z","committedDate":"2026-08-20T01:00:00Z","url":"https://github.com/acme/project/commit/base","author":{"name":"Octo Cat","user":{"login":"octocat"}},"committer":{"name":"Octo Cat","user":{"login":"octocat"}}}}],"pageInfo":{"hasPreviousPage":false,"startCursor":"base"}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_BASE_OID"
    ;;
  *"commits(last:100,before:"*)
    printf '{"data":{"repository":{"pullRequest":{"baseRefOid":"%s","headRefOid":"%s","commits":{"totalCount":2,"nodes":[{"commit":{"oid":"%s","abbreviatedOid":"feature0","messageHeadline":"add the feature","authoredDate":"2026-08-21T02:00:00Z","committedDate":"2026-08-21T02:00:00Z","url":"https://github.com/acme/project/commit/feature","author":{"name":"Octo Cat","user":{"login":"octocat"}},"committer":{"name":"Octo Cat","user":{"login":"octocat"}}}}],"pageInfo":{"hasPreviousPage":true,"startCursor":"older"}}}}}}' "$FAKE_BASE_OID" "$FAKE_HEAD_OID" "$FAKE_HEAD_OID"
    ;;
  *"number=42"*)
    printf 'PR_node\t42\tAdd feature\tBody from fixture\toctocat\tOPEN\tfalse\t2026-08-21T02:00:00Z\thttps://github.com/acme/project/pull/42\tmain\tfeature\tacme/project\tfalse\t1\t0\t1\t%s\t%s\t2026-08-20T01:00:00Z\tfalse\ttrue\tfalse\ttrue\ttrue\ttrue\ttrue\ttrue\tSUBSCRIBED\tCLEAN\tMERGEABLE\ttrue\ttrue\t\t\t0\t\t\tAPPROVED\n' "$FAKE_BASE_OID" "$FAKE_HEAD_OID"
    ;;
  "pr checks 42 "*)
    printf 'Unit tests\tCI\tSUCCESS\tpass\tAll tests passed\thttps://github.com/acme/project/actions/runs/77/job/123\t2026-08-21T01:00:00Z\t2026-08-21T01:02:00Z\n'
    printf 'Lint\tQuality\tIN_PROGRESS\tpending\tChecking style\thttps://github.com/acme/project/actions/runs/78/job/124\t2026-08-21T01:03:00Z\t\n'
    ;;
  *"actions/jobs/123/logs"*)
    printf '2026-08-21T01:00:01Z preparing runner\n2026-08-21T01:01:01Z tests passed\n'
    ;;
  *"actions/jobs/123"*)
    printf '1\tSet up\tcompleted\tsuccess\t2026-08-21T01:00:00Z\t2026-08-21T01:00:30Z\n'
    printf '2\tRun tests\tcompleted\tsuccess\t2026-08-21T01:00:31Z\t2026-08-21T01:02:00Z\n'
    ;;
  "pr close 42 "*|"pr comment 42 "*|"pr merge 42 "*)
    ;;
  *)
    printf 'unexpected fake gh invocation: %s\n' "$*" >&2
    exit 91
    ;;
esac
"#;

pub(super) const OPEN_SCRIPT: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_OPEN_CAPTURE"
"#;

#[doc = " The fake GitHub CLI is assembled from a shared head and tail plus one"]
#[doc = " block of cases per feature, so no single test file has to carry every"]
#[doc = " fixture the suite needs."]
pub(super) fn gh_script() -> String {
    let mut script = String::from(GH_SCRIPT_HEAD);
    script.push_str(GH_CASES);
    script.push_str(ANNOTATION_CASES);
    script.push_str(ACTION_CASES);
    script.push_str(REVIEW_CASES);
    script.push_str(GH_SCRIPT_TAIL);
    script
}

pub(super) fn executable(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}
