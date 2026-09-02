use std::ffi::OsString;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use super::{CacheLife, GitHubRepository, PullRequest, Repository, parse_tsv_record};

#[doc = " A dependency comparison is keyed by two immutable object names, so its"]
#[doc = " answer never changes. Alerts do, so they keep a short clock."]
const ALERT_CACHE_TTL: Duration = Duration::from_secs(60);
const MAX_DEPENDENCY_CHANGES: usize = 500;
const MAX_ALERTS: usize = 200;
const DEPENDENCY_TSV_FIELDS: usize = 8;
const VULNERABILITY_TSV_FIELDS: usize = 6;
const CODE_SCANNING_TSV_FIELDS: usize = 7;

const DEPENDENCY_TSV_JQ: &str = r#".[] | [(.change_type // ""), (.manifest // ""), (.ecosystem // ""), (.name // ""), (.version // ""), (.license // ""), (.scope // ""), ((.vulnerabilities | length)|tostring)] | @tsv"#;
const VULNERABILITY_TSV_JQ: &str = r#".[] | . as $change | (.vulnerabilities[]? | [($change.name // ""), ($change.version // ""), (.severity // ""), (.advisory_ghsa_id // ""), (.advisory_summary // ""), (.first_patched_version // "")] | @tsv)"#;
const CODE_SCANNING_TSV_JQ: &str = r#".[] | [((.number // 0)|tostring), (.rule.id // ""), (.rule.severity // ""), (.rule.description // ""), (.most_recent_instance.location.path // ""), ((.most_recent_instance.location.start_line // 0)|tostring), (.html_url // "")] | @tsv"#;

mod alerts;
mod model;
mod review;

pub(crate) use model::*;

#[cfg(test)]
mod tests;
