use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{PullRequest, PullRequestReviewSnapshot, PullRequestReviewThread, Repository};

#[doc = " A suggestion replaces a handful of lines. One that claims to replace"]
#[doc = " more than this is not a suggestion anyone wrote by hand, and applying"]
#[doc = " it blind would rewrite a file rather than fix a line."]
const MAX_SUGGESTION_LINES: usize = 512;
#[doc = " The fence GitHub renders as an apply-able suggestion."]
const SUGGESTION_FENCE: &str = "suggestion";

mod apply;
mod model;
mod parse;

pub(crate) use apply::SuggestionPlan;
pub(crate) use model::*;
pub(crate) use parse::{collect_suggestions, suggestion_body};

#[cfg(test)]
mod tests;
