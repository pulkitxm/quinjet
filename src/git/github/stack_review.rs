#[cfg_attr(not(test), expect(clippy::wildcard_imports, reason = "shared"))]
use super::*;

#[doc = " A stack is read whole, so the per-member reads are bounded rather than"]
#[doc = " one query per file across an arbitrarily deep stack."]
pub(crate) const MAX_REVIEWED_STACK_MEMBERS: usize = 16;
#[doc = " Enough paths per member to spot an overlap without turning the review"]
#[doc = " into a file listing."]
const MAX_MEMBER_PATHS: usize = 200;

mod feedback;
mod model;
mod review;

pub(crate) use feedback::{StackFeedbackInputs, build_stack_feedback};
pub(crate) use model::*;
pub(crate) use review::{StackReviewInputs, StackReviewMemberInputs, build_stack_review};

#[cfg(test)]
mod tests;
