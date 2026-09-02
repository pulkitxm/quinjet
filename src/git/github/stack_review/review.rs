#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " One member's reads, gathered by the caller. Keeping the fetching"]
#[doc = " outside means the reduction is pure: the same stack read twice"]
#[doc = " produces the same review, order and all."]
pub(crate) struct StackReviewMemberInputs {
    pub position: usize,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub selected: bool,
    pub gate: MergeGate,
    #[doc = " The paths this member changes against its own base, which is the"]
    #[doc = " parent's head rather than the trunk. `None` when the incremental"]
    #[doc = " comparison could not be made."]
    pub paths: Option<Vec<PathBuf>>,
    pub additions: usize,
    pub deletions: usize,
}

pub(crate) struct StackReviewInputs {
    pub number: u64,
    pub base_ref: String,
    pub size: usize,
    pub selected_position: usize,
    pub members: Vec<StackReviewMemberInputs>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

#[doc = " Turn a stack's per-member gates into the three answers a stack raises"]
#[doc = " that a single pull request does not: what can merge now, which one"]
#[doc = " member everything else is waiting on, and where two members touch the"]
#[doc = " same file."]
pub(crate) fn build_stack_review(inputs: StackReviewInputs) -> StackReview {
    let mut members: Vec<StackReviewMember> = inputs.members.into_iter().map(member).collect();
    members.sort_by_key(|member| member.position);
    let merge_order = merge_order(&members);
    let critical_position = members
        .iter()
        .find(|member| !member.is_clear())
        .map(|member| member.position);
    let critical_path = critical_position.map_or_else(Vec::new, |position| {
        members
            .iter()
            .filter(|member| member.position >= position)
            .map(|member| member.position)
            .collect()
    });
    let downstream_blocked = downstream_blocked(&mut members, critical_position);
    StackReview {
        schema_version: StackReview::SCHEMA_VERSION,
        number: inputs.number,
        base_ref: inputs.base_ref,
        size: inputs.size,
        selected_position: inputs.selected_position,
        earliest_failing_check: earliest_failing_check(&members),
        duplicated_paths: duplicated_paths(&members),
        stale_approvals: members
            .iter()
            .map(|member| member.stale_approvals.len())
            .sum(),
        unresolved_threads: members.iter().map(|member| member.unresolved_threads).sum(),
        merge_order,
        critical_path,
        critical_position,
        downstream_blocked,
        members,
        truncated: inputs.truncated,
        warnings: inputs.warnings,
    }
}

fn member(inputs: StackReviewMemberInputs) -> StackReviewMember {
    let gate = inputs.gate;
    let (paths, paths_truncated) = inputs.paths.map_or_else(
        || (Vec::new(), false),
        |mut paths| {
            paths.sort();
            paths.dedup();
            let truncated = paths.len() > MAX_MEMBER_PATHS;
            paths.truncate(MAX_MEMBER_PATHS);
            (paths, truncated)
        },
    );
    StackReviewMember {
        position: inputs.position,
        number: inputs.number,
        title: inputs.title,
        url: inputs.url,
        selected: inputs.selected,
        verdict: gate.verdict,
        block_source: StackBlockSource::None,
        blockers: gate
            .blockers
            .iter()
            .map(|blocker| format!("{}: {}", blocker.kind.label(), blocker.summary))
            .collect(),
        head_oid: gate.branch.head_oid.clone(),
        additions: inputs.additions,
        deletions: inputs.deletions,
        changed_files: paths.len(),
        stale_approvals: stale_approvals(&gate),
        unresolved_threads: gate.review.unresolved_threads,
        failing_checks: gate.checks.failing().map(GateCheck::display_name).collect(),
        paths,
        paths_truncated,
    }
}

fn stale_approvals(gate: &MergeGate) -> Vec<StaleApproval> {
    gate.review
        .reviews
        .iter()
        .filter(|review| review.stale && review.state.eq_ignore_ascii_case("APPROVED"))
        .map(|review| StaleApproval {
            reviewer: review.author.clone(),
            approved_oid: review.commit_oid.clone(),
            head_oid: gate.branch.head_oid.clone(),
        })
        .collect()
}

#[doc = " What can merge now: the clear members from the bottom, stopping at the"]
#[doc = " first that is not. A clear member above a blocked one cannot merge"]
#[doc = " either, because its base has not landed."]
fn merge_order(members: &[StackReviewMember]) -> Vec<usize> {
    let mut order = Vec::new();
    for member in members {
        if !member.is_clear() {
            break;
        }
        order.push(member.position);
    }
    order
}

#[doc = " Mark each member with where its block comes from, and collect the ones"]
#[doc = " that are only waiting for a layer below. Those are the members where"]
#[doc = " there is nothing to do, which is worth knowing before anybody starts."]
fn downstream_blocked(
    members: &mut [StackReviewMember],
    critical_position: Option<usize>,
) -> Vec<usize> {
    let mut blocked = Vec::new();
    for member in members.iter_mut() {
        let waiting_below = critical_position.is_some_and(|position| member.position > position);
        member.block_source = if !member.is_clear() {
            StackBlockSource::Own
        } else if waiting_below {
            StackBlockSource::Downstream
        } else {
            StackBlockSource::None
        };
        if member.block_source == StackBlockSource::Downstream {
            blocked.push(member.position);
        }
    }
    blocked
}

#[doc = " The failing check lowest in merge order. Everything above it waits for"]
#[doc = " that member either way, so it is the one worth looking at first."]
fn earliest_failing_check(members: &[StackReviewMember]) -> Option<StackCheckFailure> {
    members.iter().find_map(|member| {
        member
            .failing_checks
            .first()
            .map(|check| StackCheckFailure {
                position: member.position,
                number: member.number,
                check: check.clone(),
                required: true,
            })
    })
}

#[doc = " Paths more than one member changes. This is where a rebase conflict"]
#[doc = " comes from, and no single pull request's diff can show it."]
fn duplicated_paths(members: &[StackReviewMember]) -> Vec<DuplicatedPath> {
    let mut seen: Vec<DuplicatedPath> = Vec::new();
    for member in members {
        for path in &member.paths {
            match seen.iter_mut().find(|entry| &entry.path == path) {
                Some(entry) => entry.positions.push(member.position),
                None => seen.push(DuplicatedPath {
                    path: path.clone(),
                    positions: vec![member.position],
                }),
            }
        }
    }
    seen.retain(|entry| entry.positions.len() > 1);
    seen.sort_by(|left, right| left.path.cmp(&right.path));
    seen
}
