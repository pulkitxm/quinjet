use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    CacheLife, GitHubRepository, PullRequest, PullRequestStack, Repository, cache_read,
    cache_write, is_commit_oid,
};

#[doc = " Gate state changes as fast as a check does, so the read is kept on a"]
#[doc = " short clock rather than on an identity: a passing run flips the verdict"]
#[doc = " without the head OID moving."]
const GATE_CACHE_TTL: Duration = Duration::from_secs(20);
const MAX_LISTED_BLOCKER_DETAILS: usize = 8;

mod blockers;
mod model;
mod query;
mod verdict;

pub(crate) use model::*;
#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use query::*;

#[cfg(test)]
mod tests;

impl Repository {
    #[doc = " Every stack member's verdict in merge order, plus the prefix that can"]
    #[doc = " merge now and the lowest blocked layer, which is the one to work on."]
    pub(crate) fn pull_request_stack_gate(
        &self,
        stack: &PullRequestStack,
        refresh: bool,
    ) -> StackGate {
        let mut members = Vec::with_capacity(stack.members.len());
        let mut warnings = Vec::new();
        for member in &stack.members {
            let Some(request) = stack.member_pull_request(member.position) else {
                warnings.push(format!(
                    "stack position {} did not resolve to a pull request",
                    member.position
                ));
                continue;
            };
            match self.pull_request_gate(&request, refresh) {
                Ok(gate) => members.push(StackGateMember {
                    position: member.position,
                    number: member.number,
                    title: member.title.clone(),
                    selected: member.position == stack.selected_position,
                    gate,
                }),
                Err(error) => warnings.push(format!(
                    "unable to read the merge gate for #{}: {error:#}",
                    member.number
                )),
            }
        }
        members.sort_by_key(|member| member.position);
        let mut mergeable_prefix = Vec::new();
        for member in &members {
            if member.gate.verdict == MergeGateVerdict::Mergeable
                || member.gate.verdict == MergeGateVerdict::Merged
            {
                mergeable_prefix.push(member.position);
            } else {
                break;
            }
        }
        let critical_position = members
            .iter()
            .find(|member| {
                member.gate.verdict != MergeGateVerdict::Mergeable
                    && member.gate.verdict != MergeGateVerdict::Merged
            })
            .map(|member| member.position);
        let verdict = members
            .iter()
            .map(|member| member.gate.verdict)
            .find(|verdict| {
                !matches!(
                    verdict,
                    MergeGateVerdict::Mergeable | MergeGateVerdict::Merged
                )
            })
            .unwrap_or(MergeGateVerdict::Mergeable);
        StackGate {
            schema_version: MergeGate::SCHEMA_VERSION,
            number: stack.number,
            base_ref: stack.base_ref.clone(),
            size: stack.size,
            selected_position: stack.selected_position,
            members,
            verdict,
            mergeable_prefix,
            critical_position,
            truncated: stack.truncated,
            warnings,
        }
    }
}
