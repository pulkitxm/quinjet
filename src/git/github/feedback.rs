use std::path::PathBuf;

use serde::Serialize;

use super::{
    AnnotationSeverity, CheckAnnotation, MergeGate, MergeGateBlockerKind, PullRequest,
    PullRequestAnnotations, PullRequestReviewSnapshot, PullRequestReviewThread,
};

#[doc = " One line of a comment is what a queue row shows; the whole body is in"]
#[doc = " the JSON."]
const MAX_EXCERPT_CHARS: usize = 72;

mod model;
mod queue;

pub(crate) use model::*;
pub(crate) use queue::{FeedbackInputs, build_feedback};

#[cfg(test)]
mod tests;
