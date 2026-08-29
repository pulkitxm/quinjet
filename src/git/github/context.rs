use std::path::PathBuf;

use serde::Serialize;

use super::{
    CheckAnnotation, MergeGate, PullRequest, PullRequestAnnotations, PullRequestCommits,
    PullRequestDependencies, PullRequestDiffIndex, PullRequestReviewSnapshot,
};

#[doc = " A budget is in characters rather than tokens: Quinjet has no tokenizer"]
#[doc = " and guessing one would be worse than a number the caller can reason"]
#[doc = " about. Four characters per token is the usual rule of thumb."]
pub(crate) const DEFAULT_CONTEXT_BUDGET: usize = 30_000;
#[doc = " Below this, nothing useful survives truncation and the caller almost"]
#[doc = " certainly meant something else."]
const MIN_BUDGET: usize = 500;
#[doc = " A section shorter than this is not worth the heading above it, so"]
#[doc = " below this much room a section is dropped whole rather than cut."]
const MIN_SECTION_CHARACTERS: usize = 125;

mod build;
mod model;

pub(crate) use build::{ContextInputs, build_context};
pub(crate) use model::*;

#[cfg(test)]
mod tests;
