use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;

use super::{
    CacheLife, PullRequest, PullRequestDiffIndex, Repository, cache_read, parse_tsv_record,
};

#[doc = " Which check runs published annotations changes as a run finishes, so the"]
#[doc = " listing is kept on the same short clock the check list uses."]
const ANNOTATION_LIST_CACHE_TTL: Duration = Duration::from_secs(30);
#[doc = " A pull request whose CI writes more annotations than this has stopped"]
#[doc = " being a review aid; the cap is reported rather than silently applied."]
const MAX_ANNOTATIONS: usize = 500;
#[doc = " A ceiling on how many annotated check runs are read in one pass."]
const MAX_ANNOTATED_CHECK_RUNS: usize = 32;
const CHECK_RUN_TSV_FIELDS: usize = 5;
const ANNOTATION_TSV_FIELDS: usize = 10;

const CHECK_RUN_TSV_JQ: &str = r#".check_runs[] | [(.id|tostring), (.name // ""), ((.output.annotations_count // 0)|tostring), (.html_url // ""), (.status // "")] | @tsv"#;
const ANNOTATION_TSV_JQ: &str = r#".[] | [(.path // ""), ((.start_line // 0)|tostring), ((.end_line // 0)|tostring), ((.start_column // 0)|tostring), ((.end_column // 0)|tostring), (.annotation_level // ""), (.title // ""), (.message // ""), (.raw_details // ""), (.blob_href // "")] | @tsv"#;

mod coverage;
mod model;
mod query;

pub(crate) use coverage::{annotated_paths, mark_diff_coverage, visible_lines};
pub(crate) use model::*;

#[cfg(test)]
mod tests;
