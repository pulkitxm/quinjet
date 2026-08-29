#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

impl Session {
    #[doc = " Read every member's gate and its incremental change, then reduce"]
    #[doc = " them to the answers a stack raises that no single pull request"]
    #[doc = " does: what can merge now, which one member everything is waiting"]
    #[doc = " on, and where two members touch the same file."]
    pub(crate) fn stack_review(
        &self,
        stack: &PullRequestStack,
        incremental: bool,
        refresh: bool,
    ) -> StackReview {
        let mut warnings = Vec::new();
        let mut members = Vec::new();
        for member in stack.members.iter().take(MAX_REVIEWED_STACK_MEMBERS) {
            let Some(request) = stack.member_pull_request(member.position) else {
                warnings.push(format!(
                    "stack position {} did not resolve to a pull request",
                    member.position
                ));
                continue;
            };
            let gate = match self.repository.pull_request_gate(&request, refresh) {
                Ok(gate) => gate,
                Err(error) => {
                    warnings.push(format!(
                        "unable to read the merge gate for #{}: {error:#}",
                        member.number
                    ));
                    continue;
                }
            };
            let change = if incremental {
                self.incremental_change(stack, member.position, &mut warnings)
            } else {
                None
            };
            let (paths, additions, deletions) = change.map_or_else(
                || (None, member.additions, member.deletions),
                |change| (Some(change.0), change.1, change.2),
            );
            members.push(StackReviewMemberInputs {
                position: member.position,
                number: member.number,
                title: member.title.clone(),
                url: member.url.clone(),
                selected: member.position == stack.selected_position,
                gate,
                paths,
                additions,
                deletions,
            });
        }
        let truncated = stack.truncated || stack.members.len() > MAX_REVIEWED_STACK_MEMBERS;
        if stack.members.len() > MAX_REVIEWED_STACK_MEMBERS {
            warnings.push(format!(
                "only the lowest {MAX_REVIEWED_STACK_MEMBERS} members were reviewed"
            ));
        }
        build_stack_review(StackReviewInputs {
            number: stack.number,
            base_ref: stack.base_ref.clone(),
            size: stack.size,
            selected_position: stack.selected_position,
            members,
            truncated,
            warnings,
        })
    }

    #[doc = " What one member changes on its own. The comparison runs from the"]
    #[doc = " member's own base to its own head, so it is the layer this pull"]
    #[doc = " request adds rather than everything below it repeated."]
    fn incremental_change(
        &self,
        stack: &PullRequestStack,
        position: usize,
        warnings: &mut Vec<String>,
    ) -> Option<(Vec<PathBuf>, usize, usize)> {
        match self
            .repository
            .prepare_pull_request_stack_diff(stack, position, position, |_| {})
        {
            Ok(prepared) => {
                let index = prepared.index();
                let additions = index
                    .files
                    .iter()
                    .filter_map(|file| file.counts)
                    .map(|counts| counts.additions)
                    .sum();
                let deletions = index
                    .files
                    .iter()
                    .filter_map(|file| file.counts)
                    .map(|counts| counts.deletions)
                    .sum();
                Some((
                    index.files.into_iter().map(|file| file.path).collect(),
                    additions,
                    deletions,
                ))
            }
            Err(error) => {
                warnings.push(format!(
                    "unable to compare stack position {position} against its parent: {error:#}"
                ));
                None
            }
        }
    }

    #[doc = " The stack's outstanding feedback, member by member, bottom first."]
    #[doc = " Reading it in stack order is the point: answering a thread on the"]
    #[doc = " bottom member is what lets anything above it move."]
    pub(crate) fn stack_feedback(&self, stack: &PullRequestStack, refresh: bool) -> StackFeedback {
        let mut warnings = Vec::new();
        let mut members = Vec::new();
        let mut viewer = String::new();
        for member in stack.members.iter().take(MAX_REVIEWED_STACK_MEMBERS) {
            let Some(request) = stack.member_pull_request(member.position) else {
                warnings.push(format!(
                    "stack position {} did not resolve to a pull request",
                    member.position
                ));
                continue;
            };
            if viewer.is_empty() {
                viewer = self.viewer_login(&request).unwrap_or_default();
            }
            let gate = self.repository.pull_request_gate(&request, refresh).ok();
            match self.repository.pull_request_review(&request) {
                Ok(review) => members.push((
                    member.position,
                    member.number,
                    member.title.clone(),
                    member.position == stack.selected_position,
                    build_feedback(&FeedbackInputs {
                        pull_request: &request,
                        gate: gate.as_ref(),
                        review: &review,
                        annotations: None,
                        viewer: &viewer,
                        warnings: Vec::new(),
                    }),
                )),
                Err(error) => warnings.push(format!(
                    "unable to read review threads for #{}: {error:#}",
                    member.number
                )),
            }
        }
        let truncated = stack.truncated || stack.members.len() > MAX_REVIEWED_STACK_MEMBERS;
        build_stack_feedback(StackFeedbackInputs {
            number: stack.number,
            size: stack.size,
            selected_position: stack.selected_position,
            viewer,
            members,
            truncated,
            warnings,
        })
    }
}
