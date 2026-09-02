#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " One member's reads, gathered by the caller so the reduction stays"]
#[doc = " pure. The tuple is (position, number, title, selected, queue)."]
pub(crate) type StackFeedbackMemberInput = (usize, u64, String, bool, PullRequestFeedback);

pub(crate) struct StackFeedbackInputs {
    pub number: u64,
    pub size: usize,
    pub selected_position: usize,
    pub viewer: String,
    pub members: Vec<StackFeedbackMemberInput>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

#[doc = " One queue across the whole stack, ordered bottom to top. Reading it in"]
#[doc = " stack order is the point: answering a thread on the bottom member is"]
#[doc = " what lets anything above it move, and answering one on the top member"]
#[doc = " changes nothing until the ones below are done."]
pub(crate) fn build_stack_feedback(inputs: StackFeedbackInputs) -> StackFeedback {
    let mut collected: Vec<StackFeedbackMember> = inputs
        .members
        .into_iter()
        .map(
            |(position, number, title, selected, queue)| StackFeedbackMember {
                position,
                number,
                title,
                selected,
                counts: queue.counts,
                items: queue.items,
            },
        )
        .collect();
    collected.sort_by_key(|member| member.position);
    let counts = total(&collected);
    let next_position = collected
        .iter()
        .find(|member| member.counts.blocking > 0)
        .map(|member| member.position);
    StackFeedback {
        schema_version: StackFeedback::SCHEMA_VERSION,
        number: inputs.number,
        size: inputs.size,
        selected_position: inputs.selected_position,
        viewer: inputs.viewer,
        members: collected,
        counts,
        next_position,
        truncated: inputs.truncated,
        warnings: inputs.warnings,
    }
}

fn total(members: &[StackFeedbackMember]) -> FeedbackCounts {
    let mut counts = FeedbackCounts::default();
    for member in members {
        counts.blocking += member.counts.blocking;
        counts.advisory += member.counts.advisory;
        counts.awaiting_you += member.counts.awaiting_you;
        counts.awaiting_others += member.counts.awaiting_others;
    }
    counts
}

impl FeedbackFilter {
    #[doc = " Narrow every member's rows and recompute the totals from what is"]
    #[doc = " left, so the summary always describes what is printed and the"]
    #[doc = " lowest blocked position moves up when the rows below are filtered"]
    #[doc = " away."]
    pub(crate) fn apply_stack(self, mut feedback: StackFeedback) -> StackFeedback {
        for member in &mut feedback.members {
            member.items.retain(|item| self.keeps(item));
            member.counts = counts(&member.items);
        }
        feedback.counts = total(&feedback.members);
        feedback.next_position = feedback
            .members
            .iter()
            .find(|member| member.counts.blocking > 0)
            .map(|member| member.position);
        feedback
    }
}

fn counts(items: &[FeedbackItem]) -> FeedbackCounts {
    let mut counts = FeedbackCounts::default();
    for item in items {
        if item.kind.is_blocking() {
            counts.blocking += 1;
        } else {
            counts.advisory += 1;
        }
        match item.owner {
            FeedbackOwner::You => counts.awaiting_you += 1,
            FeedbackOwner::Others => counts.awaiting_others += 1,
            FeedbackOwner::Nobody => {}
        }
    }
    counts
}
